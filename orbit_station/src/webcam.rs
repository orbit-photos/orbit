use v4l::prelude::*;
use v4l::{FourCC};
use std::thread;
use image::{ImageFormat, RgbaImage, DynamicImage, RgbImage};
use crate::picture::ImageTransformExt;
use std::net::{TcpStream, IpAddr, SocketAddr};
use std::mem::size_of;
use std::io::{self, Read, Write};
use std::collections::HashMap;
use std::sync::mpsc::{Sender, Receiver};
use byteorder::{ReadBytesExt, BigEndian};
use orbit_types::{DeviceId, Request, StreamResponse, SnapResponse};
use chrono::Utc;
use glium::index::PrimitiveType;
use glium::{glutin, Surface};
use glium::{implement_vertex, program, uniform};
use std::sync::{mpsc, RwLock};
use std::time::Instant;
use orbit_types::CapturedFrame;
use glium::glutin::event::{DeviceEvent, VirtualKeyCode};
use glium::texture::ClientFormat;
use std::borrow::Cow;
use std::collections::hash_map::Entry;

const WINDOW_WIDTH: usize = 920;
const WINDOW_HEIGHT: usize = 1080;

const WEBCAM_STREAM_WIDTH: usize = 1280;
const WEBCAM_STREAM_HEIGHT: usize = 720;

const TILE_WIDTH: usize = 176;
const TILE_HEIGHT: usize = TILE_WIDTH*WEBCAM_STREAM_HEIGHT/WEBCAM_STREAM_WIDTH;
const HORIZONTAL_TILES: usize = WINDOW_WIDTH/TILE_WIDTH;

#[derive(Eq, PartialEq, Hash, Copy, Clone, Debug)]
pub struct CameraId {
    socket_addr: SocketAddr,
    device_id: DeviceId,
}

pub enum ThreadRequest {
    Stop,
    Stream,
    Shutter,
}

pub fn network(addrs: &[SocketAddr]) {
    // Setup the GL display stuff
    let event_loop = glutin::event_loop::EventLoop::new();
    let wb = glutin::window::WindowBuilder::new();
    let cb = glutin::ContextBuilder::new().with_vsync(true);
    let display = glium::Display::new(wb, cb, &event_loop).unwrap();

    // building the vertex buffer, which contains all the vertices that we will draw
    let vertex_buffer = {
        #[derive(Copy, Clone)]
        struct Vertex {
            position: [f32; 2],
            tex_coords: [f32; 2],
        }

        implement_vertex!(Vertex, position, tex_coords);

        glium::VertexBuffer::new(
            &display,
            &[
                Vertex {
                    position: [-1.0, -1.0],
                    tex_coords: [0.0, 0.0],
                },
                Vertex {
                    position: [-1.0, 1.0],
                    tex_coords: [0.0, 1.0],
                },
                Vertex {
                    position: [1.0, 1.0],
                    tex_coords: [1.0, 1.0],
                },
                Vertex {
                    position: [1.0, -1.0],
                    tex_coords: [1.0, 0.0],
                },
            ],
        ).unwrap()
    };

    // building the index buffer
    let index_buffer = glium::IndexBuffer::new(
        &display,
        PrimitiveType::TriangleStrip,
        &[1 as u16, 2, 0, 3]
    ).unwrap();

    // compiling shaders and linking them together
    let program = program!(&display,
        140 => {
            vertex: "
                #version 140
                uniform mat4 matrix;
                in vec2 position;
                in vec2 tex_coords;
                out vec2 v_tex_coords;
                void main() {
                    gl_Position = matrix * vec4(position, 0.0, 1.0);
                    v_tex_coords = tex_coords;
                }
            ",

            fragment: "
                #version 140
                uniform sampler2D tex;
                in vec2 v_tex_coords;
                out vec4 f_color;
                void main() {
                    f_color = texture(tex, v_tex_coords);
                }
            "
        },
    ).unwrap();


    let mut frame_receivers = Vec::new();
    for &ip in addrs {
        let (image_sender, frame_receiver) = std::sync::mpsc::channel();
        frame_receivers.push(frame_receiver);

        thread::spawn(move || {
            stream(ip, &image_sender).unwrap();
        });
    }

    let mut offsets = HashMap::new();

    let mut i = 0;

    event_loop.run(move |event, _, control_flow| {
        let mut target = display.draw();
        target.clear_color(0.0, 0.0, 0.0, 0.0);

        for frame_receiver in frame_receivers.iter() {
            while let Ok((id, image)) = frame_receiver.try_recv() {
                let i = *offsets.entry(id).or_insert_with(|| {
                    let ret = i;
                    i += 1;
                    ret
                });

                dbg!(id);

                let tile_x = TILE_WIDTH * (i%HORIZONTAL_TILES);
                let tile_y = TILE_HEIGHT * (i/HORIZONTAL_TILES);

                let scale_x = 1.0 / 10.0;
                let scale_y = 1.0 / 10.0;

                let tile_x = tile_x as f32 * scale_x;
                let tile_y = tile_y as f32 * scale_y;

                let image = glium::texture::RawImage2d {
                    data: Cow::Borrowed(image.as_raw()),
                    width: image.width(),
                    height: image.height(),
                    format: ClientFormat::U8U8U8,
                };

                let opengl_texture = glium::texture::Texture2d::new(&display, image).unwrap();

                // building the uniforms
                let uniforms = uniform! {
                    matrix: [
                        [scale_x, 0.0, 0.0, 0.0],
                        [0.0, scale_y, 0.0, 0.0],
                        [0.0, 0.0, 1.0, 0.0],
                        [tile_x, tile_y, 0.0, 1.0]
                    ],
                    tex: &opengl_texture
                };

                // drawing a frame
                target.draw(
                    &vertex_buffer,
                    &index_buffer,
                    &program,
                    &uniforms,
                    &Default::default(),
                ).unwrap();
            }
        }
        target.finish().unwrap();

        // polling and handling the events received by the window
        match event {
            glutin::event::Event::WindowEvent { event, .. } => match event {
                glutin::event::WindowEvent::CloseRequested => {
                    *control_flow = glutin::event_loop::ControlFlow::Exit;
                }
                _ => {}
            },
            glutin::event::Event::DeviceEvent { event: DeviceEvent::Key(input), .. } => {
                match input.virtual_keycode {
                    Some(VirtualKeyCode::S) => {
                        dbg!("s pressed");
                    },
                    Some(VirtualKeyCode::Space) => {
                        dbg!("space pressed");
                    },
                    _ => {},
                }
            }
            _ => {}
        }
    });
}


