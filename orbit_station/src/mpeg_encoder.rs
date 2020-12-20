/*************
NOTICE: This module was taken and modified from the unmaintained crate `mpeg_encoder`, at
https://github.com/sebcrozet/mpeg_encoder and https://crates.io/crates/mpeg_encoder.
This usage was not endorsed by him. His crate is licensed under the 3 Clause BSD License:

Copyright (c) 2017, Sébastien Crozet
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

3. Neither the name of the author nor the names of its contributors may be used
   to endorse or promote products derived from this software without specific
   prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND
ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 *************/


//! MPEG  video encoder.
//!

#![deny(non_camel_case_types)]
#![deny(non_upper_case_globals)]
#![deny(unused_qualifications)]
#![deny(missing_docs)]
#![deny(unused_results)]


// Inspired by the muxing sample: http://ffmpeg.org/doxygen/trunk/muxing_8c-source.html

use ffmpeg_sys_next::{SwsContext, AVCodec, AVCodecContext, AVPacket, AVFormatContext, AVStream,
                      AVFrame, AVRational, AVPixelFormat, AVPicture, AVCodecID};
use std::ptr;
use std::mem;
use std::path::{Path, PathBuf};
use std::ffi::CString;
use std::sync::Once;
use image::RgbImage;

static mut AVFORMAT_INIT: Once = Once::new();

/// MPEG video recorder.
pub struct Encoder {
    tmp_frame_buf: Vec<u8>,
    frame_buf: Vec<u8>,
    curr_frame_index: usize,
    initialized: bool,
    bit_rate: usize,
    target_width: usize,
    target_height: usize,
    time_base: (usize, usize),
    gop_size: usize,
    max_b_frames: usize,
    pix_fmt: AVPixelFormat,
    tmp_frame: *mut AVFrame,
    frame: *mut AVFrame,
    context: *mut AVCodecContext,
    format_context: *mut AVFormatContext,
    video_st: *mut AVStream,
    scale_context: *mut SwsContext,
    path: PathBuf,
}

impl Encoder {
    /// Creates a new video recorder.
    ///
    /// # Arguments:
    /// * `path`   - path to the output file.
    /// * `width`  - width of the recorded video.
    /// * `height` - height of the recorded video.
    pub fn new<P: AsRef<Path>>(path: P, width: usize, height: usize) -> Encoder {
        Encoder::new_with_params(path, width, height, None, None, None, None, None)
    }

    /// Creates a new video recorder with custom recording parameters.
    ///
    /// # Arguments:
    /// * `path`         - path to the output file.
    /// * `width`        - width of the recorded video.
    /// * `height`       - height of the recorded video.
    /// * `bit_rate`     - the average bit rate. Default value: 400000.
    /// * `time_base`    - this is the fundamental unit of time (in seconds) in terms of which
    ///                    frame timestamps are represented. Default value: (1, 60), i-e, 60fps.
    /// * `gop_size`     - the number of pictures in a group of pictures. Default value: 10.
    /// * `max_b_frames` - maximum number of B-frames between non-B-frames. Default value: 1.
    /// * `pix_fmt`      - pixel format. Default value: `AVPixelFormat::PIX_FMT_YUV420P`.
    pub fn new_with_params<P: AsRef<Path>>(
        path: P,
        width: usize,
        height: usize,
        bit_rate: Option<usize>,
        time_base: Option<(usize, usize)>,
        gop_size: Option<usize>,
        max_b_frames: Option<usize>,
        pix_fmt: Option<AVPixelFormat>
    ) -> Encoder {
        unsafe {
            AVFORMAT_INIT.call_once(|| {
                ffmpeg_sys_next::av_register_all();
            });
        }

        let bit_rate = bit_rate.unwrap_or(40_0000); // FIXME
        let time_base = time_base.unwrap_or((1, 60));
        let gop_size = gop_size.unwrap_or(10);
        let max_b_frames = max_b_frames.unwrap_or(1);
        let pix_fmt = pix_fmt.unwrap_or(AVPixelFormat::AV_PIX_FMT_YUV420P);
        // width and height must be a multiple of two.
        let width = if width % 2 == 0 {
            width
        } else {
            width + 1
        };
        let height = if height % 2 == 0 {
            height
        } else {
            height + 1
        };

        let mut pathbuf = PathBuf::new();
        pathbuf.push(path);

        Encoder {
            initialized: false,
            curr_frame_index: 0,
            bit_rate,
            target_width: width,
            target_height: height,
            time_base,
            gop_size,
            max_b_frames,
            pix_fmt,
            frame: ptr::null_mut(),
            tmp_frame: ptr::null_mut(),
            context: ptr::null_mut(),
            scale_context: ptr::null_mut(),
            format_context: ptr::null_mut(),
            video_st: ptr::null_mut(),
            path: pathbuf,
            frame_buf: Vec::new(),
            tmp_frame_buf: Vec::new(),
        }
    }

    pub fn encode_image(&mut self, image: &RgbImage) {
        self.encode_rgb(
            image.width() as usize,
            image.height() as usize,
            &*image
        );
    }

