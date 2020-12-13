use crate::{TAG_SIZE_METERS};
use piston_window::{PistonWindow, WindowSettings, Texture, TextureSettings, Transformed, ButtonEvent};
use crate::camera::CameraParameters;
use crate::picture::ImageTransformExt;
use piston_window::texture::CreateTexture;
use crate::apriltag::Detector;
use std::collections::HashMap;
use crate::calibration::CalibrationResult;
use std::sync::Arc;
//
// pub fn live_calibration_demo(device_index: usize) {
//     let config = crate::CAMERA_CONFIG;
//     let camera = CameraParameters::SQ11;
//
//     let mut detector = Detector::new(camera, TAG_SIZE_METERS);
//
//     let mut stream = Camera::new(&format!("/dev/video{}", device_index)).unwrap();
//     stream.start(&config).unwrap();
//
//     let mut window: PistonWindow = WindowSettings::new("Webcam", (1920, 1080))
//         .exit_on_esc(true)
//         .build()
//         .unwrap();
//
//     let mut texture_context = window.create_texture_context();
//
//     while let Some(e) = window.next() {
//         window.draw_2d(&e, |c, g, _| {
//             let frame = stream.capture().unwrap();
//             let mut image = Image::from_rscam_frame(frame);
//
//             if let Ok(euler_angles) = detector.get_tag_pose(&image) {
//                 image = image
//                     .rotate(dbg!(euler_angles.yaw))
//                     .with_border(1920, 1920)
//                     .shift(0, camera.vertical_size_pixels( 0.5-euler_angles.pitch) as isize)
//                     .rescale(1080, 1080);
//
//                 let texture = Texture::create(
//                     &mut texture_context,
//                     piston_window::texture::Format::Rgba8,
//                     &image.data_with_alpha(),
//                     [image.width() as u32, image.height() as u32],
//                     &TextureSettings::new(),
//                 ).unwrap();
//
//                 piston_window::image(&texture, c.transform, g);
//             }
//         });
//     }
// }
//
//
// pub fn show_with_calibration(calibration_result: CalibrationResult, cameras: &[Arc<Camera>]) {
//     const WINDOW_WIDTH: u32 = 1920;
//     const WINDOW_HEIGHT: u32 = 1080;
//
//     let mut window: PistonWindow = WindowSettings::new("Webcam", (WINDOW_WIDTH, WINDOW_HEIGHT))
//         .exit_on_esc(true)
//         .build()
//         .unwrap();
//
//     let mut texture_context = window.create_texture_context();
//
//     while let Some(e) = window.next() {
//         window.draw_2d(&e, |c, g, device| {
//             let mut x = 0;
//             const WIDTH: usize = 16 * 40;
//             const HEIGHT: usize = 9 * 40;
//
//             piston_window::rectangle_from_to(
//                 [0.0, 0.0, 0.0, 1.0],
//                 [0.0, 0.0], [WINDOW_WIDTH as f64, WINDOW_HEIGHT as f64],
//                 c.transform, g
//             );
//
//             for (camera, adjustment) in cameras.iter().zip(&calibration_result.adjustments) {
//                 let frame = if let Ok(frame) = camera.capture() { frame } else { continue };
//
//                 let image = Image::from_rscam_frame(frame);
//                 let image = adjustment.apply(&image, calibration_result.output_size)
//                     .rescale(WIDTH, HEIGHT);
//
//                 let texture = Texture::create(
//                     &mut texture_context,
//                     piston_window::texture::Format::Rgba8,
//                     &image.data_with_alpha(),
//                     [WIDTH as u32, HEIGHT as u32],
//                     &TextureSettings::new(),
//                 ).unwrap();
//
//                 let a = WIDTH * (x % 3);
//                 let b = HEIGHT * (x / 3);
//
//                 let transform = c.transform.trans(a as f64, b as f64);
//
//                 piston_window::image(&texture, transform, g);
//
//                 x += 1;
//             }
//         });
//     }
// }
//
// pub fn show_all_cameras() {
//     let config = crate::CAMERA_CONFIG;
//
//     const WINDOW_WIDTH: u32 = 1920;
//     const WINDOW_HEIGHT: u32 = 1080;
//
//     let mut cameras_map = HashMap::new();
//
//     let mut window: PistonWindow = WindowSettings::new("Webcam", (WINDOW_WIDTH, WINDOW_HEIGHT))
//         .exit_on_esc(true)
//         .build()
//         .unwrap();
//
//     let mut texture_context = window.create_texture_context();
//
//     let mut glyphs = window.load_font("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf").unwrap();
//
//     while let Some(e) = window.next() {
//         if e.button_args().is_some() { break }
//
//         window.draw_2d(&e, |c, g, device| {
//             let mut x = 0;
//             const WIDTH: usize = 16*40;
//             const HEIGHT: usize = 9*40;
//
//             piston_window::rectangle_from_to(
//                 [0.0, 0.0, 0.0, 1.0],
//                 [0.0, 0.0], [WINDOW_WIDTH as f64, WINDOW_HEIGHT as f64],
//                 c.transform, g
//             );
//
//
//             for file in std::fs::read_dir("/sys/class/video4linux").unwrap() {
//                 let mut file = file.unwrap().path().to_str().unwrap().to_string();
//                 let index = file.pop().unwrap();
//                 let path = format!("/dev/video{}", index);
//
//                 let frame =
//                     if cameras_map.contains_key(&path) {
//                         cameras_map.get(&path).unwrap()
//                     } else if let Ok(mut camera) = Camera::new(&path) {
//                         camera.start(&config).unwrap();
//                         cameras_map.insert(path.clone(), camera);
//                         cameras_map.get(&path).unwrap()
//                     } else {
//                         continue;
//                     }.capture();
//
//                 let frame = if let Ok(frame) = frame { frame } else { continue };
//
//                 let image = Image::from_rscam_frame(frame);
//                 let image = image.rescale(WIDTH, HEIGHT);
//                 let (width, height) = (image.width() as u32, image.height() as u32);
//
//                 let texture = Texture::create(
//                     &mut texture_context,
//                     piston_window::texture::Format::Rgba8,
//                     &image.data_with_alpha(),
//                     [width, height],
//                     &TextureSettings::new(),
//                 ).unwrap();
//
//                 let a = WIDTH * (x % 3);
//                 let b = HEIGHT * (x / 3);
//
//                 let transform = c.transform.trans(a as f64, b as f64);
//
//                 piston_window::image(&texture, transform, g);
//                 piston_window::text::Text::new_color([1.0, 0.0, 0.0, 1.0], 32).draw(
//                     &format!("{}", index),
//                     &mut glyphs,
//                     &c.draw_state,
//                     transform.trans(0.0, 32.0),
//                     g
//                 ).unwrap();
//
//                 glyphs.factory.encoder.flush(device);
//
//                 x += 1;
//             }
//         });
//     }
// }