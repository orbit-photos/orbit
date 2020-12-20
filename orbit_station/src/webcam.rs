use std::{thread, fs};
use image::{ImageFormat, RgbImage};
use crate::picture::{rotation_matrix};
use std::net::{TcpStream, SocketAddr};
use std::io::{self, BufReader};
use std::collections::HashMap;
use std::sync::mpsc::{Sender, Receiver};
use orbit_types::{DeviceId, Request, StreamResponse, SnapResponse};
use chrono::Utc;
use glium::{glutin, Surface, DrawParameters, Rect, Display, VertexBuffer, IndexBuffer, Program, implement_vertex,uniform};
use std::sync::{Arc};
use orbit_types::CapturedFrame;
use glium::glutin::event::{DeviceEvent, VirtualKeyCode, ElementState, Event};
use glium::texture::{ClientFormat, RawImage2d};
use std::borrow::Cow;
use crate::{STILL_CAPTURE_DELAY_MILLIS};
use glium::glutin::dpi::PhysicalSize;
use std::sync::atomic::{Ordering, AtomicU32};
use glium::glutin::event_loop::{EventLoopWindowTarget, ControlFlow};

#[derive(Eq, PartialEq, Hash, Copy, Clone, Debug, Ord, PartialOrd)]
pub struct StreamId {
    socket_addr: SocketAddr,
    device_id: DeviceId,
}

#[derive(Copy, Clone)]
pub struct Vertex { pub position: [f32; 2], pub tex_coords: [f32; 2] }

implement_vertex!(Vertex, position, tex_coords);


