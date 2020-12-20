#![allow(dead_code)]
#![allow(unused_imports)]

mod apriltag;
mod mpeg_encoder;
mod camera;
mod demos;
mod calibration;
mod calibrate;
mod message;
mod picture;
mod webcam;
mod display;

const OUTPUT_WIDTH: usize = 500;
const OUTPUT_HEIGHT: usize = 500;
const TAG_SIZE_METERS: f32 = 162.0 / 1000.0;

const INPUT_FOLDER: &str = "resources/images10";
const OUTPUT_FILE: &str = "output.mp4";


use std::{fs, io};

use crate::picture::{ImageTransformExt};
use crate::camera::CameraParameters;
use crate::calibration::{CalibrationResult};
use crate::apriltag::Detector;
use image::{GenericImageView, EncodableLayout, DynamicImage};
use std::f32::consts::PI;
use v4l::prelude::{CaptureDevice, MmapStream};
use v4l::buffer::Stream;
use v4l::FourCC;
use rand::{thread_rng, Rng};
use std::net::SocketAddr;

const INITIAL_WINDOW_WIDTH: u32 = 920;
const INITIAL_WINDOW_HEIGHT: u32 = 800;
const STREAM_ASPECT_RATIO: (u32, u32) = (16, 9);


fn main() {
    let addrs: &[SocketAddr] = &[
        "192.168.2.100:2000".parse().unwrap(),
        // "192.168.2.101:2000".parse().unwrap(),
    ];

    // println!("{}", webcam::integer_square_root(0));

    webcam::network(&addrs);
}