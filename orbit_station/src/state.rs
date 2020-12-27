use std::{fs};
use image::{ImageFormat, RgbImage};
use crate::picture::{rotation_matrix, crop_rotate_scale};
use std::net::{SocketAddr};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver};
use orbit_types::{DeviceId};
use glium::{program, glutin, Surface, DrawParameters, Rect, Display, VertexBuffer, IndexBuffer, Program, implement_vertex,uniform};
use std::sync::{Arc};
use orbit_types::CapturedFrame;
use glium::glutin::event::{VirtualKeyCode, ElementState, Event, MouseButton, KeyboardInput, WindowEvent};
use glium::texture::{ClientFormat, RawImage2d};
use std::borrow::Cow;
use crate::{TAG_SIZE_METERS, STREAM_ASPECT_HEIGHT, STREAM_ASPECT_WIDTH, FOCAL_LENGTH_PIXELS, INITIAL_WINDOW_WIDTH, INITIAL_WINDOW_HEIGHT};
use glium::glutin::dpi::{PhysicalSize, LogicalSize};
use std::sync::atomic::{Ordering, AtomicU32};
use glium::glutin::event_loop::{ControlFlow};
use apriltag::{EulerAngles, ApriltagDetector, TagFamily};
use std::f64::consts::TAU;
use glium::backend::glutin::glutin::dpi::PhysicalPosition;
use glium::index::PrimitiveType;
use glium::backend::glutin::glutin::event_loop::EventLoop;
use crate::frame_receiver::Message;
use chrono::{Local};
use std::path::PathBuf;


pub struct State {
    layout: Layout,
    stream_ids: StreamIds,
    apriltag_detector: ApriltagDetector,
    calibration_events: CalibrationEvents,
    cardinal_rotations: HashMap<StreamId, CardinalRotation>,
    textures: HashMap<StreamId, RgbImage>,
    selected: Option<(StreamOrdinal, f64, f64)>,
    pictures_taken: Arc<AtomicU32>,

    for_calibration: HashSet<u32>,
    cursor_position: PhysicalPosition<f64>,

    display: Display,

    panel_vertex_buffer: VertexBuffer<PanelVertex>,
    panel_index_buffer: IndexBuffer<u16>,
    panel_shaders: Program,

    selection_box_vertex_buffer: VertexBuffer<SelectionBoxVertex>,
    selection_box_index_buffer: IndexBuffer<u16>,
    selection_box_shaders: Program,
}

impl State {
    pub fn new(window_width: u32, window_height: u32, event_loop: &EventLoop<()>, pictures_taken: Arc<AtomicU32>) -> State {
        let wb = glutin::window::WindowBuilder::new()
            .with_inner_size(LogicalSize::new(INITIAL_WINDOW_WIDTH as f32, INITIAL_WINDOW_HEIGHT as f32))
            .with_title("Orbit Station");

        let display = glium::Display::new(
            wb,
            glutin::ContextBuilder::new().with_vsync(true),
            &event_loop
        ).unwrap();

        let panel_vertex_buffer = glium::VertexBuffer::new(&display, &[
            PanelVertex { position: [-1.0, -1.0], tex_coords: [0.0, 0.0] },
            PanelVertex { position: [-1.0, 1.0], tex_coords: [0.0, 1.0] },
            PanelVertex { position: [1.0, 1.0], tex_coords: [1.0, 1.0] },
            PanelVertex { position: [1.0, -1.0], tex_coords: [1.0, 0.0] },
        ]).unwrap();

        let panel_index_buffer = glium::IndexBuffer::new(
            &display,
            PrimitiveType::TriangleStrip,
            &[1 as u16, 2, 0, 3]
        ).unwrap();

        let panel_shaders = program!(&display,
            140 => {
                vertex: include_str!("shaders/panel_vertex.glsl"),
                fragment: include_str!("shaders/panel_fragment.glsl"),
            },
        ).unwrap();

        let selection_box_vertex_buffer = glium::VertexBuffer::new(&display, &[
            SelectionBoxVertex { position: [-1.0, -1.0] },
            SelectionBoxVertex { position: [-1.0, 1.0] },
            SelectionBoxVertex { position: [1.0, 1.0] },
            SelectionBoxVertex { position: [1.0, -1.0] },
        ]).unwrap();

        let selection_box_index_buffer = glium::IndexBuffer::new(
            &display,
            PrimitiveType::LineLoop,
            &[0u16, 1, 2, 3],
        ).unwrap();

        let selection_box_shaders = program!(&display,
            140 => {
                vertex: include_str!("shaders/selection_box_vertex.glsl"),
                fragment: include_str!("shaders/selection_box_fragment.glsl"),
            },
        ).unwrap();

        State {
            calibration_events: CalibrationEvents::new(),
            apriltag_detector: ApriltagDetector::new(TagFamily::Tag16h5),
            cardinal_rotations: HashMap::new(),
            layout: Layout::new(window_width, window_height, 0),
            stream_ids: StreamIds::new(),
            textures: HashMap::new(),
            selected: None,

            pictures_taken,
            for_calibration: HashSet::new(),
            cursor_position: PhysicalPosition::new(0.0, 0.0),

            display,
            panel_vertex_buffer,
            panel_index_buffer,
            panel_shaders,
            selection_box_vertex_buffer,
            selection_box_index_buffer,
            selection_box_shaders,
        }
    }
    