    /// Adds a image with a RGB pixel format to the video.
    pub fn encode_rgb(
        &mut self,
        width: usize,
        height: usize,
        data: &[u8]
    ) {
        assert!(!data.is_empty());
        assert_eq!(data.len(), width * height * 3);

        self.init();

        let mut pkt: AVPacket = unsafe { mem::MaybeUninit::uninit().assume_init() };
        unsafe { ffmpeg_sys_next::av_init_packet(&mut pkt); }
        pkt.data = ptr::null_mut();  // packet data will be allocated by the encoder
        pkt.size = 0;

        // Fill the snapshot frame.
        //
        //
        self.tmp_frame_buf.clear();
        self.tmp_frame_buf.extend_from_slice(data);

        unsafe {
            (*self.tmp_frame).width = width as i32;
            (*self.tmp_frame).height = height as i32;

            let _ = ffmpeg_sys_next::avpicture_fill(
                self.tmp_frame as *mut AVPicture,
                &self.tmp_frame_buf[0],
                AVPixelFormat::AV_PIX_FMT_RGB24,
                width as i32,
                height as i32,
            );
        }

        // Convert the snapshot frame to the right format for the destination frame.
        //
        unsafe {
            self.scale_context =
                ffmpeg_sys_next::sws_getCachedContext(
                    self.scale_context,
                    width as i32,
                    height as i32,
                    AVPixelFormat::AV_PIX_FMT_RGB24,
                    self.target_width as i32,
                    self.target_height as i32,
                    AVPixelFormat::AV_PIX_FMT_YUV420P,
                    ffmpeg_sys_next::SWS_BICUBIC as i32,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null()
                );

            let _ = ffmpeg_sys_next::sws_scale(
                self.scale_context,
                &(*self.tmp_frame).data[0] as *const *mut u8 as *const *const u8,
                &(*self.tmp_frame).linesize[0],
                0,
                height as i32,
                &(*self.frame).data[0] as *const *mut u8,
                &(*self.frame).linesize[0],
            );
        }


        // Encode the image.

        let mut got_output = 0;
        let ret;

        unsafe {
            ret = ffmpeg_sys_next::avcodec_encode_video2(
                self.context,
                &mut pkt,
                self.frame,
                &mut got_output
            );
        }

        if ret < 0 {
            panic!("Error encoding frame.");
        }

        if got_output != 0 {
            unsafe {
                let _ = ffmpeg_sys_next::av_interleaved_write_frame(self.format_context, &mut pkt);
                ffmpeg_sys_next::av_free_packet(&mut pkt);
            }
        }

        unsafe {
            (*self.frame).pts += ffmpeg_sys_next::av_rescale_q(
                1,
                (*self.context).time_base,
                (*self.video_st).time_base
            );
            self.curr_frame_index += self.curr_frame_index;
        }
    }

