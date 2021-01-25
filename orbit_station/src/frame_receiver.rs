use image::{RgbImage, ImageFormat};
use std::net::{SocketAddr, TcpStream};
use orbit_types::{CapturedFrame, Request, StreamResponse, SnapResponse};
use std::sync::mpsc::Sender;
use std::{thread, io};
use std::io::BufReader;
use chrono::Utc;
use crate::STILL_CAPTURE_DELAY_MILLIS;
use crate::streams::StreamSource;
use crate::state::{PictureEventState, PictureEvent};

pub enum Message {
    StreamDeregistered(StreamSource),
    NewImage(StreamSource, RgbImage),
    Stills(PictureEvent, Vec<(SocketAddr, Vec<CapturedFrame>)>),
}

pub fn spawn_capture_loop(addrs: Vec<SocketAddr>, message_sender: Sender<Message>, picture_event_state: PictureEventState) {
    thread::spawn(move || {
        loop {
            // streaming mode
            let last_event = picture_event_state.current_event();

            let stream_handles: Vec<_> = addrs.iter()
                .map(|&socket_addr| {
                    let message_sender = message_sender.clone();
                    let picture_event_state = picture_event_state.clone();
                    thread::spawn(move || stream(socket_addr, &message_sender, last_event, picture_event_state))
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
            message_sender.send(Message::Stills(last_event, stills)).unwrap();
        }
    });
}

fn stream(
    socket_addr: SocketAddr,
    message_sender: &Sender<Message>,
    last_event: PictureEvent,
    picture_event_state: PictureEventState,
) -> io::Result<()> {
    let mut connection = TcpStream::connect(socket_addr)?;

    bincode::serialize_into(
        &mut connection,
        &Request::Stream,
    ).unwrap();

    let mut connection = BufReader::new(connection);

    loop {
        if picture_event_state.has_been_new_event_since(last_event) { break Ok(()) }

        let response = StreamResponse::deserialize_from(&mut connection).unwrap();

        match response {
            StreamResponse::Stop(device_id) => {
                let stream_id = StreamSource::new(socket_addr, device_id);
                message_sender.send(Message::StreamDeregistered(stream_id)).unwrap();
            },
            StreamResponse::Frame(frame) => {
                let stream_id = StreamSource::new(socket_addr, frame.device_id());

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

    let requested_capture_time = Utc::now() + chrono::Duration::milliseconds(STILL_CAPTURE_DELAY_MILLIS);
    println!("requested a frame at {:?}", requested_capture_time);

    bincode::serialize_into(
        &mut connection,
        &Request::Snap(requested_capture_time),
    ).unwrap();

    let mut connection = BufReader::new(connection);

    let snap_response: SnapResponse = bincode::deserialize_from(&mut connection).unwrap();

    Ok(snap_response)
}
