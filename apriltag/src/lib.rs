#![allow(dead_code)]

mod sys;

use std::mem::MaybeUninit;
use nalgebra::{Matrix3, Rotation3, Point3};
use sys::*;
use std::os::raw::c_int;
use std::slice;

pub struct ApriltagDetector {
    detector: *mut apriltag_detector_t,
    family: *mut apriltag_family_t,
}

impl ApriltagDetector {
    pub fn new(family: TagFamily) -> ApriltagDetector {
        unsafe {
            let family = match family {
                TagFamily::Tag16h5 => tag16h5_create(),
                TagFamily::Tag36h11 => tag36h11_create(),
            };
            let detector = apriltag_detector_create();
            apriltag_detector_add_family_bits(detector, family, 2);

            ApriltagDetector { detector, family }
        }
    }

    pub fn search(
        &mut self,
        gray_image_data: &[u8],
        image_width: u32,
        image_height: u32,
        tag_size_meters: f64,
        focal_length_pixels: f64,
    ) -> Vec<ApriltagDetection> {
        assert_eq!(image_width*image_height, gray_image_data.len() as u32);

        unsafe {
            let greyscale = &mut image_u8_t {
                width: image_width as c_int,
                height: image_height as c_int,
                stride: image_width as c_int,
                buf: gray_image_data.as_ptr() as *mut _,
            };

            let detections = apriltag_detector_detect(self.detector, greyscale);

            let detections_slice = slice::from_raw_parts(
                (*detections).data as *const *const apriltag_detection_t as *const *mut _,
                (*detections).size as usize,
            );

            dbg!((*detections).size as usize);

            let ret: Vec<_> = detections_slice.iter()
                .map(|&detection| {
                    let mut info = apriltag_detection_info_t {
                        det: detection,
                        tagsize: tag_size_meters,
                        fx: focal_length_pixels,
                        fy: focal_length_pixels,
                        cx: image_width as f64 / 2.0,
                        cy: image_height as f64 / 2.0,
                    };

                    dbg!();

                    let mut pose: MaybeUninit<_> = MaybeUninit::zeroed();
                    let error = estimate_tag_pose(&mut info, pose.as_mut_ptr());
                    let pose: apriltag_pose_t = pose.assume_init();

                    let center: [f64; 2] = (*detection).c;
                    let corners: [[f64; 2]; 4] = (*detection).p;

                    dbg!();

                    let d = ApriltagDetection::from_pose(
                        pose,
                        error,
                        (*detection).id as usize,
                        (center[0] as u32, center[1] as u32),
                        [
                            (corners[0][0] as u32, corners[0][1] as u32),
                            (corners[1][0] as u32, corners[1][1] as u32),
                            (corners[2][0] as u32, corners[2][1] as u32),
                            (corners[3][0] as u32, corners[3][1] as u32),
                        ],
                    );

                    dbg!();

                    matd_destroy(pose.R);
                    matd_destroy(pose.t);

                    d
                })
                .collect();

            apriltag_detections_destroy(detections);

            dbg!();

            ret
        }
    }
}

impl Drop for ApriltagDetector {
    fn drop(&mut self) {
        unsafe {
            apriltag_detector_destroy(self.detector);
            tag36h11_destroy(self.family);
        }
    }
}

#[derive(Copy, Clone)]
pub enum TagFamily {
    Tag36h11,
    Tag16h5,
}

pub struct ApriltagDetection {
    error: f64,
    tag_id: usize,
    image_center: (u32, u32),
    image_tag_corners: [(u32, u32); 4],
    rotation: Rotation3<f64>,
    translation: Point3<f64>,
}

impl ApriltagDetection {
    unsafe fn from_pose(
        pose: apriltag_pose_t,
        error: f64,
        tag_id: usize,
        image_center: (u32, u32),
        image_tag_corners: [(u32, u32); 4],
    ) -> ApriltagDetection {

        let matd_rot = pose.R;
        assert_eq!((*matd_rot).nrows, 3);
        assert_eq!((*matd_rot).ncols, 3);
        let rotation_matrix_elems = (*matd_rot).data.as_slice(9);
        let rotation_matrix = Matrix3::from_row_slice(rotation_matrix_elems);
        let rotation = Rotation3::from_matrix_eps(&rotation_matrix, 0.0001, 30, Rotation3::identity()).inverse();

        let matd_trans = pose.t;
        assert_eq!((*matd_trans).nrows, 3);
        assert_eq!((*matd_trans).ncols, 1);
        let translation_vector_components = (*matd_trans).data.as_slice(3);
        let translation = Point3::from_slice(translation_vector_components);

        ApriltagDetection { error, rotation, translation, tag_id, image_center, image_tag_corners }
    }

    pub fn euler_angles(&self) -> EulerAngles {
        let (pitch, roll, yaw) = self.rotation.euler_angles();
        EulerAngles { pitch, roll, yaw }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct EulerAngles {
    pub pitch: f64,
    pub roll: f64,
    pub yaw: f64,
}