    pub fn event_handler(&mut self,
         event: Event<'_, ()>,
         control_flow: &mut ControlFlow,
         message_receiver: &Receiver<Message>,
    ) {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                },
                WindowEvent::Resized(new_size) => {
                    let PhysicalSize { width, height } = new_size;
                    self.resize(width, height);
                },
                WindowEvent::CursorMoved { position, .. } => {
                    self.update_cursor_position(position);
                },
                WindowEvent::MouseInput { button: MouseButton::Left, state: ElementState::Pressed, .. } => {
                    self.select();
                },
                WindowEvent::MouseInput { button: MouseButton::Left, state: ElementState::Released, .. } => {
                    self.unselect();
                },
                WindowEvent::MouseInput { button: MouseButton::Right, state: ElementState::Pressed, .. } => {
                    self.cardinal_rotation();
                },
                WindowEvent::KeyboardInput { input, .. } => match input {
                    KeyboardInput { state: ElementState::Pressed, virtual_keycode: Some(VirtualKeyCode::Space), .. } => {
                        self.pictures_taken.fetch_add(1, Ordering::SeqCst);
                        println!("requested picture");
                    },
                    KeyboardInput { state: ElementState::Pressed, virtual_keycode: Some(VirtualKeyCode::Return), .. } => {
                        let start = self.pictures_taken.fetch_add(1, Ordering::SeqCst);
                        self.make_for_calibration(start);
                        println!("requested calibration");
                    },
                    _ => {},
                }
                _ => {}
            },
            Event::RedrawEventsCleared => {
                while let Ok(message) = message_receiver.try_recv() {
                    self.message_handler(message);
                }
                self.draw();
            },
            _ => {}
        }
    }

    fn draw(&self) {
        let crop_factor = self.calibration_events.crop_factor as f32;

        let (last, rest) = self.frames();

        let mut target = self.display.draw();
        target.clear_color(0.0, 0.0, 0.0, 0.0);

        for panel in rest {
            panel.display(crop_factor, &self.display, &mut target, &self.panel_vertex_buffer, &self.panel_index_buffer, &self.panel_shaders);
        }

        if let Some(last) = last {
            last.display(crop_factor, &self.display, &mut target, &self.panel_vertex_buffer, &self.panel_index_buffer, &self.panel_shaders);
        }

        if self.selected.is_some() {
            if let Some(tile) = self.layout.get_cursor_tile(self.cursor_position.x as u32, self.cursor_position.y as u32, &self.stream_ids) {
                draw_box(&mut target, tile, &self.layout, &self.selection_box_vertex_buffer, &self.selection_box_index_buffer, &self.selection_box_shaders);
            }
        }

        target.finish().unwrap();
    }

    fn message_handler(&mut self, message: Message) {
        match message {
            Message::StreamDeregistered(stream_id) => self.deregister_stream(stream_id),
            Message::NewImage(stream_id, image) => self.register_frame(stream_id, image),
            Message::Stills(pictures_taken_start, devices) if self.is_for_calibration(pictures_taken_start) => {
                println!("new calibration event");
                self.register_calibration_event(&devices);
            },
            Message::Stills(_, devices) => {
                let mut devices: Vec<_> = devices.into_iter()
                    .map(|(addr, stills)|
                        stills.into_iter().map(move |still| {
                            let stream_id = StreamId::new(addr, still.device_id());
                            (stream_id, still)
                        }))
                    .flatten()
                    .collect();

                devices.sort_by_key(|&(s, _)| self.stream_ids.get_stream_id(s));


                let dir = PathBuf::from(format!("outputs/{}", Local::now()));
                fs::create_dir(&dir).unwrap();
                for ((_addr, still), i) in devices.into_iter().zip(1..) {
                    fs::write(
                        dir.join(format!("frame{:02}.jpg", i)),
                        still.frame_data()
                    ).unwrap();
                }

                println!("saved images");
            },
        }
    }

    fn make_for_calibration(&mut self, pictures_taken: u32) {
        self.for_calibration.insert(pictures_taken);
    }
    
    fn is_for_calibration(&self, pictures_taken: u32) -> bool {
        self.for_calibration.contains(&pictures_taken)
    }
    
    fn update_cursor_position(&mut self, new_position: PhysicalPosition<f64>) {
        self.cursor_position = new_position;
    }
    
    fn register_calibration_event(&mut self, devices: &[(SocketAddr, Vec<CapturedFrame>)]) {
        let event = CalibrationEvent::new(devices, &mut self.apriltag_detector);
        self.calibration_events.add_event(event);
    }

    fn deregister_stream(&mut self, stream_id: StreamId) {
        self.stream_ids.remove(stream_id);
        self.textures.remove(&stream_id);
    }

    fn select(&mut self) {
        if let Some(tile) = self.layout.get_cursor_tile(self.cursor_position.x as u32, self.cursor_position.y as u32, &self.stream_ids) {
            let rect = self.layout.viewport_rect(tile);
            let cursor_distance_from_bottom = self.layout.window_height as f64 - self.cursor_position.y;
            self.selected = Some((tile, rect.left as f64-self.cursor_position.x as f64, rect.bottom as f64-cursor_distance_from_bottom));
        }
    }

    fn unselect(&mut self) {
        if let Some((old_index, _, _)) = self.selected {
            if let Some(new_index) = self.layout.get_cursor_tile(self.cursor_position.x as u32, self.cursor_position.y as u32, &self.stream_ids) {
                let stream_id = self.stream_ids.remove_by_ordinal(old_index);
                self.stream_ids.insert_at(stream_id, new_index);
            }

            self.selected = None;
        }
    }

    fn cardinal_rotation(&mut self) {
        if let Some(tile) = self.layout.get_cursor_tile(self.cursor_position.x as u32, self.cursor_position.y as u32, &self.stream_ids) {
            if let Some(stream_id) = self.stream_ids.get_stream_id_at(tile) {
                self.cardinal_rotations.entry(stream_id)
                    .or_insert(CardinalRotation::default())
                    .next();
            }
        }
    }

    fn resize(&mut self, new_width: u32, new_height: u32) {
        self.layout = Layout::new(new_width, new_height, self.stream_ids.len() as u32);
    }

    fn register_frame(&mut self, stream_id: StreamId, image: RgbImage) {
        if !self.stream_ids.contains(stream_id) {
            self.stream_ids.add(stream_id);
            self.layout = Layout::new(self.layout.window_width, self.layout.window_height, self.stream_ids.len() as u32);
        }

        self.textures.insert(stream_id, image);
    }

    fn frames<'a>(&'a self) -> (Option<PanelToDisplay<'a>>, impl Iterator<Item=PanelToDisplay<'a>> + 'a) {
        let last = self.stream_ids.iter()
            .find(|&(_, tile_index)| matches!(self.selected, Some((selected_tile, _, _)) if selected_tile == tile_index))
            .map(move |(stream_id, tile_index)| {
                let (_, dx, dy) = self.selected.unwrap();
                let mut viewport_rect = self.layout.viewport_rect(tile_index);
                viewport_rect.left = (self.cursor_position.x + dx) as u32;
                let cursor_distance_from_bottom = self.layout.window_height as f64 - self.cursor_position.y;
                viewport_rect.bottom = (dy + cursor_distance_from_bottom) as u32;

                let cardinal_rotation = self.cardinal_rotations.get(&stream_id)
                    .unwrap_or(&CardinalRotation::default())
                    .get_angle();
                let adjustment = self.calibration_events.get_adjustment_for(stream_id);
                let image = &self.textures[&stream_id];
                let image = RawImage2d {
                    width: image.width(),
                    height: image.height(),
                    data: Cow::Borrowed(image.as_raw()),
                    format: ClientFormat::U8U8U8,
                };

                PanelToDisplay { viewport_rect, adjustment, cardinal_rotation, image }
            });

        let iter = self.stream_ids.iter()
            .filter(move |&(_, tile_index)| !matches!(self.selected, Some((selected_tile, _, _)) if selected_tile == tile_index))
            .map(move |(stream_id, tile_index)| {
                let viewport_rect = self.layout.viewport_rect(tile_index);
                let cardinal_rotation = self.cardinal_rotations.get(&stream_id)
                    .unwrap_or(&CardinalRotation::default())
                    .get_angle();
                let adjustment = self.calibration_events.get_adjustment_for(stream_id);
                let image = &self.textures[&stream_id];
                let image = RawImage2d {
                    width: image.width(),
                    height: image.height(),
                    data: Cow::Borrowed(image.as_raw()),
                    format: ClientFormat::U8U8U8,
                };
                PanelToDisplay { viewport_rect, adjustment, cardinal_rotation, image }
            });

        (last, iter)
    }
}

