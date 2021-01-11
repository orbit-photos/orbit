use std::borrow::Cow;
use std::f64::consts::TAU;
use std::net::SocketAddr;

use glium::texture::{ClientFormat, RawImage2d};
use image::{DynamicImage, RgbImage};

use apriltag::{ApriltagDetector};
use orbit_types::CapturedFrame;
use orbit_types::DeviceId;

use crate::calibration::{Adjustment, CalibrationEvent};
use crate::picture::{ImageTransformExt};

pub struct Streams {
    crop_factor: f64,
    calibration_events: Vec<CalibrationEvent>,
    streams: Vec<StreamInfo>, // in the order they are displayed on screen
}

impl Streams {
    pub fn new() -> Streams {
        Streams {
            crop_factor: 1.0,
            calibration_events: Vec::new(),
            streams: Vec::new(),
        }
    }

    pub fn crop_factor(&self) -> f64 {
        self.crop_factor
    }

    pub fn register_frame(&mut self, source: StreamSource, image: RgbImage) {
        match self.streams.iter_mut().find(|s| s.source == source) {
            Some(info) => info.image = image,
            None => self.streams.push(StreamInfo {
                source,
                flip_flop: FlipFlop::new(image.width(), image.height()),
                adjustment: None,
                image,
            }),
        }
    }

    pub fn calibrate(&mut self, devices: Vec<(SocketAddr, Vec<CapturedFrame>)>, detector: &mut ApriltagDetector) {
        self.calibration_events.push(CalibrationEvent::new(devices, detector, self));

        self.crop_factor = 1.0;
        for stream_info in self.streams.iter_mut() {
            let adjustment = Adjustment::new(stream_info.source, &self.calibration_events);
            stream_info.adjustment = Some(adjustment);
            self.crop_factor = self.crop_factor.min(adjustment.get_crop_factor());
        }
    }

    pub fn flip(&mut self, ordinal: StreamOrdinal) {
        if let Some(stream_info) = self.streams.get_mut(ordinal.index) {
            stream_info.flip();
        }
    }

    pub fn transform_image(&self, ordinal: StreamOrdinal, image: &DynamicImage) -> DynamicImage {
        self.streams[ordinal.index].transform_image(image, self.crop_factor)
    }

    pub fn cardinal_transform_image(&self, ordinal: StreamOrdinal, image: &DynamicImage) -> DynamicImage {
        self.streams[ordinal.index].cardinal_transform_image(image)
    }

    pub fn remove_and_insert(&mut self, old: StreamOrdinal, new: StreamOrdinal) {
        let info = self.streams.remove(old.index);
        self.streams.insert(new.index, info);
    }

    pub fn stream_count(&self) -> usize {
        self.streams.len()
    }

    pub fn get_ordinal(&self, index: usize) -> Option<StreamOrdinal> {
        if index < self.streams.len() {
            Some(StreamOrdinal { index })
        } else {
            None
        }
    }

    pub fn deregister_stream(&mut self, source: StreamSource) {
        if let Some(place) = self.streams.iter().position(|s| s.source == source) {
            self.streams.remove(place);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item=(StreamOrdinal, RawImage2d<u8>, f64)> + '_ {
        self.streams.iter().enumerate()
            .map(move |(index, stream_info)| (StreamOrdinal { index }, stream_info.glium_image(), stream_info.total_rotation_angle()))
    }

    pub fn get_stream_tile(&self, source: StreamSource) -> Option<StreamOrdinal> {
        let inner = self.streams.iter().position(|s| s.source == source)?;
        Some(StreamOrdinal { index: inner })
    }
}

#[derive(Eq, PartialEq, Hash, Copy, Clone, Debug, Ord, PartialOrd)]
pub struct StreamSource {
    socket_addr: SocketAddr,
    device_id: DeviceId,
}

impl StreamSource {
    pub fn new(socket_addr: SocketAddr, device_id: DeviceId) -> StreamSource {
        StreamSource { socket_addr, device_id }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StreamOrdinal {
    index: usize,
}

impl StreamOrdinal {
    pub fn index(&self) -> usize {
        self.index
    }
}

struct StreamInfo {
    source: StreamSource,
    flip_flop: FlipFlop,
    adjustment: Option<Adjustment>,
    image: RgbImage,
}

impl StreamInfo {
    fn flip(&mut self) {
        self.flip_flop.flip();
    }

    fn glium_image(&self) -> RawImage2d<u8> {
        RawImage2d {
            data: Cow::Borrowed(self.image.as_raw()),
            width: self.image.width(),
            height: self.image.height(),
            format: ClientFormat::U8U8U8,
        }
    }

    fn transform_image(&self, image: &DynamicImage, crop_factor: f64) -> DynamicImage {
        // TODO: pitch and yaw correction

        let image = self.flip_flop.rotate_image(image);

        let radians = self.adjustment.map_or(0.0, |a| a.roll()) as f32;

        image.crop_rotate(radians, crop_factor as f32)
    }

    fn cardinal_transform_image(&self, image: &DynamicImage) -> DynamicImage {
        self.flip_flop.rotate_image(image)
    }

    fn total_rotation_angle(&self) -> f64 {
        self.flip_flop.get_angle() + self.adjustment.map_or(0.0, |a| a.roll())
    }
}

#[derive(Copy, Clone, Debug)]
struct FlipFlop {
    /// Should we do a 90 degree rotation because we are using a landscape camera but we want vertical?
    flop: bool,
    /// Should we do a 180 degree rotation because the stream is upside-down?
    flip: bool,
}

impl FlipFlop {
    fn new(image_width: u32, image_height: u32) -> FlipFlop {
        let is_horizontal = image_width > image_height;
        FlipFlop { flop: is_horizontal, flip: false }
    }

    fn flip(&mut self) {
        self.flip = !self.flip;
    }

    fn rotate_image(self, image: &DynamicImage) -> DynamicImage {
        // N.B.: `image` likes to rotate images clockwise, and the convention with math
        // is that we do rotations counter-clockwise. So that's why the angles here don't match
        // `CardinalRotation::get_angle()`
        match self {
            FlipFlop { flop: false, flip: false } => image.clone(),
            FlipFlop { flop: false, flip: true } => image.rotate180(),
            FlipFlop { flop: true, flip: false } => image.rotate270(),
            FlipFlop { flop: true, flip: true } => image.rotate90(),
        }
    }

    fn get_angle(self) -> f64 {
        match self {
            FlipFlop { flop: false, flip: false } => 0.0*TAU/4.0,
            FlipFlop { flop: false, flip: true } => 2.0*TAU/4.0,
            FlipFlop { flop: true, flip: false } => 1.0*TAU/4.0,
            FlipFlop { flop: true, flip: true } => 3.0*TAU/4.0,
        }
    }
}
