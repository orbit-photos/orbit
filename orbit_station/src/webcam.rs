use v4l::prelude::*;
use v4l::{FourCC};
use std::thread;
use piston_window::{WindowSettings, PistonWindow, Texture, TextureSettings, clear, Transformed, IdleEvent, ButtonEvent, Key, Button};
use image::{ImageFormat, RgbaImage, DynamicImage};
use crate::picture::ImageTransformExt;
use std::net::{TcpStream, IpAddr, SocketAddr};
use std::mem::size_of;
use std::io::{self, Read, Write};
use std::collections::HashMap;
use std::sync::mpsc::{Sender, Receiver};
use byteorder::{ReadBytesExt, BigEndian};
use orbit_types::{DeviceId, Request, StreamResponse, SnapResponse};
use chrono::Utc;


const WINDOW_WIDTH: usize = 920;
const WINDOW_HEIGHT: usize = 1080;

const WEBCAM_STREAM_WIDTH: usize = 1280;
const WEBCAM_STREAM_HEIGHT: usize = 720;

const TILE_WIDTH: usize = 176;
const TILE_HEIGHT: usize = TILE_WIDTH*WEBCAM_STREAM_HEIGHT/WEBCAM_STREAM_WIDTH;
const HORIZONTAL_TILES: usize = WINDOW_WIDTH/TILE_WIDTH;

#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub struct CameraId {
    socket_addr: SocketAddr,
    device_id: DeviceId,
}

pub enum ThreadRequest {
    Stop,
    Stream,
    Shutter,
}

pub fn network() {
    let ips: [SocketAddr; 2] = [
        "192.168.2.100:2000".parse().unwrap(),
        "192.168.2.101:2000".parse().unwrap(),
    ];

    let mut frame_receivers = Vec::new();

    for &ip in ips.iter() {
        let (image_sender, frame_receiver) = std::sync::mpsc::channel();
        let (tx, thread_request_receiver) = std::sync::mpsc::channel();
        frame_receivers.push(frame_receiver);

        let mut request = ThreadRequest::Stream;

        thread::spawn(move || {

            loop {
                match request {
                    ThreadRequest::Stream => {
                        let new = stream(ip, &image_sender, &thread_request_receiver).unwrap();
                        request = new;
                    },
                    ThreadRequest::Shutter => {
                        let images = shutter(ip).unwrap();
                    },
                    ThreadRequest::Stop => {

                    },
                }
            }
        });
    }

    // display code
    let mut window: PistonWindow = WindowSettings::new("view", (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32))
        .exit_on_esc(true)
        .build()
        .unwrap();

    let mut texture_context = window.create_texture_context();

    let mut textures = HashMap::new();

    while let Some(e) = window.next() {
        for frame_receiver in frame_receivers.iter() {
            while let Ok((id, image)) = frame_receiver.try_recv() {
                textures.insert(
                    id,
                    Texture::from_image(
                        &mut texture_context,
                        &image,
                        &TextureSettings::new()
                    ).unwrap()
                );
            }
        }

        e.button(|args| {
            match args.button {
                Button::Keyboard(key) => {
                    match key {
                        Key::C => { // calibrate

                        },
                        Key::Space | Key::Return | Key::S => { // shutter

                        }
                        _ => {},
                    }
                },
                _ => {},
            }
        });

        window.draw_2d(&e, |c, g, _| {
            clear([0.0; 4], g);

            for (i, texture) in textures.values().enumerate() {
                let tile_x = TILE_WIDTH * (i%HORIZONTAL_TILES);
                let tile_y = TILE_HEIGHT * (i/HORIZONTAL_TILES);
                piston_window::image(
                    texture,
                    c.transform.trans(tile_x as f64, tile_y as f64),
                    g
                );
            }
        });
    }
}

fn stream(
    socket_addr: SocketAddr,
    image_sender: &Sender<(CameraId, RgbaImage)>,
    thread_request_receiver: &Receiver<ThreadRequest>,
) -> io::Result<ThreadRequest> {

    let mut connection = TcpStream::connect(socket_addr)?;
    bincode::serialize_into(
        &mut connection,
        &Request::Stream,
    ).unwrap();

    loop {
        let response: StreamResponse = bincode::deserialize_from(&mut connection).unwrap(); // TODO: remove unwrap

        match thread_request_receiver.try_recv() {
            Ok(ThreadRequest::Stream) => {},
            Ok(message) => return Ok(message),
            Err(_) => {},
        }

        match response {
            StreamResponse::Stop(stopped_device) => {
                // TODO: handle
            },
            StreamResponse::Frame(frame) => {
                let camera_id = CameraId { socket_addr, device_id: frame.device_id() };

                let image = image::load_from_memory_with_format(
                    &frame.frame_data(),
                    ImageFormat::Jpeg,
                );

                if let Ok(image) = image {
                    let image = image
                        .thumbnail_exact(TILE_WIDTH as u32, TILE_HEIGHT as u32)
                        .into_rgba8();

                    image_sender.send((camera_id, image)).unwrap(); // TODO: remove unwrap
                }
            },
        }
    }
}

fn shutter(
    socket_addr: SocketAddr,
) -> io::Result<Vec<DynamicImage>> {

    let mut connection = TcpStream::connect(socket_addr)?;

    bincode::serialize_into(
        &mut connection,
        &Request::Snap(Utc::now() + chrono::Duration::seconds(3)),
    ).unwrap();

    let snap_response: SnapResponse = bincode::deserialize_from(&mut connection).unwrap();

    let mut images = Vec::new();

    for frame in snap_response.frames {
        let image = image::load_from_memory_with_format(
            &frame.frame_data(),
            ImageFormat::Jpeg,
        );

        if let Ok(image) = image {
            images.push(image);
        }
    }

    Ok(images)
}
