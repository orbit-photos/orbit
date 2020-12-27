#![allow(dead_code)]

mod mpeg_encoder;
mod camera;
mod picture;
mod state;
mod frame_receiver;

use std::net::SocketAddr;
use glium::{glutin};
use crate::state::{State};
use std::sync::{Arc, mpsc};
use std::sync::atomic::AtomicU32;
use frame_receiver::capture_loop;

const TAG_SIZE_METERS: f64 = 162.0 / 1000.0;
const INITIAL_WINDOW_WIDTH: u32 = STREAM_ASPECT_WIDTH*400;
const INITIAL_WINDOW_HEIGHT: u32 = STREAM_ASPECT_HEIGHT*400;
const STREAM_ASPECT_WIDTH: u32 = 9;
const STREAM_ASPECT_HEIGHT: u32 = 16;
const STILL_CAPTURE_DELAY_MILLIS: i64 = 500;
const FOCAL_LENGTH_PIXELS: f64 = 1484.1702992321477;

fn main() {
    let addrs: Vec<SocketAddr>= vec![
        "192.168.2.100:2000".parse().unwrap(),
        "192.168.2.101:2000".parse().unwrap(),
    ];


    let pictures_taken = Arc::new(AtomicU32::new(0));
    let (message_sender, message_receiver) = mpsc::channel();

    capture_loop(addrs, message_sender, Arc::clone(&pictures_taken));

    let event_loop = glutin::event_loop::EventLoop::new();
    let mut state = State::new(INITIAL_WINDOW_WIDTH, INITIAL_WINDOW_HEIGHT, &event_loop, Arc::clone(&pictures_taken));

    event_loop.run(move |event, _, control_flow| {
        state.event_handler(event, control_flow, &message_receiver)
    });
}
