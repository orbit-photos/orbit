#![allow(non_camel_case_types)]

use crate::picture::{ImageTransformExt};
use crate::camera::{CameraParameters};

use std::mem::MaybeUninit;
use nalgebra::{Matrix3, Rotation3};
use sys::*;
use std::os::raw::c_int;
use image::{DynamicImage, GenericImageView, GrayImage};

pub struct Detector {
    detector: *mut apriltag_detector_t,
    family: *mut apriltag_family_t,
    parameters: CameraParameters,
    tag_size_meters: f32,
}

impl Detector {
    pub fn new(parameters: CameraParameters, tag_size_meters: f32) -> Detector {
        unsafe {
            let family = tag36h11_create();
            let detector = apriltag_detector_create();
            apriltag_detector_add_family_bits(detector, family, 2);

            Detector { detector, family, parameters, tag_size_meters }
        }
    }

    pub fn camera_euler_angles(&mut self, image: &GrayImage) -> Result<EulerAngles, NoTagFound> {
        assert_eq!(image.width(), self.parameters.image_width_pixels() as u32);
        assert_eq!(image.height(), self.parameters.image_height_pixels() as u32);

        unsafe {
            let greyscale = &mut image_u8_t {
                width: image.width() as c_int,
                height: image.height() as c_int,
                stride: image.width() as c_int,
                buf: image.as_ptr(),
            };

            let detections = apriltag_detector_detect(self.detector, greyscale);
            
            if (*detections).size == 0 { return Err(NoTagFound) }

            // get first element
            let detection = *((*detections).data as *const *const apriltag_detection_t);

            let mut info = apriltag_detection_info_t {
                det: detection,
                tagsize: self.tag_size_meters as f64,
                fx: self.parameters.focal_length_pixels() as f64, // focal length, pixels
                fy: self.parameters.focal_length_pixels() as f64, //
                cx: self.parameters.image_width_pixels() as f64 / 2.0,
                cy: self.parameters.image_height_pixels() as f64 / 2.0,
            };

            let mut pose: MaybeUninit<_> = MaybeUninit::zeroed();
            let _error = estimate_tag_pose(&mut info, pose.as_mut_ptr());
            let pose: apriltag_pose_t = pose.assume_init();

            let euler_angles = EulerAngles::from_apriltag_pose(pose);

            matd_destroy(pose.r);
            matd_destroy(pose.t);
            apriltag_detections_destroy(detections);

            Ok(euler_angles)
        }
    }
}

impl Drop for Detector {
    fn drop(&mut self) {
        unsafe {
            apriltag_detector_destroy(self.detector);
            tag36h11_destroy(self.family);
        }
    }
}

#[derive(Debug)]
pub struct NoTagFound;

#[derive(Debug, Copy, Clone)]
pub struct EulerAngles {
    pub pitch: f32,
    pub roll: f32,
    pub yaw: f32,
}

impl EulerAngles {
    unsafe fn from_apriltag_pose(pose: apriltag_pose_t) -> EulerAngles {
        let matd_rot = pose.r;
        assert_eq!((*matd_rot).nrows, 3);
        assert_eq!((*matd_rot).ncols, 3);
        let elems = (*matd_rot).data.slice(9);

        let matrix = Matrix3::from_row_slice(elems);

        let (rx, ry, rz) = Rotation3::from_matrix(&matrix)
            .inverse() // convert from apriltag pose to camera pose
            .euler_angles();

        EulerAngles {
            pitch: rx as f32,
            roll: ry as f32,
            yaw: rz as f32,
        }
    }
}

mod sys {
    use libc::{c_int, c_float, c_double, c_char, size_t, pthread_mutex_t, pthread_t, pthread_cond_t, c_uint, c_void};
    use std::marker::PhantomData;
    use std::slice;

