use image::{GrayImage, DynamicImage, ImageResult};
use crate::apriltag::{Detector, NoTagFound, EulerAngles};
use crate::picture::ImageTransformExt;
use nalgebra::Scalar;
use std::collections::HashMap;
use crate::webcam::CameraId;

pub struct CalibrationData {
    euler_angles: EulerAngles,
    average_pitch: Averager,
    average_roll: Averager,
}

pub fn calibrate(
    detector: &mut Detector,
    images: &[(CameraId, GrayImage)]
) -> Result<HashMap<CameraId, CalibrationData>, NoTagsFoundAt> {

    let mut euler_angles_map = Vec::new();
    let mut average_roll = Averager::new();
    let mut average_pitch = Averager::new();

    let mut no_tags_found_at = Vec::new();

    for &(id, ref image) in images {
        match detector.camera_euler_angles(image) {
            Ok(euler_angles) => {
                euler_angles_map.push((id, euler_angles));
                average_pitch.add(euler_angles.pitch);
                average_roll.add(euler_angles.roll);
            },
            Err(_) => no_tags_found_at.push(id),
        }
    }

    if no_tags_found_at.is_empty() {
        Ok(euler_angles_map.iter()
            .map(|&(id, euler_angles)| {
                (id, CalibrationData { euler_angles, average_pitch, average_roll })
            })
            .collect())
    } else {
        Err(NoTagsFoundAt {cameras: no_tags_found_at })
    }
}

pub struct NoTagsFoundAt {
    cameras: Vec<CameraId>,
}

#[derive(Copy, Clone)]
pub struct Averager {
    sum: f32,
    count: f32,
}

impl Averager {
    fn new() -> Averager {
        Averager { sum: 0.0, count: 0.0 }
    }

    fn add(&mut self, value: f32) {
        self.sum += value;
        self.count += 1.0;
    }

    fn read(self) -> f32 {
        self.sum / self.count
    }

    fn merge(self, other: Averager) -> Averager {
        Averager {
            sum: self.sum + other.sum,
            count: self.count + other.count,
        }
    }
}

