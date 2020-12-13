use std::sync::Arc;
use v4l::{device, v4l2, Buffer, Memory};
use std::{io, mem};
use v4l::buffer::{StreamItem, Metadata};
use std::os::raw::c_short;
use crate::POLL_TIMEOUT_MILLIS;
use crate::polling_stream_fork::public_arena::{Arena};
use v4l::v4l_sys::*;

pub mod public_arena {
    use std::{io, mem, ptr, slice, sync::Arc};

    use v4l::v4l2;
    use v4l::v4l_sys::*;
    use v4l::{device, memory::Memory};

    /// Manage mapped buffers
    ///
    /// All buffers are unmapped in the Drop impl.
    /// In case of errors during unmapping, we panic because there is memory corruption going on.
    pub struct Arena<'a> {
        handle: Arc<device::Handle>,
        bufs: Vec<&'a [u8]>,
    }

    impl<'a> Arena<'a> {
        /// Returns a new buffer manager instance
        ///
        /// You usually do not need to use this directly.
        /// A MappedBufferStream creates its own manager instance by default.
        pub fn new(dev: &dyn device::Device) -> Self {
            Arena {
                handle: dev.handle(),
                bufs: Vec::new(),
            }
        }

        pub fn allocate(&mut self, count: u32) -> io::Result<u32> {
            let mut v4l2_reqbufs: v4l2_requestbuffers;
            unsafe {
                v4l2_reqbufs = mem::zeroed();
                v4l2_reqbufs.type_ = v4l2_buf_type_V4L2_BUF_TYPE_VIDEO_CAPTURE;
                v4l2_reqbufs.count = count;
                v4l2_reqbufs.memory = Memory::Mmap as u32;
                v4l2::ioctl(
                    self.handle.fd(),
                    v4l2::vidioc::VIDIOC_REQBUFS,
                    &mut v4l2_reqbufs as *mut _ as *mut std::os::raw::c_void,
                )?;
            }

            for i in 0..v4l2_reqbufs.count {
                let mut v4l2_buf: v4l2_buffer;
                unsafe {
                    v4l2_buf = mem::zeroed();
                    v4l2_buf.type_ = v4l2_buf_type_V4L2_BUF_TYPE_VIDEO_CAPTURE;
                    v4l2_buf.memory = Memory::Mmap as u32;
                    v4l2_buf.index = i;
                    v4l2::ioctl(
                        self.handle.fd(),
                        v4l2::vidioc::VIDIOC_QUERYBUF,
                        &mut v4l2_buf as *mut _ as *mut std::os::raw::c_void,
                    )?;

                    v4l2::ioctl(
                        self.handle.fd(),
                        v4l2::vidioc::VIDIOC_QBUF,
                        &mut v4l2_buf as *mut _ as *mut std::os::raw::c_void,
                    )?;

                    let ptr = v4l2::mmap(
                        ptr::null_mut(),
                        v4l2_buf.length as usize,
                        libc::PROT_READ | libc::PROT_WRITE,
                        libc::MAP_SHARED,
                        self.handle.fd(),
                        v4l2_buf.m.offset as libc::off_t,
                    )?;

                    let slice = slice::from_raw_parts::<u8>(ptr as *mut u8, v4l2_buf.length as usize);
                    self.bufs.push(slice);
                }
            }

            Ok(v4l2_reqbufs.count)
        }

        pub fn release(&mut self) -> io::Result<()> {
            for buf in &self.bufs {
                unsafe {
                    v4l2::munmap(buf.as_ptr() as *mut core::ffi::c_void, buf.len())?;
                }
            }

            // free all buffers by requesting 0
            let mut v4l2_reqbufs: v4l2_requestbuffers;
            unsafe {
                v4l2_reqbufs = mem::zeroed();
                v4l2_reqbufs.type_ = v4l2_buf_type_V4L2_BUF_TYPE_VIDEO_CAPTURE;
                v4l2_reqbufs.count = 0;
                v4l2_reqbufs.memory = Memory::Mmap as u32;
                v4l2::ioctl(
                    self.handle.fd(),
                    v4l2::vidioc::VIDIOC_REQBUFS,
                    &mut v4l2_reqbufs as *mut _ as *mut std::os::raw::c_void,
                )?;
            }

            self.bufs.clear();
            Ok(())
        }

        pub fn get_unchecked(&self, index: usize) -> &'a [u8] {
            &self.bufs[index]
        }
    }

    impl<'a> Drop for Arena<'a> {
        fn drop(&mut self) {
            if let Err(e) = self.release() {
                if let Some(code) = e.raw_os_error() {
                    // ENODEV means the file descriptor wrapped in the handle became invalid, most
                    // likely because the device was unplugged or the connection (USB, PCI, ..)
                    // broke down. Handle this case gracefully by ignoring it.
                    if code == 19 {
                        /* ignore */
                        return;
                    }
                }

                panic!("{:?}", e)
            }
        }
    }
}