    #[link(name = "apriltag")]
    extern {
        pub fn apriltag_detector_create() -> *mut apriltag_detector_t;
        pub fn apriltag_detector_detect(td: *mut apriltag_detector_t, im_orig: *mut image_u8_t) -> *mut zarray_t;

        pub fn apriltag_detector_destroy(detector: *mut apriltag_detector_t);

        pub fn apriltag_detections_destroy(detections: *mut zarray_t);

        pub fn apriltag_detector_add_family_bits(detector: *mut apriltag_detector_t, family: *mut apriltag_family_t, bits_corrected: c_int);

        pub fn estimate_tag_pose(info: *mut apriltag_detection_info_t, pose: *mut apriltag_pose_t) -> c_double;

        pub fn matd_destroy(matd: *mut matd_t);

        pub fn tag36h11_create() -> *mut apriltag_family_t;
        pub fn tag36h11_destroy(family: *mut apriltag_family_t);
    }

    #[derive(Debug)]
    #[repr(C)]
    pub struct CDynamicArray<T> {
        phantom_data: PhantomData<T>,
    }

    impl<T> CDynamicArray<T> {
        pub unsafe fn slice(&self, len: usize) -> &[T] {
            let ptr = self as *const CDynamicArray<T> as *const T;
            slice::from_raw_parts(ptr, len)
        }
    }

    #[repr(C)]
    pub struct apriltag_detection_info_t {
        pub det: *const apriltag_detection_t,
        pub tagsize: c_double,
        // In meters.
        pub fx: c_double,
        // In pixels.
        pub fy: c_double,
        // In pixels.
        pub cx: c_double,
        // In pixels.
        pub cy: c_double, // In pixels.
    }

    #[derive(Debug, Clone, Copy)]
    #[repr(C)]
    pub struct apriltag_pose_t {
        pub r: *mut matd_t,
        pub t: *mut matd_t,
    }

    #[derive(Debug)]
    #[repr(C)]
    pub struct apriltag_detection_t {
        family: *const apriltag_family_t,
        id: c_int,
        hamming: c_int,
        decision_margin: c_float,
        h: *const matd_t,
        c: [c_double; 2],
        p: [[c_double; 2]; 4],
    }

    #[derive(Debug)]
    #[repr(C)]
    pub struct matd_t {
        pub nrows: c_uint,
        pub ncols: c_uint,
        pub data: CDynamicArray<c_double>,
    }

    #[repr(C)]
    pub struct apriltag_family_t {
        ncodes: u32,
        codes: *const u64,
        width_at_border: c_int,
        total_width: c_int,
        reversed_border: bool,
        nbits: u32,
        bit_x: *const u32,
        bit_y: *const u32,
        h: u32,
        name: *const c_char,
        r#impl: *const c_void,
    }

    #[derive(Debug, Copy, Clone)]
    #[repr(C)]
    pub struct image_u8_t {
        pub width: i32,
        pub height: i32,
        pub stride: i32,
        pub buf: *const u8,
    }


    #[repr(C)]
    struct workerpool_t {
        nthreads: c_int,
        tasks: *const zarray_t,
        taskspos: c_int,
        threads: *const pthread_t,
        status: *const c_int,

        mutex: pthread_mutex_t,
        startcond: pthread_cond_t,
        endcond: pthread_cond_t,
        end_count: c_int,
    }

    #[derive(Debug)]
    #[repr(C)]
    struct timeprofile_t {
        utime: i64,
        stamps: *const zarray_t,
    }

    #[derive(Debug)]
    #[repr(C)]
    pub struct zarray_t {
        pub el_sz: size_t,
        pub size: c_int,
        pub alloc: c_int,
        pub data: *const c_char,
    }

    #[derive(Debug)]
    #[repr(C)]
    struct apriltag_quad_thresh_params {
        min_cluster_pixels: c_int,
        max_nmaxima: c_int,
        critical_rad: c_float,
        cos_critical_rad: c_float,
        max_line_fit_mse: c_float,
        min_white_black_diff: c_int,
        deglitch: c_int,
    }

    #[repr(C)]
    pub struct apriltag_detector_t {
        nthreads: c_int,
        quad_decimate: c_float,
        quad_sigma: c_float,
        refine_edges: c_int,
        decode_sharpening: c_double,

        debug: c_int,

        qtp: apriltag_quad_thresh_params,
        tp: timeprofile_t,

        nedges: u32,
        nsegments: u32,
        nquads: u32,

        tag_families: *const zarray_t,

        wp: *const workerpool_t,
        mutex: pthread_mutex_t,
    }
}