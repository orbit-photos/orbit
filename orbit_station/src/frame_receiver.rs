use crate::state::StreamId;
use image::{RgbImage, ImageFormat};
use std::net::{SocketAddr, TcpStream};
use orbit_types::{CapturedFrame, Request, StreamResponse, SnapResponse};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{thread, io};
use std::io::BufReader;
use chrono::Utc;
use crate::STILL_CAPTURE_DELAY_MILLIS;

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
                let stream_id = StreamId::new(socket_addr, device_id);
                message_sender.send(Message::StreamDeregistered(stream_id)).unwrap();
            },
            StreamResponse::Frame(frame) => {
                let stream_id = StreamId::new(socket_addr, frame.device_id());

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
