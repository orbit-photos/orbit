
// cp etomicbomb@192.168.2.1:/home/etomicbomb/Desktop/orbit_helper/target/armv7-unknown-linux-gnueabihf/release/orbit_helper .

use std::net::{TcpListener};
use std::time::{Duration};
use crate::known_devices::KnownDevices;
use libc::c_int;
use v4l::{Format, FourCC};
use orbit_types::{Request};

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
const SNAP_FORMAT: Format = new_format(1280, 720, b"MJPG");
const STREAM_FORMAT: Format = new_format(300, 144, b"MJPG");

fn main() {
    let mut known_devices = KnownDevices::new();

    let listener = TcpListener::bind("0.0.0.0:2000").unwrap();
    for connection in listener.incoming() {
        if let Ok(mut connection) = connection {
            match bincode::deserialize_from(&mut connection) {
                Ok(Request::Stream) => stream::stream(connection, &mut known_devices),
                Ok(Request::Snap(target_time)) => {
                    let _ = snap::snap(target_time, &mut known_devices, connection);
                },
                Err(_) => {},
            }
        }
    }
}

const fn new_format(width: u32, height: u32, fourcc: &[u8; 4]) -> Format {
    use v4l::format::{FieldOrder, Colorspace, Quantization, TransferFunction, Flags};

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
