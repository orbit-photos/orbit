use std::fs;
use std::collections::{HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::Receiver;
use chrono::Local;
use glium::{Display, DrawParameters, glutin, implement_vertex, IndexBuffer, program, Program, Rect, Surface, uniform, VertexBuffer};
use glium::backend::glutin::glutin::dpi::PhysicalPosition;
use glium::backend::glutin::glutin::event_loop::EventLoop;
use glium::glutin::dpi::{LogicalSize, PhysicalSize};
use glium::glutin::event::{ElementState, Event, KeyboardInput, MouseButton, VirtualKeyCode, WindowEvent};
use glium::glutin::event_loop::ControlFlow;
use glium::index::PrimitiveType;
use glium::texture::{RawImage2d};
use image::{ImageFormat, RgbImage};
use apriltag::{ApriltagDetector, TagFamily};

use crate::{INITIAL_WINDOW_HEIGHT, INITIAL_WINDOW_WIDTH, STREAM_ASPECT_HEIGHT, STREAM_ASPECT_WIDTH, VIDEO_FRAMERATE};
use crate::frame_receiver::Message;
use crate::mpeg_encoder::MpegEncoder;
use crate::picture::{rotation_matrix};
use crate::streams::{Streams, StreamOrdinal, StreamSource};
use crate::layout_engine::LayoutEngine;

pub struct State {
    layout: LayoutEngine,

    streams: Streams,
    apriltag_detector: ApriltagDetector,

    selected: Option<(StreamOrdinal, f64, f64)>,

    pictures_taken: Arc<AtomicU32>,
    pictures_for_calibration: HashSet<u32>,

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
            apriltag_detector: ApriltagDetector::new(TagFamily::Tag36h11),
            layout: LayoutEngine::new(window_width, window_height, 1),
            streams: Streams::new(),
            selected: None,

            pictures_taken,
            pictures_for_calibration: HashSet::new(),
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
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
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
                    self.flip();
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

    fn message_handler(&mut self, message: Message) {
        match message {
            Message::StreamDeregistered(stream_id) => self.streams.deregister_stream(stream_id),
            Message::NewImage(stream_id, image) => self.register_frame(stream_id, image),
            Message::Stills(pictures_taken_start, devices) if self.is_for_calibration(pictures_taken_start) => {
                println!("new calibration event");
                self.streams.calibrate(devices, &mut self.apriltag_detector);
            },
            Message::Stills(_, devices) => {
                let mut devices: Vec<_> = devices.into_iter()
                    .map(|(addr, stills)|
                        stills.into_iter().map(move |still| {
                            let stream_id = StreamSource::new(addr, still.device_id());
                            (stream_id, still)
                        }))
                    .flatten()
                    .collect();

                devices.sort_by_key(|&(s, _)| self.streams.get_stream_tile(s));

                let dir = PathBuf::from(format!("outputs/{}", Local::now()));
                fs::create_dir(&dir).unwrap();

                let mut video = MpegEncoder::new_with_params(
                    dir.join("video.mp4"),
                    1080,
                    (1080*STREAM_ASPECT_HEIGHT/STREAM_ASPECT_WIDTH) as usize,
                    None,
                    Some(VIDEO_FRAMERATE),
                    None,
                    None,
                    None,
                );

                for ((source, image), i) in devices.into_iter().zip(1..) {
                    let image = image::load_from_memory_with_format(
                        image.frame_data(),
                        ImageFormat::Jpeg,
                    );

                    if let Ok(image) = image {
                        if let Some(ordinal) = self.streams.get_stream_tile(source) {
                            let image = self.streams.transform_image(ordinal, &image);

                            // save to photo sequence
                            image.save(dir.join(format!("frame{:02}.png", i))).unwrap();
                            // save to video
                            video.encode_image(image.as_rgb8().unwrap());
                        }
                    }
                }

                println!("saved images");
            },
        }
    }

    fn draw(&self) {
        let crop_factor = self.streams.crop_factor() as f32;
        let (rest, last) = self.frames();
        let mut target = self.display.draw();

        target.clear_color(0.0, 0.0, 0.0, 0.0);

        for panel in rest {
            panel.display(crop_factor, &self.display, &mut target, &self.panel_vertex_buffer, &self.panel_index_buffer, &self.panel_shaders);
        }

        if let Some(panel) = last {
            panel.display(crop_factor, &self.display, &mut target, &self.panel_vertex_buffer, &self.panel_index_buffer, &self.panel_shaders);
        }

        if self.selected.is_some() {
            if let Some(tile) = self.hovering_over() {
                draw_box(&mut target, tile, &self.layout, &self.selection_box_vertex_buffer, &self.selection_box_index_buffer, &self.selection_box_shaders);
            }
        }

        target.finish().unwrap();
    }

    fn register_frame(&mut self, source: StreamSource, image: RgbImage) {
        self.streams.register_frame(source, image);
        self.layout.update_stream_count(self.streams.stream_count() as u32);
    }

    fn make_for_calibration(&mut self, pictures_taken: u32) {
        self.pictures_for_calibration.insert(pictures_taken);
    }
    
    fn is_for_calibration(&self, pictures_taken: u32) -> bool {
        self.pictures_for_calibration.contains(&pictures_taken)
    }
    
    fn update_cursor_position(&mut self, new_position: PhysicalPosition<f64>) {
        self.cursor_position = new_position;
    }

    fn hovering_over(&self) -> Option<StreamOrdinal> {
        self.layout.cursor_is_over(self.cursor_position.x as u32, self.cursor_position.y as u32, &self.streams)
    }

    fn select(&mut self) {
        if let Some(tile) = self.hovering_over() {
            let rect = self.layout.viewport_rect(tile);
            self.selected = Some((
                tile,
                rect.left as f64 - self.cursor_position.x,
                rect.bottom as f64 + self.cursor_position.y
            ));
        }
    }

    fn unselect(&mut self) {
        if let Some((old_index, ..)) = self.selected.take() {
            if let Some(new_index) = self.hovering_over() {
                self.streams.remove_and_insert(old_index, new_index);
            }
        }
    }

    fn flip(&mut self) {
        if let Some(ordinal) = self.hovering_over() {
            self.streams.flip(ordinal);
        }
    }

    fn resize(&mut self, new_width: u32, new_height: u32) {
        self.layout.update_screen_size(new_width, new_height);
    }

    fn frames<'a>(&'a self) -> (impl Iterator<Item=PanelToDisplay<'a>> + 'a, Option<PanelToDisplay<'a>>) {
        let iter = self.streams.iter()
            .filter(move |&(tile_index, ..)| !matches!(self.selected, Some((selected_tile, _, _)) if selected_tile == tile_index))
            .map(move |(tile_index, image, rotation_angle)| {
                let viewport_rect = self.layout.viewport_rect(tile_index);
                PanelToDisplay { viewport_rect, rotation_angle, image }
            });

        let last = self.streams.iter()
            .find(|&(tile_index, ..)| matches!(self.selected, Some((selected_tile, _, _)) if selected_tile == tile_index))
            .map(move |(tile_index, image, rotation_angle)| {
                let (_, dx, dy) = self.selected.unwrap();
                let mut viewport_rect = self.layout.viewport_rect(tile_index);
                viewport_rect.left = (dx + self.cursor_position.x) as u32;
                viewport_rect.bottom = (dy - self.cursor_position.y) as u32;
                PanelToDisplay { viewport_rect, rotation_angle, image }
            });

        (iter, last)
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
    layout: &LayoutEngine,
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
    rotation_angle: f64,
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

        let [m0, m1, m2, m3] = rotation_matrix(self.rotation_angle as f32);

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

