use chrono::{DateTime, Utc};
use crate::{SNAP_FORMAT};
use v4l::prelude::{CaptureDevice};
use std::net::TcpStream;
use std::{io, thread};
use std::mem::MaybeUninit;
use crate::polling_stream_fork::{Stream};
use std::thread::JoinHandle;
use crate::known_devices::KnownDevices;
use libc::{CLOCK_MONOTONIC, timespec, clock_gettime};
use orbit_types::{CapturedFrame, SnapResponse};

pub fn snap(target_time: DateTime<Utc>, known_devices: &mut KnownDevices, mut writer: TcpStream) {
    let mut handles = Vec::new();

    let boot_time_utc = boot_time_utc();

    for (d, device_id) in known_devices.video_devices() {
        let handle: JoinHandle<io::Result<CapturedFrame>> = thread::spawn(move || {

            let mut dev = CaptureDevice::new(d.file_index())?;
            let used_format = dev.set_format(&SNAP_FORMAT)?;
            let stream = Stream::with_buffers(&mut dev, 1)?;
            let mut active = stream.start()?;

            let burned_frame = active.next()?;

            let mut last_frame = CapturedFrame::from_frame(&burned_frame, used_format, boot_time_utc, device_id);
            let mut last_diff = duration_abs(target_time - *last_frame.captured_at());

            loop {
                let frame = active.next()?;
                let frame = CapturedFrame::from_frame(&frame, used_format, boot_time_utc, device_id);

                let diff = duration_abs(target_time - *frame.captured_at());

                // now we are getting further away from the target time
                if diff >= last_diff { break }

                last_diff = diff;
                last_frame = frame;
            }

            Ok(last_frame)
        });

        handles.push(handle);
    }

    let mut stills = Vec::new();

    for handle in handles {
        if let Ok(frame) = handle.join().unwrap() {
            stills.push(frame);
        }
    }

    let _ = bincode::serialize_into(
        &mut writer,
        &SnapResponse { stills },
    );
}

pub fn duration_abs(duration: chrono::Duration) -> chrono::Duration {
    let nanos = duration.num_nanoseconds().unwrap();
    chrono::Duration::nanoseconds(nanos.abs())
}

pub fn boot_time_utc() -> DateTime<Utc> {
    let time_since_boot = unsafe {
        let mut boot_time = MaybeUninit::<timespec>::uninit();
        clock_gettime(CLOCK_MONOTONIC, boot_time.as_mut_ptr());
        boot_time.assume_init()
    };

    let time_since_boot = chrono::Duration::seconds(time_since_boot.tv_sec as i64)
        + chrono::Duration::nanoseconds(time_since_boot.tv_nsec as i64);

    Utc::now() - time_since_boot
}
