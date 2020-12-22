use std::{thread, fs};
use image::{ImageFormat, RgbImage};
use crate::picture::{rotation_matrix, crop_rotate_scale};
use std::net::{TcpStream, SocketAddr};
use std::io::{self, BufReader};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Sender, Receiver};
use orbit_types::{DeviceId, Request, StreamResponse, SnapResponse};
use chrono::Utc;
use glium::{glutin, Surface, DrawParameters, Rect, Display, VertexBuffer, IndexBuffer, Program, implement_vertex,uniform};
use std::sync::{Arc};
use orbit_types::CapturedFrame;
use glium::glutin::event::{DeviceEvent, VirtualKeyCode, ElementState, Event};
use glium::texture::{ClientFormat, RawImage2d};
use std::borrow::Cow;
use crate::{STILL_CAPTURE_DELAY_MILLIS, TAG_SIZE_METERS, STREAM_ASPECT_HEIGHT, STREAM_ASPECT_WIDTH, FOCAL_LENGTH_PIXELS};
use glium::glutin::dpi::PhysicalSize;
use std::sync::atomic::{Ordering, AtomicU32};
use glium::glutin::event_loop::{EventLoopWindowTarget, ControlFlow};
use crate::calibrate::Averager;
use apriltag::{EulerAngles, ApriltagDetector};

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

    for_calibration: &mut HashSet<u32>,
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
                Some(VirtualKeyCode::Return) => {
                    let start = pictures_taken.fetch_add(1, Ordering::SeqCst);
                    for_calibration.insert(start);
                    println!("requested calibration");
                },
                Some(VirtualKeyCode::Space) => {
                    pictures_taken.fetch_add(1, Ordering::SeqCst);
                    println!("requested picture");
                },
                _ => {},
            }
        },
        glutin::event::Event::RedrawEventsCleared => {
            while let Ok(message) = message_receiver.try_recv() {
                match message {
                    Message::StreamDeregistered(stream_id) => streams.deregister_stream(&stream_id),
                    Message::NewImage(stream_id, image) => streams.register_frame(stream_id, image),
                    Message::Stills(pictures_taken_start, devices) => {
                        if for_calibration.contains(&pictures_taken_start) {
                            // just trying to calibrate, don't want to save the images
                            println!("new calibration event");
                            streams.register_calibration_event(&devices);
                        } else {
                            // regular old picture taking
                            for (addr, stills) in devices {
                                for still in stills {
                                    println!("received a still from {} {:?} at {}", addr, still.device_id(), still.captured_at());
                                    fs::write(format!("orbit_station/outputs/{:?}{:?}.jpg", addr, still.device_id()), still.frame_data()).unwrap();
                                }
                            }
                        }
                    },
                }
            }

            let mut target = display.draw();
            target.clear_color(0.0, 0.0, 0.0, 0.0);

            for (viewport, adjustment, image) in streams.frames() {
                let opengl_texture = glium::texture::Texture2d::new(display, image).unwrap();

                let [m0, m1, m2, m3] = rotation_matrix(adjustment.roll as f32);

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
    Stills(u32, Vec<(SocketAddr, Vec<CapturedFrame>)>),
}

pub fn capture_loop(addrs: Vec<SocketAddr>, message_sender: Sender<Message>, pictures_taken: Arc<AtomicU32>) {
    thread::spawn(move || {
        loop {
            // streaming mode
            let pictures_taken_start = pictures_taken.load(Ordering::Relaxed);

            let stream_handles: Vec<_> = addrs.iter()
                .map(|&socket_addr| {
                    let message_sender = message_sender.clone();
                    let pictures_taken = Arc::clone(&pictures_taken);
                    thread::spawn(move || stream(socket_addr, &message_sender, pictures_taken_start, pictures_taken))
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
            message_sender.send(Message::Stills(pictures_taken_start, stills)).unwrap();
        }
    });
}

fn stream(
    socket_addr: SocketAddr,
    message_sender: &Sender<Message>,
    pictures_taken_start: u32,
    pictures_taken: Arc<AtomicU32>,
) -> io::Result<()> {
    let mut connection = TcpStream::connect(socket_addr)?;

    bincode::serialize_into(
        &mut connection,
        &Request::Stream,
    ).unwrap();

    let mut connection = BufReader::new(connection);

    loop {
        if pictures_taken_start < pictures_taken.load(Ordering::Relaxed) { break Ok(()) }

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

fn shutter(socket_addr: SocketAddr) -> io::Result<SnapResponse> {

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
    apriltag_detector: ApriltagDetector,
    calibration_events: CalibrationEvents,
    textures: HashMap<StreamId, RgbImage>,
}

impl Streams {
    pub fn new(window_width: u32, window_height: u32) -> Streams {
        Streams {
            calibration_events: CalibrationEvents::new(),
            apriltag_detector: ApriltagDetector::new(),
            layout: Layout::new(window_width, window_height, 0),
            stream_ids: Vec::new(),
            textures: HashMap::new(),
        }
    }

    fn register_calibration_event(&mut self, devices: &[(SocketAddr, Vec<CapturedFrame>)]) {
        let event = CalibrationEvent::new(devices, &mut self.apriltag_detector);
        self.calibration_events.add_event(event);
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
            self.layout = Layout::new(self.layout.window_width, self.layout.window_height, self.stream_ids.len() as u32);
        }

        self.textures.insert(stream_id, image);
    }

    fn frames<'a>(&'a self) -> impl Iterator<Item=(Rect, Adjustment, RawImage2d<'a, u8>)> + 'a {
        self.stream_ids.iter().zip(0..)
            .map(move |(&stream_id, tile_index)| {
                let viewport = self.layout.viewport_rect(tile_index);
                let adjustment = self.calibration_events.get_adjustment_for(stream_id);
                let image = &self.textures[&stream_id];
                let image = RawImage2d {
                    width: image.width(),
                    height: image.height(),
                    data: Cow::Borrowed(image.as_raw()),
                    format: ClientFormat::U8U8U8,
                };
                (viewport, adjustment, image)
            })
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Adjustment {
    roll: f64,
    pitch: f64,
    crop: f64,
}

impl Adjustment {
    const NO_TRANSFORMATION: Adjustment = Adjustment {
        roll: 0.0,
        pitch: 0.0,
        crop: 1.0,
    };
}


struct CalibrationEvents {
    events: Vec<CalibrationEvent>,
    measured_streams: HashSet<StreamId>,
    adjustments: HashMap<StreamId, Adjustment>,
}

impl CalibrationEvents {
    fn new() -> CalibrationEvents {
        CalibrationEvents {
            events: Vec::new(),
            measured_streams: HashSet::new(),
            adjustments: HashMap::new(),
        }
    }

    fn add_event(&mut self, event: CalibrationEvent) {
        self.measured_streams.extend(event.includes_streams.keys().copied());
        self.events.push(event);

        let mut min_crop = 1.0f64;
        let mut adjustments = Vec::with_capacity(self.measured_streams.len());
        for &stream in self.measured_streams.iter() {
            let (pitch, roll) = adjustment(stream, &self.events);
            let crop = crop_rotate_scale(STREAM_ASPECT_HEIGHT as f64/STREAM_ASPECT_WIDTH as f64, roll);

            min_crop = min_crop.min(crop);
            adjustments.push((stream, pitch, roll));
        }

        self.adjustments.clear();
        for (stream, pitch, roll) in adjustments {
            self.adjustments.insert(stream, Adjustment { roll, pitch, crop: min_crop });
        }
    }

    fn get_adjustment_for(&self, stream: StreamId) -> Adjustment {
        *self.adjustments.get(&stream).unwrap_or(&Adjustment::NO_TRANSFORMATION)
    }
}

fn adjustment(stream: StreamId, events: &[CalibrationEvent]) -> (f64, f64) {
    // take a weighted average of the adjustments over all of the samples that include uur
    // stream

    let mut roll_numerator = 0.0;
    let mut pitch_numerator = 0.0;
    let mut total_samples = 0.0;

    for event in events {
        if let Some(euler_angles) = event.includes_streams.get(&stream) {
            let measurement_count = event.includes_streams.len() as f64;
            roll_numerator += measurement_count*(euler_angles.roll - event.average_roll.read());
            pitch_numerator += measurement_count*(euler_angles.pitch - event.average_pitch.read());
            total_samples += measurement_count;
        }
    }

    if total_samples == 0.0 {
        (0.0, 0.0)
    } else {
        (roll_numerator/total_samples, pitch_numerator/total_samples)
    }
}



struct CalibrationEvent {
    includes_streams: HashMap<StreamId, EulerAngles>,
    average_pitch: Averager,
    average_roll: Averager,
}

impl CalibrationEvent {
    fn new(devices: &[(SocketAddr, Vec<CapturedFrame>)], apriltag_detector: &mut ApriltagDetector) -> CalibrationEvent {
        let mut average_pitch = Averager::new();
        let mut average_roll = Averager::new();
        let mut includes_streams = HashMap::new();

        for &(socket_addr, ref stills) in devices {
            for still in stills {
                let stream_id = StreamId { socket_addr, device_id: still.device_id() };

                let image = image::load_from_memory_with_format(
                    still.frame_data(),
                    ImageFormat::Jpeg,
                );

                if let Ok(image) = image {
                    let image = image.into_luma8();

                    let detection = apriltag_detector.search(
                        image.as_raw(),
                        image.width(),
                        image.height(),
                        TAG_SIZE_METERS,
                        FOCAL_LENGTH_PIXELS,
                    );

                    match detection {
                        Ok(detection) => {
                            let euler_angles = detection.euler_angles();

                            average_pitch.add(euler_angles.pitch);
                            average_roll.add(euler_angles.roll);
                            includes_streams.insert(stream_id, euler_angles);
                        },
                        Err(_) => {}, // TODO: display the ones that fail on the screen
                    }
                }
            }
        }

        CalibrationEvent { includes_streams, average_pitch, average_roll }
    }
}


// #[derive(Eq, PartialEq, Hash)]
// pub struct SamplingEventId(u32);
// pub struct SamplingEventIdGenerator(u32);
// impl SamplingEventIdGenerator {
//     fn new() -> SamplingEventIdGenerator {
//         SamplingEventIdGenerator(0)
//     }
//
//     fn next(&mut self) -> SamplingEventId {
//         self.0 += 1;
//         SamplingEventId(self.0)
//     }
// }


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
                1+integer_square_root(STREAM_ASPECT_HEIGHT*tile_count*window_width/(window_height*STREAM_ASPECT_WIDTH)),
            ))
            .filter(|&(window_width, horizontal_tile_count)| {
                let required_rows = ceiling_div(tile_count, horizontal_tile_count);
                let available_rows = horizontal_tile_count*window_height*STREAM_ASPECT_WIDTH/(window_width*STREAM_ASPECT_HEIGHT);
                required_rows <= available_rows
            })
            .max_by_key(|&(window_width, horizontal_tile_count)| window_width/horizontal_tile_count)
            .unwrap();

        let tile_width = w/horizontal_tile_count;
        let tile_height = tile_width*STREAM_ASPECT_HEIGHT/STREAM_ASPECT_WIDTH;

        Layout {
            window_width,
            window_height,
            tile_count,
            horizontal_tile_count,
            tile_width,
            tile_height,
        }
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
    (n as f64).sqrt() as u32
}


fn ceiling_div(a: u32, b: u32) -> u32 {
    a/b + (a%b != 0) as u32
}
