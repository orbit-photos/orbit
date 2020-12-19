use crate::picture::ImageTransformExt;
use crate::camera::CameraParameters;
use std::{thread, io};
use std::sync::{Arc};
use crate::apriltag::{Detector, NoTagFound, EulerAngles};
use std::path::Path;
use crate::mpeg_encoder::Encoder;
use image::{DynamicImage, GenericImageView, GrayImage};

#[derive(Copy, Clone)]
pub struct OutputSize {
    parameters: CameraParameters,
    min_vertical_shift: isize,
    max_vertical_shift: isize,
    output_width: usize,
    output_height: usize,
}

impl OutputSize {
    fn new(parameters: CameraParameters) -> OutputSize {
        let width = parameters.image_width_pixels() as usize;
        let height = parameters.image_height_pixels() as usize;

        OutputSize {
            parameters,
            output_width: width,
            output_height: height,
            min_vertical_shift: isize::MAX,
            max_vertical_shift: isize::MIN,
        }
    }

    fn update(&mut self, adjustment: Adjustment) {
        let roll = -adjustment.euler_angles.roll;
        let pitch = adjustment.euler_angles.pitch;

        let input_width = self.parameters.image_width_pixels();
        let input_height = self.parameters.image_height_pixels();

        let new_width = input_width*roll.cos().abs() + input_height*roll.sin().abs();
        let new_height = input_width*roll.sin().abs() + input_height*roll.cos().abs();

        let vertical_shift = self.parameters.vertical_size_pixels(pitch).round() as isize;

        self.min_vertical_shift = self.min_vertical_shift.min(vertical_shift as isize);
        self.max_vertical_shift = self.max_vertical_shift.max(vertical_shift as isize);

        self.output_width = self.output_width.max(new_width.round() as usize);
        self.output_height = self.output_height.max(new_height.round() as usize);
    }

    fn get_vertical_shift(&self, pitch: f32) -> isize {
        let vertical_shift = self.parameters.vertical_size_pixels(pitch) as isize;

        // know that vertical_shift is between self.min_vertical_shift and self.max_vertical_shift
        let offset = (self.min_vertical_shift + self.max_vertical_shift) / 2;
        vertical_shift - offset
    }
}

pub struct CalibrationResult {
    pub adjustments: Vec<Adjustment>,
    parameters: CameraParameters,
    pub output_size: OutputSize,
}

impl CalibrationResult {
    pub fn calibrate(_pictures: &[DynamicImage], _parameters: CameraParameters, _detector: &mut Detector) -> OrbitResult<CalibrationResult> {
        // let mut adjustments = Vec::with_capacity(cameras.len());
        //
        // let mut output_size = OutputSize::new(parameters);
        //
        // for image in capture_all(cameras) {
        //     let image = image?;
        //
        //     if let Ok(adjustment) = Adjustment::from_image(&image, detector) {
        //         adjustments.push(dbg!(adjustment));
        //
        //         output_size.update(adjustment);
        //     } else {
        //         image.write_to_file("uncooperative.png").unwrap();
        //         panic!("failed");
        //     }
        // }

        todo!()
    }

    pub fn capture<P: AsRef<Path>>(&self, cameras: Vec<DynamicImage>, output_path: P) -> OrbitResult<()> {
        let OutputSize { output_width, output_height, .. } = self.output_size;

        let mut encoder = Encoder::new(output_path, output_width, output_height);

        for (image, adjustment) in cameras.into_iter().zip(&self.adjustments) {
            let image = adjustment.apply(&image, self.output_size);

            assert_eq!((output_width as u32, output_height as u32), (image.width(), image.height()));
            let image = image.to_rgb8();

            encoder.encode_rgb(image.width() as usize, image.height() as usize, image.as_raw());
        }

        Ok(())
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Adjustment {
    euler_angles: EulerAngles,
}

impl Adjustment {
    fn from_image(image: &GrayImage, detector: &mut Detector) -> Result<Adjustment, NoTagFound> {
        let euler_angles = detector.camera_euler_angles(image)?;
        Ok(Adjustment { euler_angles })
    }

    pub fn apply(&self, image: &DynamicImage, output_size: OutputSize) -> DynamicImage {
        let roll = self.euler_angles.roll;
        let pitch = self.euler_angles.pitch;

        let _vertical_shift = output_size.get_vertical_shift(pitch);

        image
            .rotate(-roll)
    }
}


pub type OrbitResult<T> = Result<T, OrbitError>;

#[derive(Debug)]
pub enum OrbitError {
    IoError(io::Error),
    NoTagsFound,
}


impl From<NoTagFound> for OrbitError {
    fn from(_: NoTagFound) -> OrbitError {
        OrbitError::NoTagsFound
    }
}

impl From<io::Error> for OrbitError {
    fn from(error: io::Error) -> OrbitError {
        OrbitError::IoError(error)
    }
}