    /// Initializes the recorder if needed.
    ///
    /// This is automatically called when the first snapshot is made. Call this explicitly if you
    /// do not want the extra time overhead when the first snapshot is made.
    pub fn init(&mut self) {
        if self.initialized {
            return;
        }

        let path_str = CString::new(self.path.to_str().unwrap()).unwrap();

        unsafe {
            // try to guess the container type from the path.
            let mut fmt = ptr::null_mut();

            let _ = ffmpeg_sys_next::avformat_alloc_output_context2(&mut fmt, ptr::null_mut(), ptr::null(), path_str.as_ptr());

            if fmt.is_null() {
                // could not guess, default to MPEG
                let mpeg = CString::new(&b"mpeg"[..]).unwrap();

                let _ = ffmpeg_sys_next::avformat_alloc_output_context2(&mut fmt, ptr::null_mut(), mpeg.as_ptr(), path_str.as_ptr());
            }

            self.format_context = fmt;

            if self.format_context.is_null() {
                panic!("Unable to create the output context.");
            }

            let fmt = (*self.format_context).oformat;

            if (*fmt).video_codec == AVCodecID::AV_CODEC_ID_NONE {
                panic!("The selected output container does not support video encoding.")
            }

            let codec: *mut AVCodec;

            let ret: i32 = 0;

            codec = ffmpeg_sys_next::avcodec_find_encoder((*fmt).video_codec);

            if codec.is_null() {
                panic!("Codec not found.");
            }

            self.video_st = ffmpeg_sys_next::avformat_new_stream(self.format_context, ptr::null());

            if self.video_st.is_null() {
                panic!("Failed to allocate the video stream.");
            }

            (*self.video_st).id = ((*self.format_context).nb_streams - 1) as i32;

            self.context = ffmpeg_sys_next::avcodec_alloc_context3(codec);

            if self.context.is_null() {
                panic!("Could not allocate video codec context.");
            }

            // sws scaling context
            self.scale_context = ffmpeg_sys_next::sws_getContext(
                self.target_width as i32, self.target_height as i32, AVPixelFormat::AV_PIX_FMT_RGB24,
                self.target_width as i32, self.target_height as i32, self.pix_fmt,
                ffmpeg_sys_next::SWS_BICUBIC as i32, ptr::null_mut(), ptr::null_mut(), ptr::null()
            );

            (*self.context).codec_id = (*fmt).video_codec;

            // Put sample parameters.
            (*self.context).bit_rate = self.bit_rate as i64;

            // Resolution must be a multiple of two.
            (*self.context).width    = self.target_width  as i32;
            (*self.context).height   = self.target_height as i32;

            // frames per second.
            let (tnum, tdenum) = self.time_base;
            (*self.context).time_base    = AVRational { num: tnum as i32, den: tdenum as i32 };
            (*self.video_st).time_base   = (*self.context).time_base;
            (*self.context).gop_size     = self.gop_size as i32;
            (*self.context).max_b_frames = self.max_b_frames as i32;
            (*self.context).pix_fmt      = self.pix_fmt;

            if (*self.context).codec_id == AVCodecID::AV_CODEC_ID_MPEG1VIDEO {
                // Needed to avoid using macroblocks in which some coeffs overflow.
                // This does not happen with normal video, it just happens here as
                // the motion of the chroma plane does not match the luma plane.
                (*self.context).mb_decision = 2;
            }

            // Open the codec.
            if ffmpeg_sys_next::avcodec_open2(self.context, codec, ptr::null_mut()) < 0 {
                panic!("Could not open the codec.");
            }

            // Init destination video frame
            self.frame = ffmpeg_sys_next::av_frame_alloc();

            if self.frame.is_null() {
                panic!("Could not allocate the video frame.");
            }

            (*self.frame).format = (*self.context).pix_fmt as i32;
            (*self.frame).width  = (*self.context).width;
            (*self.frame).height = (*self.context).height;
            (*self.frame).pts    = 0;

            // alloc the buffer
            let nframe_bytes = ffmpeg_sys_next::avpicture_get_size(
                self.pix_fmt,
                self.target_width as i32,
                self.target_height as i32
            );

            self.frame_buf = vec![0; nframe_bytes as usize];

            let _ = ffmpeg_sys_next::avpicture_fill(
                self.frame as *mut AVPicture,
                &self.frame_buf[0],
                self.pix_fmt,
                self.target_width as i32,
                self.target_height as i32
            );

            // Init the temporary video frame.
            self.tmp_frame = ffmpeg_sys_next::av_frame_alloc();

            if self.tmp_frame.is_null() {
                panic!("Could not allocate the video frame.");
            }

            (*self.tmp_frame).format = (*self.context).pix_fmt as i32;
            // the rest (width, height, data, linesize) are set at the moment of the snapshot.

            if ffmpeg_sys_next::avcodec_parameters_from_context((*self.video_st).codecpar, self.context) < 0 {
                panic!("Failed to set codec parameters.");
            }

            ffmpeg_sys_next::av_dump_format(self.format_context, 0, path_str.as_ptr(), 1);

            // Open the output file.
            static AVIO_FLAG_WRITE: i32 = 2; // XXX: this should be defined by the bindings.
            if ffmpeg_sys_next::avio_open(&mut (*self.format_context).pb, path_str.as_ptr(), AVIO_FLAG_WRITE) < 0 {
                panic!("Failed to open the output file.");
            }

            if ffmpeg_sys_next::avformat_write_header(self.format_context, ptr::null_mut()) < 0 {
                panic!("Failed to open the output file.");
            }

            if ret < 0 {
                panic!("Could not allocate raw picture buffer");
            }
        }

        self.initialized = true;
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        if self.initialized {
            // Get the delayed frames.
            let mut pkt: AVPacket = unsafe { mem::MaybeUninit::uninit().assume_init() };
            let mut got_output = 1;
            while got_output != 0 {
                let ret;

                unsafe {
                    ffmpeg_sys_next::av_init_packet(&mut pkt);
                }

                pkt.data = ptr::null_mut();  // packet data will be allocated by the encoder
                pkt.size = 0;

                unsafe {
                    ret = ffmpeg_sys_next::avcodec_encode_video2(
                        self.context,
                        &mut pkt,
                        ptr::null(),
                        &mut got_output
                    );
                }

                if ret < 0 {
                    panic!("Error encoding frame.");
                }

                if got_output != 0 {
                    unsafe {
                        let _ = ffmpeg_sys_next::av_interleaved_write_frame(
                            self.format_context,
                            &mut pkt
                        );
                        ffmpeg_sys_next::av_free_packet(&mut pkt);
                    }
                }
            }

            // Write trailer
            unsafe {
                if ffmpeg_sys_next::av_write_trailer(self.format_context) < 0 {
                    panic!("Error writing trailer.");
                }
            }

            // Free things and stuffs.
            unsafe {
                ffmpeg_sys_next::avcodec_free_context(&mut self.context);
                ffmpeg_sys_next::av_frame_free(&mut self.frame);
                ffmpeg_sys_next::av_frame_free(&mut self.tmp_frame);
                ffmpeg_sys_next::sws_freeContext(self.scale_context);
                if ffmpeg_sys_next::avio_closep(&mut (*self.format_context).pb) < 0 {
                    println!("Warning: failed closing output file");
                }
                ffmpeg_sys_next::avformat_free_context(self.format_context);
            }
        }
    }
}