#[derive(Eq, PartialEq, Hash, Copy, Clone, Debug, Ord, PartialOrd)]
pub struct StreamId {
    socket_addr: SocketAddr,
    device_id: DeviceId,
}

impl StreamId {
    pub fn new(socket_addr: SocketAddr, device_id: DeviceId) -> StreamId {
        StreamId { socket_addr, device_id }
    }
}

#[derive(Copy, Clone)]
struct SelectionBoxVertex {
    position: [f32; 2],
}
implement_vertex!(SelectionBoxVertex, position);

fn draw_box(
    target: &mut glium::Frame,
    tile_index: StreamOrdinal,
    layout: &Layout,
    vertex_buffer: &VertexBuffer<SelectionBoxVertex>,
    index_buffer: &IndexBuffer<u16>,
    shaders: &Program,
) {
    let uniforms = uniform! {};

    let viewport = layout.viewport_rect(tile_index);

    let draw_parameters: DrawParameters = DrawParameters {
        viewport: Some(viewport),
        line_width: Some(10.0),
        ..Default::default()
    };

    target.draw(
        vertex_buffer,
        index_buffer,
        shaders,
        &uniforms,
        &draw_parameters,
    ).unwrap();
}

#[derive(Copy, Clone)]
pub struct PanelVertex {
    position: [f32; 2],
    tex_coords: [f32; 2]
}