/// Stream of mapped buffers
///
/// An arena instance is used internally for buffer handling.
pub struct Stream<'a> {
    handle: Arc<device::Handle>,
    arena: Arena<'a>,
    arena_index: usize,
    arena_len: u32,
}

impl<'a> Stream<'a> {
    pub fn with_buffers(dev: &dyn device::Device, count: u32) -> io::Result<Self> {
        let mut arena = Arena::new(dev);
        let count = arena.allocate(count)?;

        Ok(Stream {
            handle: dev.handle(),
            arena,
            arena_index: 0,
            arena_len: count,
        })
    }

    pub fn start(self) -> io::Result<ActiveStream<'a>> {
        unsafe {
            let mut typ = v4l2_buf_type_V4L2_BUF_TYPE_VIDEO_CAPTURE;
            v4l2::ioctl(
                self.handle.fd(),
                v4l2::vidioc::VIDIOC_STREAMON,
                &mut typ as *mut _ as *mut std::os::raw::c_void,
            )?;
        }

        Ok(ActiveStream {
            inner: self,
            queued: true,
        })
    }

    fn ioctl_stop(&mut self) -> io::Result<()> {
        unsafe {
            let mut typ = v4l2_buf_type_V4L2_BUF_TYPE_VIDEO_CAPTURE;
            v4l2::ioctl(
                self.handle.fd(),
                v4l2::vidioc::VIDIOC_STREAMOFF,
                &mut typ as *mut _ as *mut std::os::raw::c_void,
            )
        }
    }

}

pub struct ActiveStream<'a> {
    inner: Stream<'a>,
    queued: bool,
}

impl<'a> ActiveStream<'a> {
    pub fn next(&mut self) -> io::Result<StreamItem<'a, Buffer<'a>>> {
        self.queue()?;
        self.dequeue()
    }

    pub fn queue(&mut self) -> io::Result<()> {
        if self.queued {
            return Ok(());
        }

        let mut v4l2_buf: v4l2_buffer;
        unsafe {
            v4l2_buf = mem::zeroed();
            v4l2_buf.type_ = v4l2_buf_type_V4L2_BUF_TYPE_VIDEO_CAPTURE;
            v4l2_buf.memory = Memory::Mmap as u32;
            v4l2_buf.index = self.inner.arena_index as u32;
            v4l2::ioctl(
                self.inner.handle.fd(),
                v4l2::vidioc::VIDIOC_QBUF,
                &mut v4l2_buf as *mut _ as *mut std::os::raw::c_void,
            )?;
        }

        self.inner.arena_index = (self.inner.arena_index + 1) % self.inner.arena_len as usize;

        Ok(())
    }

    pub fn dequeue(&mut self) -> io::Result<StreamItem<'a, Buffer<'a>>> {
        unsafe {
            let mut poll_fd = libc::pollfd {
                fd: self.inner.handle.fd(),
                events: v4l2::vidioc::VIDIOC_DQBUF as c_short,
                revents: 0,
            };
            let devices_set = libc::poll(&mut poll_fd, 1, POLL_TIMEOUT_MILLIS);

            if devices_set != 1 { return Err(io::ErrorKind::TimedOut.into()) }
        }

        let mut v4l2_buf: v4l2_buffer;
        unsafe {
            v4l2_buf = mem::zeroed();
            v4l2_buf.type_ = v4l2_buf_type_V4L2_BUF_TYPE_VIDEO_CAPTURE;
            v4l2_buf.memory = Memory::Mmap as u32;
            v4l2::ioctl(
                self.inner.handle.fd(),
                v4l2::vidioc::VIDIOC_DQBUF,
                &mut v4l2_buf as *mut _ as *mut std::os::raw::c_void,
            )?;
        }
        self.queued = false;

        let view = self.inner.arena.get_unchecked(v4l2_buf.index as usize);
        let buf = Buffer::new(
            view,
            Metadata {
                bytesused: v4l2_buf.bytesused,
                flags: v4l2_buf.flags.into(),
                timestamp: v4l2_buf.timestamp.into(),
                sequence: v4l2_buf.sequence,
            },
        );
        Ok(StreamItem::new(buf))
    }
}


impl<'a> Drop for Stream<'a> {
    fn drop(&mut self) {
        if let Err(e) = self.ioctl_stop() {
            if let Some(code) = e.raw_os_error() {
                // ENODEV means the file descriptor wrapped in the handle became invalid, most
                // likely because the device was unplugged or the connection (USB, PCI, ..)
                // broke down. Handle this case gracefully by ignoring it.
                if code == 19 {
                    /* ignore */
                    return;
                }
            }

            panic!("{:?}", e)
        }
    }
}
