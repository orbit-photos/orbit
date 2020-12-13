use std::net::{TcpStream, UdpSocket};
use std::io::{Write, Read, BufReader};
use image::{ImageDecoder, ImageFormat};

use crate::picture::ImageTransformExt;
use piston_window::{TextureSettings, Texture, WindowSettings, PistonWindow};
use piston_window::texture::CreateTexture;
use std::time::UNIX_EPOCH;

const HELPER_IPS: &[&str] = &[
    "192.168.2.100:2000",
    //"192.168.2.101:2000",
];

fn main() {

    // udp_test();
    for ip in HELPER_IPS {
        let mut stream = TcpStream::connect(ip).unwrap();

        write!(stream, "snap").unwrap();

        let mut stream = BufReader::new(stream);

        let start = std::time::Instant::now();

        let mut frames_data = Vec::new();
        for _ in 0..4 {
            let mut content_len_buf = [0; 4];
            stream.read_exact(&mut content_len_buf).unwrap();

            let content_len = u32::from_be_bytes(content_len_buf);
            let mut frame_data = vec![0; content_len as usize];

            stream.read_exact(&mut frame_data).unwrap();

            frames_data.push(frame_data);
        }

        std::mem::drop(stream);

        // let mut frames_data: Vec<Vec<u8>> = bincode::deserialize_from(&mut stream).unwrap();

        dbg!(start.elapsed());

        for (i, frame_data) in frames_data.into_iter().enumerate() {
            std::fs::write(&format!("hello{}.png", i), frame_data).unwrap();
        }
    }
}
