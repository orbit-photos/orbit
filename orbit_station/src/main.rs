#![allow(dead_code)]

mod mpeg_encoder;
mod camera;
mod calibration;
mod calibrate;
mod picture;
mod webcam;

use std::net::SocketAddr;
use glium::backend::glutin::glutin::dpi::LogicalSize;
use webcam::Vertex;
use glium::index::PrimitiveType;
use glium::{program, glutin};
use crate::webcam::Streams;
use std::sync::{Arc, mpsc};
use std::sync::atomic::AtomicU32;

const TAG_SIZE_METERS: f32 = 162.0 / 1000.0;
const INITIAL_WINDOW_WIDTH: u32 = 920;
const INITIAL_WINDOW_HEIGHT: u32 = 800;
const STREAM_ASPECT_RATIO: (u32, u32) = (16, 9);
const STILL_CAPTURE_DELAY_MILLIS: i64 = 500;

fn main() {
    let addrs: Vec<SocketAddr>= vec![
        "192.168.2.100:2000".parse().unwrap(),
        // "192.168.2.101:2000".parse().unwrap(),
    ];

    let event_loop = glutin::event_loop::EventLoop::new();
    let wb = glutin::window::WindowBuilder::new()
        .with_inner_size(LogicalSize::new(INITIAL_WINDOW_WIDTH as f32, INITIAL_WINDOW_HEIGHT as f32))
        .with_title("Orbit Station");
    let cb = glutin::ContextBuilder::new().with_vsync(true);
    let display = glium::Display::new(wb, cb, &event_loop).unwrap();

    let vertex_buffer = glium::VertexBuffer::new(&display, &[
        Vertex { position: [-1.0, -1.0], tex_coords: [0.0, 0.0] },
        Vertex { position: [-1.0, 1.0], tex_coords: [0.0, 1.0] },
        Vertex { position: [1.0, 1.0], tex_coords: [1.0, 1.0] },
        Vertex { position: [1.0, -1.0], tex_coords: [1.0, 0.0] },
    ]).unwrap();

    let index_buffer = glium::IndexBuffer::new(
        &display,
        PrimitiveType::TriangleStrip,
        &[1 as u16, 2, 0, 3]
    ).unwrap();

    let program = program!(&display,
        140 => {
            vertex: include_str!("vertex.glsl"),
            fragment: include_str!("fragment.glsl"),
        },
    ).unwrap();

    let mut streams = Streams::new(INITIAL_WINDOW_WIDTH, INITIAL_WINDOW_HEIGHT);
    let pictures_taken = Arc::new(AtomicU32::new(0));
    let (message_sender, message_receiver) = mpsc::channel();

    webcam::capture_loop(addrs, message_sender, Arc::clone(&pictures_taken));

    event_loop.run(move |event, thing, control_flow| {
        webcam::event_handler(event, thing, control_flow, &vertex_buffer, &index_buffer, &program, &display, &mut streams, &pictures_taken, &message_receiver);
    });
}
