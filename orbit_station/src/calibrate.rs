// use image::{GrayImage};
// use apriltag::{ApriltagDetector, EulerAngles};
// use std::collections::HashMap;
// use crate::webcam::StreamId;
// use crate::TAG_SIZE_METERS;
//
// pub struct CalibrationData {
//     euler_angles: EulerAngles,
//     average_pitch: Averager,
//     average_roll: Averager,
// }
//
// pub fn calibrate(
//     detector: &mut ApriltagDetector,
//     images: &[(StreamId, GrayImage)]
// ) -> Result<HashMap<StreamId, CalibrationData>, NoTagsFoundAt> {
//
//     let mut euler_angles_map = Vec::new();
//     let average_roll = Averager::new();
//     let average_pitch = Averager::new();
//
//     let mut no_tags_found_at = Vec::new();
//
//     for &(id, ref image) in images {
//         match detector.search(image, image.width(), image.height(), TAG_SIZE_METERS, todo!()) {
//             Ok(detection) => {
//                 let euler_angles = detection.euler_angles();
//                 euler_angles_map.push((id, euler_angles));
//                 average_pitch.add(euler_angles.pitch);
//                 average_roll.add(euler_angles.roll);
//             },
//             Err(_) => no_tags_found_at.push(id),
//         }
//     }
//
//     if no_tags_found_at.is_empty() {
//         Ok(euler_angles_map.iter()
//             .map(|&(id, euler_angles)| {
//                 (id, CalibrationData { euler_angles, average_pitch, average_roll })
//             })
//             .collect())
//     } else {
//         Err(NoTagsFoundAt {cameras: no_tags_found_at })
//     }
// }
//
// pub struct NoTagsFoundAt {
//     cameras: Vec<StreamId>,
// }

#[derive(Copy, Clone)]
pub struct Averager {
    sum: f64,
    measurement_count: f64,
}

impl Averager {
    pub fn new() -> Averager {
        Averager { sum: 0.0, measurement_count: 0.0 }
    }

    pub fn add(&mut self, value: f64) {
        self.sum += value;
        self.measurement_count += 1.0;
    }

    pub fn measurement_count(self) -> f64 {
        self.measurement_count
    }

    pub fn read(self) -> f64 {
        self.sum / self.measurement_count
    }

    fn merge(self, other: Averager) -> Averager {
        Averager {
            sum: self.sum + other.sum,
            measurement_count: self.measurement_count + other.measurement_count,
        }
    }
}

