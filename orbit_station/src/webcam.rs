use v4l::prelude::*;
use v4l::{FourCC};
use std::thread;
use image::{ImageFormat, RgbaImage, DynamicImage, RgbImage};
use crate::picture::ImageTransformExt;
use std::net::{TcpStream, IpAddr, SocketAddr};
use std::mem::size_of;
use std::io::{self, Read, Write, BufReader};
use std::collections::HashMap;
use std::sync::mpsc::{Sender, Receiver};
use byteorder::{ReadBytesExt, BigEndian};
use orbit_types::{DeviceId, Request, StreamResponse, SnapResponse};
use chrono::Utc;
use glium::index::PrimitiveType;
use glium::{glutin, Surface, DrawParameters, Rect};
use glium::{implement_vertex, program, uniform};
use std::sync::{mpsc, RwLock};
use std::time::{Instant, SystemTime};
use orbit_types::CapturedFrame;
use glium::glutin::event::{DeviceEvent, VirtualKeyCode};
use glium::texture::{ClientFormat, RawImage2d};
use std::borrow::Cow;
use std::collections::hash_map::Entry;
use glium::backend::glutin::glutin::dpi::LogicalSize;
use glium::backend::glutin::glutin::window::Window;
use crate::{INITIAL_WINDOW_WIDTH, INITIAL_WINDOW_HEIGHT};
use glium::glutin::dpi::PhysicalSize;

#[derive(Eq, PartialEq, Hash, Copy, Clone, Debug, Ord, PartialOrd)]
pub struct StreamId {
    socket_addr: SocketAddr,
    device_id: DeviceId,
}

