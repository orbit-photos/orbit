use std::collections::HashMap;
use std::net::SocketAddr;

use image::ImageFormat;

use apriltag::{ApriltagDetector, EulerAngles};
use orbit_types::CapturedFrame;

use crate::{FOCAL_LENGTH_PIXELS, STREAM_ASPECT_HEIGHT, STREAM_ASPECT_WIDTH, TAG_SIZE_METERS};
use crate::picture::crop_rotate_scale;
use crate::streams::{Streams, StreamSource};

#[derive(Debug, Copy, Clone)]
pub struct Adjustment {
    roll: f64,
    pitch: f64,
}

impl Adjustment {
    pub fn new(stream: StreamSource, events: &[CalibrationEvent]) -> Adjustment {
        // take a weighted average of the adjustments over all of the samples that include uur
        // stream
        let mut roll_numerator = 0.0;
        let mut pitch_numerator = 0.0;
        let mut total_samples = 0.0;

        for event in events {
            if let Some(euler_angles) = event.includes_streams.get(&stream) {
                let measurement_count = event.includes_streams.len() as f64;
                roll_numerator += measurement_count*(euler_angles.roll - event.average_roll.read());
                pitch_numerator += measurement_count*(euler_angles.pitch - event.average_pitch.read());
                total_samples += measurement_count;
            }
        }

        if total_samples == 0.0 {
            Adjustment { roll: 0.0, pitch: 0.0 }
        } else {
            Adjustment {
                roll: roll_numerator/total_samples,
                pitch: pitch_numerator/total_samples,
            }
        }
    }

    pub fn roll(&self) -> f64 {
        self.roll
    }

    pub fn get_crop_factor(&self) -> f64 {
        let total_rotation = self.roll;
        crop_rotate_scale(STREAM_ASPECT_HEIGHT as f64/STREAM_ASPECT_WIDTH as f64, total_rotation)
    }
}

pub struct CalibrationEvent {
    includes_streams: HashMap<StreamSource, EulerAngles>,
    average_pitch: Averager,
    average_roll: Averager,
}

impl CalibrationEvent {
    pub fn new(
        devices: Vec<(SocketAddr, Vec<CapturedFrame>)>,
        apriltag_detector: &mut ApriltagDetector,
        streams: &Streams,
    ) -> CalibrationEvent {
        let mut average_pitch = Averager::new();
        let mut average_roll = Averager::new();
        let mut includes_streams = HashMap::new();

        for (socket_addr, stills) in devices {
            for still in stills {
                let source = StreamSource::new(socket_addr, still.device_id());

                let image = image::load_from_memory_with_format(
                    still.frame_data(),
                    ImageFormat::Jpeg,
                );

                if let Ok(image) = image {
                    if let Some(ordinal) = streams.get_stream_tile(source) {
                        // ^^ if we're in the condition where we get a frame that we've never seen
                        // before, the cardinal rotation might be wrong and the whole calibration
                        // will be messed up. Instead, we choose to ignore the frame

                        let image = streams
                            .cardinal_transform_image(ordinal, &image)
                            .into_luma8();

                        let detection = apriltag_detector.search(
                            image.as_raw(),
                            image.width(),
                            image.height(),
                            TAG_SIZE_METERS,
                            FOCAL_LENGTH_PIXELS,
                        );

                        match detection.first() {
                            Some(detection) => {
                                let euler_angles = detection.euler_angles();

                                average_pitch.add(euler_angles.pitch);
                                average_roll.add(euler_angles.roll);
                                includes_streams.insert(source, euler_angles);
                            },
                            None => {}, // TODO: display the ones that fail on the screen
                        }
                    }
                }
            }
        }

        CalibrationEvent { includes_streams, average_pitch, average_roll }
    }
}

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
}
