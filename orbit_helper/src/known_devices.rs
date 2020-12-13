use std::collections::{HashMap};
use std::fs;
use orbit_types::{DeviceId, DeviceIdGenerator};
use std::fs::DirEntry;

pub struct KnownDevices {
    index_to_id: HashMap<DeviceFileIndex, DeviceId>,
    id_to_index: HashMap<DeviceId, DeviceFileIndex>,
    device_id_generator: DeviceIdGenerator,

    current_devices: Vec<DeviceFileIndex>,
    to_add: Vec<DeviceFileIndex>,
    to_remove: Vec<DeviceFileIndex>,
}

impl KnownDevices {
    pub fn new() -> KnownDevices {
        let mut known_devices = KnownDevices {
            index_to_id: HashMap::new(),
            id_to_index: HashMap::new(),
            device_id_generator: DeviceIdGenerator::new(),

            current_devices: Vec::new(),
            to_add: Vec::new(),
            to_remove: Vec::new(),
        };

        known_devices.update();

        known_devices
    }

    pub fn update(&mut self) {
        match fs::read_dir("/sys/class/video4linux") {
            Ok(dir) => self.current_devices.extend(dir
                .filter_map(Result::ok)
                .filter_map(DeviceFileIndex::from_dir_entry)),
            Err(_) => {},
        };

        self.to_add.clear();
        for index in self.current_devices.iter() {
            if !!self.index_to_id.contains_key(index) {
                self.to_add.push(*index);
            }
        }

        self.to_remove.clear();
        for index in self.index_to_id.keys() {
            if !self.current_devices.contains(index) {
                self.to_remove.push(*index);
            }
        }

        for &index in self.to_add.iter() {
            let id = self.device_id_generator.next();
            self.index_to_id.insert(index, id);
            self.id_to_index.insert(id, index);
        }

        for &index in self.to_remove.iter() {
            let id: DeviceId = self.index_to_id[&index];
            self.index_to_id.remove(&index);
            self.id_to_index.remove(&id);
        }
    }

    pub fn recently_added(&mut self) ->  impl Iterator<Item=(DeviceFileIndex, DeviceId)> + '_ {
        self.update();

        self.to_add.iter()
            .map(|index| (*index, self.index_to_id[index]))
            .collect::<Vec<_>>()
            .into_iter()
    }

    pub fn video_devices(&mut self) -> impl Iterator<Item=(DeviceFileIndex, DeviceId)> + '_ {
        self.update();
        self.index_to_id.iter()
            .map(|(&k, &v)| (k, v))
    }
}

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
pub struct DeviceFileIndex {
    pub inner: usize,
}

impl DeviceFileIndex {
    pub fn from_dir_entry(dir_entry: DirEntry) -> Option<DeviceFileIndex> {
        let s = dir_entry.file_name();
        let s = s.to_str()?;

        if !s.starts_with("video") { return None }

        let start = "video".len();

        let inner = s[start..].parse().ok()?;

        if is_video_device(inner) {
            Some(DeviceFileIndex { inner })
        } else {
            None
        }
    }

    pub fn file_index(self) -> usize {
        self.inner
    }
}


/// For some reason, when I updated to Ubuntu 20, every video device creates two files in /sys/class/video4linux.
/// One with an odd index, and one with an even index. All of the odd ones aren't valid (ie they
/// don't have any formats). That's what we check for here
fn is_video_device(index: usize) -> bool {
    match v4l::capture::Device::new(index) {
        Ok(d) => d.enum_formats().map_or(false, |l| l.len() > 0),
        Err(_) => false,
    }
}