pub fn event_handler(
    event: Event<'_, ()>,
    _target: &EventLoopWindowTarget<()>,
    control_flow: &mut ControlFlow,

    vertex_buffer: &VertexBuffer<Vertex>,
    index_buffer: &IndexBuffer<u16>,
    program: &Program,
    display: &Display,

    streams: &mut Streams,
    pictures_taken: &Arc<AtomicU32>,
    message_receiver: &Receiver<Message>,
) {
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
            if input.state != ElementState::Pressed { return }

            match input.virtual_keycode {
                Some(VirtualKeyCode::S) => {
                    println!("requested calibration");
                },
                Some(VirtualKeyCode::Space) => {
                    println!("requested picture");
                    pictures_taken.fetch_add(1, Ordering::SeqCst);
                },
                _ => {},
            }
        },
        glutin::event::Event::RedrawEventsCleared => {
            while let Ok(message) = message_receiver.try_recv() {
                match message {
                    Message::StreamDeregistered(stream_id) => streams.deregister_stream(&stream_id),
                    Message::NewImage(stream_id, image) => streams.register_frame(stream_id, image),
                    Message::Stills(devices) => {
                        for (addr, stills) in devices {
                            for still in stills {
                                println!("received a still from {} {:?} at {}", addr, still.device_id(), still.captured_at());
                                fs::write(format!("orbit_station/outputs/{:?}.jpg", still.device_id()), still.frame_data()).unwrap();
                            }
                        }
                    },
                }
            }

            let mut target = display.draw();
            target.clear_color(0.0, 0.0, 0.0, 0.0);

            for (viewport, image) in streams.frames() {
                let opengl_texture = glium::texture::Texture2d::new(display, image).unwrap();

                // theta += 0.002;

                let [m0, m1, m2, m3] = rotation_matrix(0.0);

                let uniforms = uniform! {
                        tex: &opengl_texture,
                        rot: [
                            [m0, m1],
                            [m2, m3],
                        ],
                        shift1: [-0.5, -0.5f32],
                        shift2: [0.5, 0.5f32],
                    };

                let mut draw_parameters: DrawParameters = Default::default();
                draw_parameters.viewport = Some(viewport);

                target.draw(
                    vertex_buffer,
                    index_buffer,
                    program,
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
}

pub enum Message {
    StreamDeregistered(StreamId),
    NewImage(StreamId, RgbImage),
    Stills(Vec<(SocketAddr, Vec<CapturedFrame>)>),
}

pub fn capture_loop(addrs: Vec<SocketAddr>, message_sender: Sender<Message>, pictures_taken: Arc<AtomicU32>) {
    thread::spawn(move || {
        loop {
            // streaming mode
            let stream_handles: Vec<_> = addrs.iter()
                .map(|&socket_addr| {
                    let message_sender = message_sender.clone();
                    let pictures_taken = Arc::clone(&pictures_taken);
                    thread::spawn(move || stream(socket_addr, &message_sender, pictures_taken))
                })
                .collect();

            for handle in stream_handles {
                handle.join().unwrap().unwrap(); // TODO: handle properly
            }

            // still frame mode
            let shutter_handles: Vec<_> = addrs.iter()
                .map(|&socket_addr| thread::spawn(move || shutter(socket_addr)))
                .collect();

            let mut stills = Vec::new();
            for (handle, &socket_addr) in shutter_handles.into_iter().zip(addrs.iter()) {
                if let Ok(snap_response) = handle.join().unwrap() {
                    stills.push((socket_addr, snap_response.stills));
                }
            }
            message_sender.send(Message::Stills(stills)).unwrap();
        }
    });
}

fn stream(
    socket_addr: SocketAddr,
    message_sender: &Sender<Message>,
    pictures_taken: Arc<AtomicU32>,
) -> io::Result<()> {
    let mut connection = TcpStream::connect(socket_addr)?;

    bincode::serialize_into(
        &mut connection,
        &Request::Stream,
    ).unwrap();

    let mut connection = BufReader::new(connection);

    let pictures_taken_start = pictures_taken.load(Ordering::Relaxed);

    loop {
        let pictures_taken_current = pictures_taken.load(Ordering::Relaxed);
        if pictures_taken_start < pictures_taken_current { break Ok(()) }

        let response = StreamResponse::deserialize_from(&mut connection).unwrap();

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
) -> io::Result<SnapResponse> {

    let mut connection = TcpStream::connect(socket_addr)?;


    bincode::serialize_into(
        &mut connection,
        &Request::Snap(Utc::now() + chrono::Duration::milliseconds(STILL_CAPTURE_DELAY_MILLIS)),
    ).unwrap();

    let mut connection = BufReader::new(connection);

    let snap_response: SnapResponse = bincode::deserialize_from(&mut connection).unwrap();

    Ok(snap_response)
}

pub struct Streams {
    layout: Layout,
    stream_ids: Vec<StreamId>,
    textures: HashMap<StreamId, RgbImage>,
}

impl Streams {
    pub fn new(window_width: u32, window_height: u32) -> Streams {
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
        let (w, horizontal_tile_count) = (1..=window_width)
            .map(|window_width| (
                window_width,
                1+integer_square_root(9*tile_count*window_width/(window_height*16)),
            ))
            .filter(|&(window_width, horizontal_tile_count)| {
                let required_rows = ceiling_div(tile_count, horizontal_tile_count);
                let available_rows = horizontal_tile_count * window_height * 16 / (window_width * 9);
                required_rows <= available_rows
            })
            .max_by_key(|&(window_width, horizontal_tile_count)| window_width/horizontal_tile_count)
            .unwrap();

        let tile_width = w / horizontal_tile_count;
        let tile_height = tile_width * 9 / 16;

        Layout {
            window_width,
            window_height,
            tile_count,
            horizontal_tile_count,
            tile_width,
            tile_height,
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

fn integer_square_root(n: u32) -> u32 {
    if n == 0 { return 0 }

    let mut x = n;

    loop {
        let x_prev = x;
        x = (x + n/x) / 2;

        if x_prev == x || x_prev + 1 == x {
            break x_prev;
        }
    }
}

fn ceiling_div(a: u32, b: u32) -> u32 {
    a/b + (a%b != 0) as u32
}
