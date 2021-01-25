
// cp etomicbomb@192.168.2.1:/home/etomicbomb/Desktop/orbit_helper/target/armv7-unknown-linux-gnueabihf/release/orbit_helper .

use std::net::{TcpListener};
use std::time::{Duration};
use crate::known_devices::KnownDevices;
use libc::c_int;
use v4l::{Format, FourCC};
use orbit_types::{Request};
use v4l::format::{FieldOrder, Colorspace, Quantization, TransferFunction, Flags};
use std::{thread, io, io::Write};
use std::fs::{File, OpenOptions};
use chrono::Local;
use std::path::PathBuf;

mod stream;
mod snap;
mod known_devices;
mod polling_stream_fork;

// TODO:
// replace
// figure out how to stop stream
// need same types in orbit_station, perhaps put everything in cargo workspace

const POLL_TIMEOUT_MILLIS: c_int = 1000;
// sleep between checking for new devices while streaming
// remember
const NEW_DEVICE_CHECK: Duration = Duration::from_secs(1);
const SNAP_FORMAT: Format = new_format(1280, 720, ACCEPTABLE_FORMAT);
const STREAM_FORMAT: Format = new_format(640, 360, ACCEPTABLE_FORMAT);
const ACCEPTABLE_FORMAT: &[u8; 4] = b"MJPG";
const CRASH_RETRY_DELAY: Duration = Duration::from_secs(2);


fn main() {
    let mut log_file = get_log_file();

    loop {
        let _ = writeln!(log_file, "{}: started", Local::now());
        println!("started");
        let error = run();
        let _ = writeln!(log_file, "{}: restarting because of error {:?}", Local::now(), error);
        println!("restarting");
        thread::sleep(CRASH_RETRY_DELAY);
    }
}

fn run() -> io::Result<()> {
    let mut known_devices = KnownDevices::new();

    let listener = TcpListener::bind("0.0.0.0:2000")?;
    for connection in listener.incoming() {
        if let Ok(mut connection) = connection {
            println!("handling new connection");
            match bincode::deserialize_from(&mut connection) {
                Ok(Request::Stream) => stream::stream(connection, &mut known_devices),
                Ok(Request::Snap(target_time)) => {
                    snap::snap(target_time, &mut known_devices, connection);
                },
                Err(_) => {},
            }
        }
    }

    Ok(())
}

fn get_log_file() -> File {
    let path = std::env::var_os("HOME").unwrap();
    println!("opening log file at {:?}", path);
    let path = PathBuf::from(path).join(".orbit_log");

    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap()
}

const fn new_format(width: u32, height: u32, fourcc: &[u8; 4]) -> Format {
    Format {
        width,
        height,
        fourcc: FourCC { repr: *fourcc },
        field_order: FieldOrder::Any,
        stride: 0,
        size: 0,
        flags: Flags::empty(),
        colorspace: Colorspace::Default,
        quantization: Quantization::Default,
        transfer: TransferFunction::Default,
    }
}