pub fn network(addrs: &[SocketAddr]) {
    // Setup the GL display stuff
    let event_loop = glutin::event_loop::EventLoop::new();
    let wb = glutin::window::WindowBuilder::new()
        .with_inner_size(LogicalSize::new(INITIAL_WINDOW_WIDTH as f32, INITIAL_WINDOW_HEIGHT as f32))
        .with_title("Orbit Station");
    let cb = glutin::ContextBuilder::new().with_vsync(true);
    let display = glium::Display::new(wb, cb, &event_loop).unwrap();

    // building the vertex buffer, which contains all the vertices that we will draw
    let vertex_buffer = {
        #[derive(Copy, Clone)]
        struct Vertex { position: [f32; 2], tex_coords: [f32; 2] }

        implement_vertex!(Vertex, position, tex_coords);

        glium::VertexBuffer::new(&display, &[
            Vertex { position: [-1.0, -1.0], tex_coords: [0.0, 0.0] },
            Vertex { position: [-1.0, 1.0], tex_coords: [0.0, 1.0] },
            Vertex { position: [1.0, 1.0], tex_coords: [1.0, 1.0] },
            Vertex { position: [1.0, -1.0], tex_coords: [1.0, 0.0] },
        ]).unwrap()
    };

    // building the index buffer
    let index_buffer = glium::IndexBuffer::new(
        &display,
        PrimitiveType::TriangleStrip,
        &[1 as u16, 2, 0, 3]
    ).unwrap();

    let program = program!(&display,
        140 => {
            vertex: "
                #version 140
                in vec2 position;
                in vec2 tex_coords;
                out vec2 v_tex_coords;
                void main() {
                    gl_Position = vec4(position, 0.0, 1.0);
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

    let (message_sender, message_receiver) = mpsc::channel();
    for &socket_addr in addrs {
        image_loop(socket_addr, message_sender.clone());
    }

    let mut streams = Streams::new(INITIAL_WINDOW_WIDTH, INITIAL_WINDOW_HEIGHT);

    event_loop.run(move |event, _, control_flow| {
        match event {
            glutin::event::Event::WindowEvent { event, .. } => match event {
                glutin::event::WindowEvent::CloseRequested => {
                    *control_flow = glutin::event_loop::ControlFlow::Exit;
                },
                glutin::event::WindowEvent::Resized(new_size) => {
                    let PhysicalSize { width, height } = new_size;
                    streams.resize(width, height);
                },
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
            },
            glutin::event::Event::RedrawEventsCleared => {
                while let Ok(message) = message_receiver.try_recv() {
                    match message {
                        Message::StreamDeregistered(stream_id) => streams.deregister_stream(&stream_id),
                        Message::NewImage(stream_id, image) => streams.register_frame(stream_id, image),
                    }
                }

                let mut target = display.draw();
                target.clear_color(0.0, 0.0, 0.0, 0.0);

                for (viewport, image) in streams.frames() {
                    let opengl_texture = glium::texture::Texture2d::new(&display, image).unwrap();
                    let uniforms = uniform! { tex: &opengl_texture };

                    let mut draw_parameters: DrawParameters = Default::default();
                    draw_parameters.viewport = Some(viewport);

                    target.draw(
                        &vertex_buffer,
                        &index_buffer,
                        &program,
                        &uniforms,
                        &draw_parameters,
                    ).unwrap();
                }

                target.finish().unwrap();
            },
            glutin::event::Event::RedrawRequested(_) => {
            },
            _ => {}
        }
    });
}


enum Message {
    StreamDeregistered(StreamId),
    NewImage(StreamId, RgbImage),
}

fn image_loop(
    socket_addr: SocketAddr,
    message_sender: Sender<Message>,
) {
    thread::spawn(move || {
        stream(socket_addr, message_sender).unwrap();
    });
}

fn stream(
    socket_addr: SocketAddr,
    message_sender: Sender<Message>,
) -> io::Result<()> {
    let mut connection = TcpStream::connect(socket_addr)?;

    bincode::serialize_into(
        &mut connection,
        &Request::Stream,
    ).unwrap();

    let mut connection = BufReader::new(connection);

    loop {
        let response: StreamResponse = bincode::deserialize_from(&mut connection).unwrap(); // TODO: remove unwrap

        match response {
            StreamResponse::Stop(device_id) => {
                let stream_id = StreamId { socket_addr, device_id };
                message_sender.send(Message::StreamDeregistered(stream_id)).unwrap();
            },
            StreamResponse::Frame(frame) => {
                let stream_id = StreamId { socket_addr, device_id: frame.device_id() };

                let image = image::load_from_memory_with_format(
                    frame.frame_data(),
                    ImageFormat::Jpeg,
                );

                if let Ok(image) = image {
                    let image = image.into_rgb8();
                    message_sender.send(Message::NewImage(stream_id, image)).unwrap();
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

    let mut connection = BufReader::new(connection);

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

pub struct Streams {
    layout: Layout,
    stream_ids: Vec<StreamId>,
    textures: HashMap<StreamId, RgbImage>,
}

impl Streams {
    fn new(window_width: u32, window_height: u32) -> Streams {
        Streams {
            layout: Layout::new(window_width, window_height, 0),
            stream_ids: Vec::new(),
            textures: HashMap::new(),
        }
    }

    fn deregister_stream(&mut self, stream_id: &StreamId) {
        if let Ok(place) = self.stream_ids.binary_search(&stream_id) {
            self.stream_ids.remove(place);
            self.textures.remove(stream_id);
        }
    }

    fn resize(&mut self, new_width: u32, new_height: u32) {
        self.layout = Layout::new(new_width, new_height, self.stream_ids.len() as u32);
    }

    fn register_frame(&mut self, stream_id: StreamId, image: RgbImage) {
        if let Err(place) = self.stream_ids.binary_search(&stream_id) {
            // new stream
            self.stream_ids.insert(place, stream_id);
            self.layout.update_tile_count(self.stream_ids.len() as u32);
        }

        self.textures.insert(stream_id, image);
    }

    fn frames<'a>(&'a self) -> impl Iterator<Item=(Rect, RawImage2d<u8>)> + 'a {
        self.stream_ids.iter().zip(0..)
            .map(move |(stream_id, tile_index)| {
                let viewport_rect = self.layout.viewport_rect(tile_index);
                let image = &self.textures[stream_id];
                let image = RawImage2d {
                    width: image.width(),
                    height: image.height(),
                    data: Cow::Borrowed(image.as_raw()),
                    format: ClientFormat::U8U8U8,
                };
                (viewport_rect, image)
            })
    }
}

pub struct Layout {
    window_width: u32,
    window_height: u32,
    tile_count: u32,
    horizontal_tile_count: u32,
    tile_width: u32,
    tile_height: u32,
}

impl Layout {
    fn new(window_width: u32, window_height: u32, tile_count: u32) -> Layout {
        fn horizontal_tile_count(window_width: u32, window_height: u32, tile_count: u32) -> u32 {
            fn integer_square_root(x: u32) -> u32 {
                (x as f32).sqrt() as u32
            }

            let horizontal_tile_count = 9*tile_count*window_width/(window_height*16);
            1 + integer_square_root(horizontal_tile_count)
        }

        // let horizontal_tile_count = (1..)
        //     .find(|&horizontal_tile_count| {
        //         let required_rows = ceiling_div(tile_count, horizontal_tile_count);
        //         let available_rows = horizontal_tile_count * window_height * 16 / (window_width * 9);
        //         if required_rows <= available_rows {
        //
        //             dbg!(required_rows, available_rows);
        //             true
        //         } else {
        //             false
        //         }
        //
        //     })
        //     .unwrap();

        let (w, horizontal_tile_count) = (1..window_width)
            .map(|window_width| {
                let horizontal_tile_count = horizontal_tile_count(window_width, window_height, tile_count);
                (window_width, horizontal_tile_count)
            })
            .filter(|&(window_width, horizontal_tile_count)| {
                let required_rows = ceiling_div(tile_count, horizontal_tile_count);
                let available_rows = horizontal_tile_count * window_height * 16 / (window_width * 9);
                required_rows <= available_rows
            })
            .max_by_key(|&(window_width, horizontal_tile_count)| window_width/horizontal_tile_count)
            .unwrap();

        let tile_width = w/horizontal_tile_count;
        let tile_height = tile_width * 9 / 16;

        Layout {
            window_width,
            window_height,
            tile_count,
            horizontal_tile_count,
            tile_width,
            tile_height
        }
    }

    fn update_tile_count(&mut self, new_tile_count: u32) {
        // lets see if we can keep the same format, and just increment tile_count
        *self = Layout::new(self.window_width, self.window_height, new_tile_count);
    }

    fn tile_count(&self) -> u32 {
        self.tile_count
    }

    fn viewport_rect(&self, i: u32) -> Rect {
        let tile_x = self.tile_width * (i % self.horizontal_tile_count);
        let tile_y = self.tile_height * (i / self.horizontal_tile_count);
        let tile_y = self.window_height - self.tile_height - tile_y;

        Rect {
            left: tile_x,
            bottom: tile_y,
            width: self.tile_width,
            height: self.tile_height,
        }
    }
}

fn ceiling_div(a: u32, b: u32) -> u32 {
    a/b + if a%b!=0 { 1 } else { 0 }
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