implement_vertex!(PanelVertex, position, tex_coords);

struct PanelToDisplay<'a> {
    viewport_rect: Rect,
    adjustment: Adjustment,
    cardinal_rotation: f64,
    image: RawImage2d<'a, u8>,
}

impl<'a> PanelToDisplay<'a> {
    fn display(
        self,
        crop_factor: f32,
        display: &Display,
        target: &mut glium::Frame,
        vertex_buffer: &VertexBuffer<PanelVertex>,
        index_buffer: &IndexBuffer<u16>,
        shaders: &Program
    ) {
        let opengl_texture = glium::texture::Texture2d::new(display, self.image).unwrap();

        let [m0, m1, m2, m3] = rotation_matrix((self.adjustment.roll+self.cardinal_rotation) as f32);

        let uniforms = uniform! {
                tex: &opengl_texture,
                rot: [
                    [m0*crop_factor, m1],
                    [m2, m3*crop_factor],
                ],
                shift1: [-0.5, -0.5f32],
                shift2: [0.5, 0.5f32],
            };

        let draw_parameters: DrawParameters = DrawParameters {
            viewport: Some(self.viewport_rect),
            ..Default::default()
        };

        target.draw(
            vertex_buffer,
            index_buffer,
            shaders,
            &uniforms,
            &draw_parameters,
        ).unwrap();
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct StreamOrdinal {
    index: usize,
}

struct StreamIds {
    stream_ids: Vec<StreamId>,
}

impl StreamIds {
    fn new() -> StreamIds {
        StreamIds {
            stream_ids: Vec::new(),
        }
    }

    fn len(&self) -> usize {
        self.stream_ids.len()
    }

    fn get_ordinal(&self, index: usize) -> Option<StreamOrdinal> {
        if index < self.stream_ids.len() {
            Some(StreamOrdinal { index })
        } else {
            None
        }
    }

    fn remove(&mut self, stream_id: StreamId) {
        if let Some(place) = self.stream_ids.iter().position(|&s| s == stream_id) {
            self.stream_ids.remove(place);
        }
    }

    fn remove_by_ordinal(&mut self, stream_ordinal: StreamOrdinal) -> StreamId {
        self.stream_ids.remove(stream_ordinal.index)
    }

    fn get_stream_id_at(&self, stream_ordinal: StreamOrdinal) -> Option<StreamId> {
        self.stream_ids.get(stream_ordinal.index).copied()
    }

    fn contains(&self, stream_id: StreamId) -> bool {
        self.stream_ids.contains(&stream_id)
    }

    fn add(&mut self, stream_id: StreamId) {
        self.stream_ids.push(stream_id);
    }

    fn iter(&self) -> impl Iterator<Item=(StreamId, StreamOrdinal)> + '_ {
        self.stream_ids.iter().copied().enumerate()
            .map(move |(inner, stream_id)| (stream_id, StreamOrdinal { index: inner }))
    }

    fn insert_at(&mut self, stream_id: StreamId, stream_ordinal: StreamOrdinal) {
        let mut place = stream_ordinal.index;
        if place > self.stream_ids.len() {
            place = self.stream_ids.len();
        }
        self.stream_ids.insert(place, stream_id);
    }

    fn is_valid(&self, stream_ordinal: StreamOrdinal) -> bool {
        stream_ordinal.index < self.stream_ids.len()
    }

    fn get_stream_id(&self, stream_id: StreamId) -> Option<StreamOrdinal> {
        let inner = self.stream_ids.iter().position(|&s| s == stream_id)?;
        Some(StreamOrdinal { index: inner })
    }
}

#[derive(Copy, Clone, Default)]
struct CardinalRotation {
    angle: f64,
}

impl CardinalRotation {
    fn next(&mut self) {
        self.angle += (TAU/4.0) % TAU;
        if self.angle < 0.0 { self.angle += TAU }
    }

