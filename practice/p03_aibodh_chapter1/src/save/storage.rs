//! The two render modes share five slots beside the executable.
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};

use crate::inventory::ItemKind;

pub(super) const SLOT_COUNT: usize = 5;
const VERSION: &str = "1";
const MAX_BYTES: u64 = 256 * 1024;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct Snapshot {
    pub seed: u64,
    pub player: PlayerSave,
    pub inventory: BTreeMap<ItemKind, u32>,
    pub enemies: BTreeMap<usize, EnemySave>,
    pub pickups: Vec<PickupSave>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct PlayerSave {
    pub character: String,
    pub position: [f32; 2],
    pub health: HealthSave,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct EnemySave {
    pub position: [f32; 2],
    pub health: HealthSave,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct HealthSave {
    pub current: f32,
    pub max: f32,
}

impl HealthSave {
    fn valid(&self) -> bool {
        self.max.is_finite() && self.max > 0.0 && self.current > 0.0 && self.current <= self.max
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct PickupSave {
    pub kind: ItemKind,
    pub position: [f32; 2],
}

impl Snapshot {
    fn validate(&self) -> Result<(), String> {
        let finite = |position: [f32; 2]| position.into_iter().all(f32::is_finite);
        if self.player.character.trim().is_empty()
            || !finite(self.player.position)
            || !self.player.health.valid()
            || self
                .enemies
                .values()
                .any(|enemy| !finite(enemy.position) || !enemy.health.valid())
            || self.pickups.iter().any(|item| !finite(item.position))
        {
            return Err("invalid character, coordinates or health".into());
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(super) struct Slots(PathBuf);

impl Slots {
    pub fn beside_executable(voxel: bool) -> io::Result<Self> {
        let exe = std::env::current_exe()?;
        let parent = exe
            .parent()
            .ok_or_else(|| io::Error::other("cannot locate executable directory"))?;
        Ok(Self(parent.join(if voxel {
            "saves-overworld"
        } else {
            "saves"
        })))
    }

    fn path(&self, slot: usize) -> Result<PathBuf, String> {
        if slot >= SLOT_COUNT {
            return Err("invalid slot number".into());
        }
        Ok(self.0.join(format!("slot_{}.sav", slot + 1)))
    }

    pub fn read(&self, slot: usize) -> Result<Option<Snapshot>, String> {
        let file = match File::open(self.path(slot)?) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("could not open save: {error}")),
        };
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("could not read save: {error}"))?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err("save is too large".into());
        }
        decode(&bytes).map(Some)
    }

    pub fn write(&self, slot: usize, snapshot: &Snapshot) -> Result<(), String> {
        let path = self.path(slot)?;
        let bytes = encode(snapshot)?;
        fs::create_dir_all(&self.0)
            .and_then(|()| write_atomic(&path, &bytes))
            .map_err(|error| format!("could not write save: {error}"))
    }
}

fn checksum(bytes: &[u8]) -> u64 {
    // FNV-1a for accidental corruption, not authentication.
    bytes.iter().fold(0xcbf29ce484222325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

fn encode(snapshot: &Snapshot) -> Result<Vec<u8>, String> {
    snapshot.validate()?;
    let body = ron::ser::to_string_pretty(snapshot, ron::ser::PrettyConfig::default())
        .map_err(|error| format!("could not encode save: {error}"))?;
    let text = format!(
        "AIBODH {VERSION} {:016x}\n{body}",
        checksum(body.as_bytes())
    );
    if text.len() as u64 > MAX_BYTES {
        return Err("save is too large".into());
    }
    Ok(text.into_bytes())
}

fn decode(bytes: &[u8]) -> Result<Snapshot, String> {
    let mut lines = bytes.splitn(2, |&byte| byte == b'\n');
    let header = lines.next().unwrap_or_default();
    let body = lines.next().ok_or("missing save header")?;
    let mut fields = std::str::from_utf8(header)
        .map_err(|_| "invalid save header")?
        .split_ascii_whitespace();
    if fields.next() != Some("AIBODH") {
        return Err("unrecognized save format".into());
    }
    if fields.next() != Some(VERSION) {
        return Err("unsupported save version".into());
    }
    let expected = fields
        .next()
        .and_then(|value| u64::from_str_radix(value, 16).ok())
        .ok_or("invalid save checksum")?;
    if fields.next().is_some() || expected != checksum(body) {
        return Err("save checksum mismatch".into());
    }
    let snapshot: Snapshot =
        ron::de::from_bytes(body).map_err(|error| format!("could not decode save: {error}"))?;
    snapshot.validate()?;
    Ok(snapshot)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let (temp, mut file) = loop {
        let nonce = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let temp = path.with_extension(format!("{}.{}.tmp", std::process::id(), nonce));
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => break (temp, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    // Keep the old slot until the replacement has been written in full.
    let result = written.and_then(|()| fs::rename(&temp, path));
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub(in crate::save) fn example() -> Snapshot {
        Snapshot {
            seed: u64::MAX - 5,
            player: PlayerSave {
                character: "Traveler".into(),
                position: [-16.0, 32.5],
                health: HealthSave {
                    current: 13.0,
                    max: 100.0,
                },
            },
            inventory: BTreeMap::from([(ItemKind::Herb, 3), (ItemKind::Fern, 1)]),
            enemies: BTreeMap::from([(
                1,
                EnemySave {
                    position: [2.0, -10.0],
                    health: HealthSave {
                        current: 4.0,
                        max: 20.0,
                    },
                },
            )]),
            pickups: vec![PickupSave {
                kind: ItemKind::Mushroom,
                position: [16.0, -64.0],
            }],
        }
    }

    #[test]
    fn round_trip_detects_bad_version_and_corruption() {
        let snapshot = example();
        let mut bytes = encode(&snapshot).unwrap();
        assert_eq!(decode(&bytes).unwrap(), snapshot);
        bytes[7] = b'0';
        assert_eq!(decode(&bytes).unwrap_err(), "unsupported save version");
        bytes[7] = b'1';
        bytes.pop();
        assert_eq!(decode(&bytes).unwrap_err(), "save checksum mismatch");
    }

    #[test]
    fn slot_can_be_overwritten_without_touching_other_slots() {
        let directory = std::env::temp_dir().join(format!(
            "aibodh-slots-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let slots = Slots(directory);
        let mut first = example();
        let second = example();
        assert!(slots.read(0).unwrap().is_none());
        assert!(slots.write(SLOT_COUNT, &first).is_err());
        slots.write(0, &first).unwrap();
        slots.write(1, &second).unwrap();
        first.player.health.current = 5.0;
        slots.write(0, &first).unwrap();
        assert_eq!(slots.read(0).unwrap(), Some(first));
        assert_eq!(slots.read(1).unwrap(), Some(second));
        fs::remove_dir_all(slots.0).unwrap();
    }

    #[test]
    fn invalid_position_and_health_are_rejected() {
        let mut snapshot = example();
        snapshot.player.position[0] = f32::NAN;
        assert!(encode(&snapshot).is_err());
        snapshot.player.position[0] = 0.0;
        snapshot.player.health.current = 0.0;
        assert!(encode(&snapshot).is_err());
    }
}
