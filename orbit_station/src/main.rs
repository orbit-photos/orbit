#![allow(dead_code)]

mod mpeg_encoder;
mod picture;
mod state;
mod streams;
mod layout_engine;
mod frame_receiver;
mod find_tags;
mod calibration;

use std::net::SocketAddr;
use glium::{glutin};
use glutin::event_loop::EventLoop;
use crate::state::{State, PictureEventState};
use std::sync::{mpsc};
use crate::frame_receiver::spawn_capture_loop;

const TAG_SIZE_METERS: f64 = 162.0 / 1000.0;
const INITIAL_WINDOW_WIDTH: u32 = STREAM_ASPECT_WIDTH*400;
const INITIAL_WINDOW_HEIGHT: u32 = STREAM_ASPECT_HEIGHT*400;
const STREAM_ASPECT_WIDTH: u32 = 9;
const STREAM_ASPECT_HEIGHT: u32 = 16;
const STILL_CAPTURE_DELAY_MILLIS: i64 = 2000;
const FOCAL_LENGTH_PIXELS: f64 = 1484.0;
const VIDEO_FRAMERATE: (usize, usize) = (1, 1); // 1 frame / second

fn main() {
    let addrs: Vec<SocketAddr>= vec![
        "192.168.2.100:2000".parse().unwrap(),
        "192.168.2.101:2000".parse().unwrap(),
        "192.168.2.102:2000".parse().unwrap(),
    ];

    let picture_event_state = PictureEventState::new();
    let (message_sender, message_receiver) = mpsc::channel();

    spawn_capture_loop(addrs, message_sender, picture_event_state.clone());

    let event_loop = EventLoop::new();
    let mut state = State::new(INITIAL_WINDOW_WIDTH, INITIAL_WINDOW_HEIGHT, &event_loop, picture_event_state.clone());

    event_loop.run(move |event, _, control_flow| {
        state.event_handler(event, control_flow, &message_receiver)
    });
}
