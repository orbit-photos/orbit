use chrono::{DateTime, Utc};
use v4l::buffer::StreamItem;
use v4l::{Buffer, Format, Timestamp};
use serde::{Serialize, Deserialize};
use std::io::{Write, Read};

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
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
    pub stills: Vec<CapturedFrame>,
}

pub enum StreamResponse {
    Stop(DeviceId),
    Frame(CapturedFrame),
}

impl StreamResponse {
    pub fn serialize_into(&self, mut writer: impl Write) -> bincode::Result<()> {
        match *self {
            StreamResponse::Stop(device_id) =>
                bincode::serialize_into(&mut writer, &StreamResponseInfo::Stop(device_id))?,
            StreamResponse::Frame(ref frame) => {
                bincode::serialize_into(&mut writer, &StreamResponseInfo::Frame(frame.metadata))?;
                writer.write_all(&frame.frame_data)?;
            },
        };
        Ok(())
    }

    pub fn deserialize_from(mut reader: impl Read) -> bincode::Result<StreamResponse> {
        let stream_response_info: StreamResponseInfo = bincode::deserialize_from(&mut reader)?;

        Ok(match stream_response_info {
            StreamResponseInfo::Stop(device_id) => StreamResponse::Stop(device_id),
            StreamResponseInfo::Frame(metadata) => {
                let mut frame_data = vec![0u8; metadata.frame_data_len as usize];
                reader.read_exact(&mut frame_data)?;
                StreamResponse::Frame(CapturedFrame { metadata, frame_data })
            },
        })
    }
}

#[derive(Serialize, Deserialize)]
enum StreamResponseInfo {
    Stop(DeviceId),
    Frame(FrameMetadata),
}

#[derive(Serialize, Deserialize, Copy, Clone)]
struct FrameMetadata {
    device_id: DeviceId,
    width: u32,
    height: u32,
    encoding_repr: [u8; 4],
    captured_at: DateTime<Utc>,
    frame_data_len: u32,
}

#[derive(Serialize, Deserialize)]
pub struct CapturedFrame {
    metadata: FrameMetadata,
    frame_data: Vec<u8>,
}

impl<'a> CapturedFrame {
    pub fn from_frame(
        frame: &'a StreamItem<'a, Buffer<'a>>,
        used_format: Format,
        boot_time_utc: DateTime<Utc>,
        device_id: DeviceId,
    ) -> CapturedFrame {
        let frame_data = frame[..frame.meta().bytesused as usize].to_vec();

        let metadata = FrameMetadata {
            device_id,
            width: used_format.width,
            height: used_format.height,
            encoding_repr: used_format.fourcc.repr,
            captured_at: timestamp_to_utc(frame.meta().timestamp, boot_time_utc),
            frame_data_len: frame_data.len() as u32,
        };

        CapturedFrame { metadata, frame_data }
    }

    pub fn device_id(&self) -> DeviceId {
        self.metadata.device_id
    }

    pub fn width(&self) -> u32 {
        self.metadata.width
    }

    pub fn height(&self) -> u32 {
        self.metadata.height
    }

    pub fn encoding_repr(&self) -> [u8; 4] {
        self.metadata.encoding_repr
    }

    pub fn captured_at(&self) -> &DateTime<Utc> {
        &self.metadata.captured_at
    }

    pub fn frame_data(&self) -> &[u8] {
        &self.frame_data
    }
}

fn timestamp_to_utc(timestamp: Timestamp, boot_time_utc: DateTime<Utc>) -> DateTime<Utc> {
    let time_after_boot = chrono::Duration::seconds(timestamp.sec as i64)
        + chrono::Duration::microseconds(timestamp.usec as i64);

    boot_time_utc + time_after_boot
}