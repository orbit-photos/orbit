// use glium::index::PrimitiveType;
// use glium::{glutin, Surface};
// use glium::{implement_vertex, program, uniform};
// use std::sync::{mpsc, RwLock};
// use std::thread;
// use std::time::Instant;
// use orbit_types::CapturedFrame;
// use glium::glutin::event::{DeviceEvent, VirtualKeyCode};
//
// fn main() {
//     // Setup the GL display stuff
//     let event_loop = glutin::event_loop::EventLoop::new();
//     let wb = glutin::window::WindowBuilder::new();
//     let cb = glutin::ContextBuilder::new().with_vsync(true);
//     let display = glium::Display::new(wb, cb, &event_loop).unwrap();
//
//     // building the vertex buffer, which contains all the vertices that we will draw
//     let vertex_buffer = {
//         #[derive(Copy, Clone)]
//         struct Vertex {
//             position: [f32; 2],
//             tex_coords: [f32; 2],
//         }
//
//         implement_vertex!(Vertex, position, tex_coords);
//
//         glium::VertexBuffer::new(
//             &display,
//             &[
//                 Vertex {
//                     position: [-1.0, -1.0],
//                     tex_coords: [0.0, 0.0],
//                 },
//                 Vertex {
//                     position: [-1.0, 1.0],
//                     tex_coords: [0.0, 1.0],
//                 },
//                 Vertex {
//                     position: [1.0, 1.0],
//                     tex_coords: [1.0, 1.0],
//                 },
//                 Vertex {
//                     position: [1.0, -1.0],
//                     tex_coords: [1.0, 0.0],
//                 },
//             ],
//         )
//             .unwrap()
//     };
//
//     // building the index buffer
//     let index_buffer =
//         glium::IndexBuffer::new(&display, PrimitiveType::TriangleStrip, &[1 as u16, 2, 0, 3])
//             .unwrap();
//
//     // compiling shaders and linking them together
//     let program = program!(&display,
//         140 => {
//             vertex: "
//                 #version 140
//                 uniform mat4 matrix;
//                 in vec2 position;
//                 in vec2 tex_coords;
//                 out vec2 v_tex_coords;
//                 void main() {
//                     gl_Position = matrix * vec4(position, 0.0, 1.0);
//                     v_tex_coords = tex_coords;
//                 }
//             ",
//
//             fragment: "
//                 #version 140
//                 uniform sampler2D tex;
//                 in vec2 v_tex_coords;
//                 out vec4 f_color;
//                 void main() {
//                     f_color = texture(tex, v_tex_coords);
//                 }
//             "
//         },
//     ).unwrap();
//
//     let (tx, rx) = std::sync::mpsc::channel::<CapturedFrame>();
//
//     event_loop.run(move |event, _, control_flow| {
//         let frame = rx.recv().unwrap();
//
//         let image =
//             glium::texture::RawImage2d::from_raw_rgb_reversed(&frame.frame_data(), (frame.width(), frame.height()));
//         let opengl_texture = glium::texture::Texture2d::new(&display, image).unwrap();
//
//         // building the uniforms
//         let uniforms = uniform! {
//             matrix: [
//                 [1.0, 0.0, 0.0, 0.0],
//                 [0.0, 1.0, 0.0, 0.0],
//                 [0.0, 0.0, 1.0, 0.0],
//                 [0.0, 0.0, 0.0, 1.0f32]
//             ],
//             tex: &opengl_texture
//         };
//
//         // drawing a frame
//         let mut target = display.draw();
//         target.clear_color(0.0, 0.0, 0.0, 0.0);
//         target
//             .draw(
//                 &vertex_buffer,
//                 &index_buffer,
//                 &program,
//                 &uniforms,
//                 &Default::default(),
//             )
//             .unwrap();
//         target.finish().unwrap();
//
//         // polling and handling the events received by the window
//         match event {
//             glutin::event::Event::WindowEvent { event, .. } => match event {
//                 glutin::event::WindowEvent::CloseRequested => {
//                     *control_flow = glutin::event_loop::ControlFlow::Exit;
//                 }
//                 _ => {}
//             },
//             glutin::event::Event::DeviceEvent { event: DeviceEvent::Key(input), .. } => {
//                 match input.virtual_keycode {
//                     Some(VirtualKeyCode::S) => {
//                         dbg!("s pressed");
//                     },
//                     Some(VirtualKeyCode::Space) => {
//                         dbg!("space pressed");
//                     },
//                     _ => {},
//                 }
//             }
//             _ => {}
//         }
//     });
// }
