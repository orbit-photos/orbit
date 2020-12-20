mod sys;

use std::mem::MaybeUninit;
use nalgebra::{Matrix3, Rotation3};
use sys::*;
use std::os::raw::c_int;

pub struct Detector {
    detector: *mut apriltag_detector_t,
    family: *mut apriltag_family_t,
    image_width: u32,
    image_height: u32,
    focal_length_pixels: f64,
    tag_size_meters: f32,
}

impl Detector {
    pub fn new(image_width: u32, image_height: u32, focal_length_pixels: f64, tag_size_meters: f32) -> Detector {
        unsafe {
            let family = tag36h11_create();
            let detector = apriltag_detector_create();
            apriltag_detector_add_family_bits(detector, family, 2);

            Detector { detector, family, image_width, image_height, focal_length_pixels, tag_size_meters }
        }
    }

    pub fn camera_euler_angles(&mut self, gray_image_data: &[u8]) -> Result<EulerAngles, NoTagFound> {
        assert_eq!(self.image_width*self.image_height, gray_image_data.len() as u32);

        unsafe {
            let greyscale = &mut image_u8_t {
                width: self.image_width as c_int,
                height: self.image_height as c_int,
                stride: self.image_width as c_int,
                buf: gray_image_data.as_ptr() as *mut u8,
            };

            let detections = apriltag_detector_detect(self.detector, greyscale);

            if (*detections).size == 0 { return Err(NoTagFound) }

            // get first element
            let detection = *((*detections).data as *const *const apriltag_detection_t as *const *mut apriltag_detection_t);

            let mut info = apriltag_detection_info_t {
                det: detection,
                tagsize: self.tag_size_meters as f64,
                fx: self.focal_length_pixels as f64,
                fy: self.focal_length_pixels as f64,
                cx: self.image_width as f64 / 2.0,
                cy: self.image_height as f64 / 2.0,
            };

            let mut pose: MaybeUninit<_> = MaybeUninit::zeroed();
            let _error = estimate_tag_pose(&mut info, pose.as_mut_ptr());
            let pose: apriltag_pose_t = pose.assume_init();

            let euler_angles = EulerAngles::from_apriltag_pose(pose);

            matd_destroy(pose.R);
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
        let matd_rot = pose.R;
        assert_eq!((*matd_rot).nrows, 3);
        assert_eq!((*matd_rot).ncols, 3);
        let elems = (*matd_rot).data.as_slice(9);

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
