//! Local-world wire protocol. Fixed-width frames (little-endian) keep reads and
//! allocations bounded. TCP orders requests; each pose request receives one snapshot.
use std::{
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpStream},
    time::Duration,
};

pub(super) const ADDRESS: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 43128);
pub(super) const MAX_CLIENTS: usize = 16;
pub(super) const TICK: Duration = Duration::from_millis(50);
pub(super) const TIMEOUT: Duration = Duration::from_secs(3);
const MAGIC: [u8; 4] = *b"ABW1";

pub(super) type Identity = u128;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Pose {
    pub position: [f32; 2],
    pub facing: [f32; 2],
    pub lift: f32,
    pub selected: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Peer {
    pub id: Identity,
    pub pose: Pose,
}

pub(super) fn configure(socket: &TcpStream) -> io::Result<()> {
    socket.set_nodelay(true)?;
    socket.set_read_timeout(Some(TIMEOUT))?;
    socket.set_write_timeout(Some(TIMEOUT))
}

pub(super) fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn read<const N: usize>(reader: &mut impl Read) -> io::Result<[u8; N]> {
    let mut bytes = [0; N];
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}

pub(super) fn send_hello(writer: &mut impl Write, id: Identity) -> io::Result<()> {
    let mut message = [0; 20];
    message[..4].copy_from_slice(&MAGIC);
    message[4..].copy_from_slice(&id.to_le_bytes());
    writer.write_all(&message)
}

pub(super) fn read_hello(reader: &mut impl Read) -> io::Result<Identity> {
    if read::<4>(reader)? != MAGIC {
        return Err(invalid("not a compatible local world client"));
    }
    let id = Identity::from_le_bytes(read(reader)?);
    if id == 0 {
        return Err(invalid("invalid local player identity"));
    }
    Ok(id)
}

pub(super) fn send_welcome(writer: &mut impl Write, seed: Option<u64>) -> io::Result<()> {
    match seed {
        Some(seed) => {
            let mut message = [1; 9];
            message[1..].copy_from_slice(&seed.to_le_bytes());
            writer.write_all(&message)
        }
        None => writer.write_all(&[0]),
    }
}

pub(super) fn read_welcome(reader: &mut impl Read) -> io::Result<u64> {
    match read::<1>(reader)?[0] {
        1 => Ok(u64::from_le_bytes(read(reader)?)),
        0 => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "this identity is already in use; use a different identity file",
        )),
        _ => Err(invalid("not a compatible local world server")),
    }
}

const POSE_BYTES: usize = 21;
fn encode_pose(pose: Pose) -> [u8; POSE_BYTES] {
    let mut bytes = [0; POSE_BYTES];
    for (slot, value) in bytes.chunks_exact_mut(4).zip(
        pose.position
            .into_iter()
            .chain(pose.facing)
            .chain([pose.lift]),
    ) {
        slot.copy_from_slice(&value.to_le_bytes());
    }
    bytes[20] = pose.selected;
    bytes
}

fn read_pose(reader: &mut impl Read) -> io::Result<Pose> {
    let bytes = read::<POSE_BYTES>(reader)?;
    let mut floats = bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().expect("four-byte chunk")));
    let [x, y, dx, dy, lift] = std::array::from_fn(|_| floats.next().expect("five floats"));
    // Keep non-finite or extreme values out of interpolation, depth and render math.
    let facing_length = dx * dx + dy * dy;
    if ![x, y]
        .into_iter()
        .all(|n| n.is_finite() && n.abs() <= 1_000_000.0)
        || !(0.99..=1.01).contains(&facing_length)
        || !(0.0..=1.0).contains(&lift)
    {
        return Err(invalid("invalid player pose"));
    }
    Ok(Pose {
        position: [x, y],
        facing: [dx, dy],
        lift,
        selected: bytes[20],
    })
}

pub(super) fn send_presence(writer: &mut impl Write, pose: Option<Pose>) -> io::Result<()> {
    match pose {
        Some(pose) => {
            let mut bytes = [1; 1 + POSE_BYTES];
            bytes[1..].copy_from_slice(&encode_pose(pose));
            writer.write_all(&bytes)
        }
        None => writer.write_all(&[0]),
    }
}

pub(super) fn read_presence(reader: &mut impl Read) -> io::Result<Option<Pose>> {
    match read::<1>(reader)?[0] {
        0 => Ok(None),
        1 => read_pose(reader).map(Some),
        _ => Err(invalid("invalid presence tag")),
    }
}

pub(super) fn send_snapshot(writer: &mut impl Write, peers: &[Peer]) -> io::Result<()> {
    let mut message = [0; 1 + MAX_CLIENTS * (16 + POSE_BYTES)];
    message[0] = peers.len() as u8;
    let mut end = 1;
    for peer in peers {
        message[end..end + 16].copy_from_slice(&peer.id.to_le_bytes());
        end += 16;
        message[end..end + POSE_BYTES].copy_from_slice(&encode_pose(peer.pose));
        end += POSE_BYTES;
    }
    writer.write_all(&message[..end])
}

pub(super) fn read_snapshot(reader: &mut impl Read) -> io::Result<Vec<Peer>> {
    let count = read::<1>(reader)?[0] as usize;
    if count >= MAX_CLIENTS {
        return Err(invalid("too many remote players in snapshot"));
    }
    (0..count)
        .map(|_| {
            Ok(Peer {
                id: Identity::from_le_bytes(read(reader)?),
                pose: read_pose(reader)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presence_round_trips_and_rejects_invalid_motion() {
        let pose = Pose {
            position: [-100.0, 200.0],
            facing: [-1.0, 0.0],
            lift: 0.5,
            selected: 5,
        };
        for presence in [Some(pose), None] {
            let mut bytes = Vec::new();
            send_presence(&mut bytes, presence).unwrap();
            assert_eq!(read_presence(&mut bytes.as_slice()).unwrap(), presence);
        }
        let mut bytes = Vec::new();
        send_presence(
            &mut bytes,
            Some(Pose {
                position: [f32::NAN, 0.0],
                ..pose
            }),
        )
        .unwrap();
        assert_eq!(
            read_presence(&mut bytes.as_slice()).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            read_snapshot(&mut [MAX_CLIENTS as u8].as_slice())
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert!(read_presence(&mut [1].as_slice()).is_err());
    }
}