    fn get_angle(self) -> f64 {
        self.angle
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Adjustment {
    roll: f64,
    pitch: f64,
}

impl Adjustment {
    const NO_TRANSFORMATION: Adjustment = Adjustment {
        roll: 0.0,
        pitch: 0.0,
    };
}


struct CalibrationEvents {
    events: Vec<CalibrationEvent>,
    measured_streams: HashSet<StreamId>,
    adjustments: HashMap<StreamId, Adjustment>,
    crop_factor: f64,
}

impl CalibrationEvents {
    fn new() -> CalibrationEvents {
        CalibrationEvents {
            events: Vec::new(),
            measured_streams: HashSet::new(),
            adjustments: HashMap::new(),
            crop_factor: 1.0,
        }
    }

    fn add_event(&mut self, event: CalibrationEvent) {
        self.measured_streams.extend(event.includes_streams.keys().copied());
        self.events.push(event);

        let mut min_crop = 1.0f64;
        for &stream in self.measured_streams.iter() {
            let (pitch, roll) = adjustment(stream, &self.events);
            let crop = crop_rotate_scale(STREAM_ASPECT_HEIGHT as f64/STREAM_ASPECT_WIDTH as f64, roll);

            min_crop = min_crop.min(crop);
            self.adjustments.insert(stream, Adjustment { roll, pitch });
        }

        self.crop_factor = min_crop;
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

                    match detection.first() {
                        Some(detection) => {
                            let euler_angles = detection.euler_angles();

                            average_pitch.add(euler_angles.pitch);
                            average_roll.add(euler_angles.roll);
                            includes_streams.insert(stream_id, euler_angles);
                        },
                        None => {}, // TODO: display the ones that fail on the screen
                    }
                }
            }
        }

        CalibrationEvent { includes_streams, average_pitch, average_roll }
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
        /// Returns the smallest value of horizontal tile count
        fn horizontal_tile_count(tile_count: u32, window_width: u32, window_height: u32) -> u32 {
            for horizontal_tile_count in 1.. {
                let required_rows = ceiling_div(tile_count, horizontal_tile_count);
                let available_rows = horizontal_tile_count*window_height*STREAM_ASPECT_WIDTH/(window_width*STREAM_ASPECT_HEIGHT);
                if required_rows <= available_rows {
                    return horizontal_tile_count;
                }
            }

            unreachable!()
        }

        let (w, horizontal_tile_count) = (1..=window_width)
            .map(|window_width| (
                window_width,
                horizontal_tile_count(tile_count, window_width, window_height),
            ))
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

    fn get_cursor_tile(&self, cursor_x: u32, cursor_y: u32, stream_ids: &StreamIds) -> Option<StreamOrdinal> {
        let tile_x = cursor_x / self.tile_width;
        let tile_y = cursor_y / self.tile_height;
        let i = tile_y*self.horizontal_tile_count + tile_x;
        stream_ids.get_ordinal(i as usize)
    }

    fn viewport_rect(&self, i: StreamOrdinal) -> Rect {
        let i = i.index as u32;
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

#[derive(Copy, Clone)]
pub struct Averager {
    sum: f64,
    measurement_count: f64,
}

impl Averager {
    pub fn new() -> Averager {
        Averager { sum: 0.0, measurement_count: 0.0 }
    }

    pub fn add(&mut self, value: f64) {
        self.sum += value;
        self.measurement_count += 1.0;
    }

    pub fn measurement_count(self) -> f64 {
        self.measurement_count
    }

    pub fn read(self) -> f64 {
        self.sum / self.measurement_count
    }

    fn merge(self, other: Averager) -> Averager {
        Averager {
            sum: self.sum + other.sum,
            measurement_count: self.measurement_count + other.measurement_count,
        }
    }
}
