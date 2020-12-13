use chrono::{DateTime, Utc};
use v4l::buffer::StreamItem;
use v4l::{Buffer, Format, Timestamp};
use serde::{Serialize, Deserialize};
use std::time;

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Hash)]
pub struct DeviceId(u32);

pub struct DeviceIdGenerator(u32);

impl DeviceIdGenerator {
    pub fn new() -> DeviceIdGenerator {
        DeviceIdGenerator(0)
    }

    pub fn next(&mut self) -> DeviceId {
        self.0 += 1;
        DeviceId(self.0)
    }
}

#[derive(Serialize, Deserialize)]
pub enum Request {
    Stream,
    Snap(DateTime<Utc>),
}

#[derive(Serialize, Deserialize)]
pub struct SnapResponse {
    pub frames: Vec<CapturedFrame>,
}

#[derive(Serialize, Deserialize)]
pub enum StreamResponse {
    Stop(DeviceId),
    Frame(CapturedFrame),
}

#[derive(Serialize, Deserialize)]
pub struct CapturedFrame {
    device_id: DeviceId,
    width: u32,
    height: u32,
    encoding_repr: [u8; 4],
    captured_at: DateTime<Utc>,
    frame_data: Vec<u8>,
}

impl<'a> CapturedFrame {
    pub fn from_frame(
        frame: &'a StreamItem<'a, Buffer<'a>>,
        used_format: Format,
        boot_time_utc: DateTime<Utc>,
        device_id: DeviceId,
    ) -> CapturedFrame {
        CapturedFrame {
            device_id,
            width: used_format.width,
            height: used_format.height,
            encoding_repr: used_format.fourcc.repr,
            captured_at: timestamp_to_utc(frame.meta().timestamp, boot_time_utc),
            frame_data: frame[..frame.meta().bytesused as usize].to_vec(),
        }
    }

    pub fn device_id(&self) -> DeviceId {
        self.device_id
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn encoding_repr(&self) -> [u8; 4] {
        self.encoding_repr
    }

    pub fn captured_at(&self) -> &DateTime<Utc> {
        &self.captured_at
    }

    pub fn frame_data(&self) -> &[u8] {
        &self.frame_data
    }
}

fn timestamp_to_utc(timestamp: Timestamp, boot_time_utc: DateTime<Utc>) -> DateTime<Utc> {
    let time_after_boot = chrono::Duration::from_std(time::Duration::from(timestamp)).unwrap();
    boot_time_utc + time_after_boot
}