// pub fn network(addrs: &[SocketAddr]) {
//     let mut frame_receivers = Vec::new();
//
//     for &ip in addrs {
//         let (image_sender, frame_receiver) = std::sync::mpsc::channel();
//         let (tx, thread_request_receiver) = std::sync::mpsc::channel();
//         frame_receivers.push(frame_receiver);
//
//         let mut request = ThreadRequest::Stream;
//
//         thread::spawn(move || {
//
//             loop {
//                 match request {
//                     ThreadRequest::Stream => {
//                         let new = stream(ip, &image_sender, &thread_request_receiver).unwrap();
//                         request = new;
//                     },
//                     ThreadRequest::Shutter => {
//                         let images = shutter(ip).unwrap();
//                     },
//                     ThreadRequest::Stop => {
//
//                     },
//                 }
//             }
//         });
//     }
//
//     // display code
//     let mut window: PistonWindow = WindowSettings::new("view", (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32))
//         .exit_on_esc(true)
//         .build()
//         .unwrap();
//
//     let mut texture_context = window.create_texture_context();
//
//     let mut textures = HashMap::new();
//
//     while let Some(e) = window.next() {
//         for frame_receiver in frame_receivers.iter() {
//             while let Ok((id, image)) = frame_receiver.try_recv() {
//                 textures.insert(
//                     id,
//                     Texture::from_image(
//                         &mut texture_context,
//                         &image,
//                         &TextureSettings::new()
//                     ).unwrap()
//                 );
//             }
//         }
//
//         e.button(|args| {
//             match args.button {
//                 Button::Keyboard(key) => {
//                     match key {
//                         Key::C => { // calibrate
//
//                         },
//                         Key::Space | Key::Return | Key::S => { // shutter
//
//                         }
//                         _ => {},
//                     }
//                 },
//                 _ => {},
//             }
//         });
//
//         window.draw_2d(&e, |c, g, _| {
//             clear([0.0; 4], g);
//
//             for (i, texture) in textures.values().enumerate() {
//                 let tile_x = TILE_WIDTH * (i%HORIZONTAL_TILES);
//                 let tile_y = TILE_HEIGHT * (i/HORIZONTAL_TILES);
//                 piston_window::image(
//                     texture,
//                     c.transform.trans(tile_x as f64, tile_y as f64),
//                     g
//                 );
//             }
//         });
//     }
// }

fn stream(
    socket_addr: SocketAddr,
    image_sender: &Sender<(CameraId, RgbImage)>,
) -> io::Result<ThreadRequest> {

    let mut connection = TcpStream::connect(socket_addr)?;
    bincode::serialize_into(
        &mut connection,
        &Request::Stream,
    ).unwrap();

    loop {
        let response: StreamResponse = bincode::deserialize_from(&mut connection).unwrap(); // TODO: remove unwrap

        match response {
            StreamResponse::Stop(stopped_device) => {
                // TODO: handle
            },
            StreamResponse::Frame(frame) => {
                let camera_id = CameraId { socket_addr, device_id: frame.device_id() };

                let image = image::load_from_memory_with_format(
                    frame.frame_data(),
                    ImageFormat::Jpeg,
                );

                if let Ok(image) = image {
                    let image = image.into_rgb8();
                    image_sender.send((camera_id, image)).unwrap();
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
            frame.frame_data(),
            ImageFormat::Jpeg,
        );

        if let Ok(image) = image {
            images.push(image);
        }
    }

    Ok(images)
}
