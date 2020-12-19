use std::{io, sync::Arc, thread};
use std::net::TcpStream;
use v4l::prelude::CaptureDevice;
use crate::known_devices::{KnownDevices, DeviceFileIndex};
use crate::{STREAM_FORMAT, NEW_DEVICE_CHECK};
use std::sync::Mutex;
use orbit_types::{CapturedFrame};
use std::sync::atomic::{AtomicBool, Ordering};
use orbit_types::{DeviceId, StreamResponse};
use crate::snap::boot_time_utc;
use crate::polling_stream_fork::Stream;

pub fn stream(
    connection: TcpStream,
    known_devices: &mut KnownDevices,
) {
    let writer = Arc::new(Mutex::new(connection));
    let should_stop = Arc::new(AtomicBool::new(false));

    for (device_index, device_id) in known_devices.video_devices() {
        let writer = Arc::clone(&writer);
        let should_stop = Arc::clone(&should_stop);

        spawn_stream_listener(device_index, device_id, writer, should_stop)
    }

    while !should_stop.load(Ordering::Relaxed) {
        for (device_index, device_id) in known_devices.recently_added() {

            let writer = Arc::clone(&writer);
            let should_stop = Arc::clone(&should_stop);

            spawn_stream_listener(device_index, device_id, writer, should_stop)
        }

        thread::sleep(NEW_DEVICE_CHECK);
    }
}

fn spawn_stream_listener(
    device_index: DeviceFileIndex,
    device_id: DeviceId,
    writer: Arc<Mutex<TcpStream>>,
    should_stop: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        println!("{:?} {:?}", device_index, device_id);
        match stream_inner(device_index, device_id, Arc::clone(&writer), Arc::clone(&should_stop)) {
            Ok(_) => {},
            Err(OrbitError::TcpStreamFailed(_)) => should_stop.store(true, Ordering::Relaxed), // can't report lol
            Err(OrbitError::WebcamFailed(_)) => {
                // ignore error because nothing to do
                let _ = bincode::serialize_into(
                    &mut *writer.lock().unwrap(),
                    &StreamResponse::Stop(device_id)
                );
            }
        }
    });
}

fn stream_inner(
    device_index: DeviceFileIndex,
    device_id: DeviceId,
    writer: Arc<Mutex<TcpStream>>,
    should_stop: Arc<AtomicBool>,
) -> OrbitResult<()> {

    let boot_time_utc = boot_time_utc();

    let mut device = CaptureDevice::new(device_index.file_index())?;
    let used_format = device.set_format(&STREAM_FORMAT)?;

    let stream = Stream::with_buffers(&device, 1)?;
    let mut stream = stream.start()?;

    loop {
        if should_stop.load(Ordering::Relaxed) { break }

        let frame = &stream.next()?;

        let frame = CapturedFrame::from_frame(frame, used_format, boot_time_utc, device_id);

        bincode::serialize_into(
            &mut *writer.lock().unwrap(),
            &StreamResponse::Frame(frame),
        )?;

        println!("sent frame {:?}", device_id);
    }

    Ok(())
}

pub type OrbitResult<T> = Result<T, OrbitError>;

pub enum OrbitError {
    TcpStreamFailed(bincode::Error),
    WebcamFailed(io::Error),
}

impl From<bincode::Error> for OrbitError {
    fn from(e: bincode::Error) -> OrbitError {
        OrbitError::TcpStreamFailed(e)
    }
}

impl From<io::Error> for OrbitError {
    fn from(e: io::Error) -> OrbitError {
        OrbitError::WebcamFailed(e)
    }
}
