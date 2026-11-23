//! Loopback-only, trusted local multiplayer server. No deployment or account setup.
#[allow(dead_code)] // This binary uses the server half of the shared codec.
#[path = "../network/protocol.rs"]
mod protocol;

use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::{self, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    sync::{Arc, Mutex},
    thread,
};

use protocol::{
    ADDRESS, Identity, MAX_CLIENTS, Peer, Pose, configure, invalid, read_hello, read_presence,
    send_snapshot, send_welcome,
};

type Roster = Mutex<HashMap<Identity, Option<Pose>>>;

fn world_seed(path: &Path, initial_seed: u64) -> io::Result<u64> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            writeln!(file, "{initial_seed}")?;
            file.sync_all()?;
            Ok(initial_seed)
        }
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => fs::read_to_string(path)?
            .trim()
            .parse()
            .map_err(|_| invalid("world file must contain one unsigned 64-bit seed")),
        Err(err) => Err(err),
    }
}

/// Registration lives for exactly as long as the socket session, even on I/O errors.
struct Session<'a> {
    roster: &'a Roster,
    id: Identity,
}

impl<'a> Session<'a> {
    fn register(roster: &'a Roster, id: Identity) -> Option<Self> {
        let mut players = roster.lock().unwrap();
        if let std::collections::hash_map::Entry::Vacant(entry) = players.entry(id) {
            entry.insert(None);
            Some(Self { roster, id })
        } else {
            None
        }
    }

    fn update(&self, pose: Option<Pose>) -> Vec<Peer> {
        let mut players = self.roster.lock().unwrap();
        players.insert(self.id, pose);
        players
            .iter()
            .filter(|&(&id, _)| id != self.id)
            .filter_map(|(&id, &pose)| Some(Peer { id, pose: pose? }))
            .collect()
    }
}

impl Drop for Session<'_> {
    fn drop(&mut self) {
        self.roster.lock().unwrap().remove(&self.id);
    }
}

fn serve(mut stream: TcpStream, seed: u64, roster: &Roster) -> io::Result<()> {
    configure(&stream)?;
    let id = read_hello(&mut stream)?;
    let Some(session) = Session::register(roster, id) else {
        return send_welcome(&mut stream, None);
    };
    send_welcome(&mut stream, Some(seed))?;
    loop {
        let pose = match read_presence(&mut stream) {
            // An ordinary client close is not an error worth logging.
            Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            result => result?,
        };
        send_snapshot(&mut stream, &session.update(pose))?;
    }
}

fn main() -> io::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let usage = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: local_world_server <world-seed-file> [initial-seed]",
        )
    };
    let path = args.next().ok_or_else(usage)?;
    let initial_seed = args
        .next()
        .map(|s| {
            s.to_str()
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or_else(usage)
        })
        .transpose()?
        .unwrap_or(42);
    if args.next().is_some() {
        return Err(usage());
    }
    let listener = TcpListener::bind(ADDRESS)?;
    let seed = world_seed(Path::new(&path), initial_seed)?;
    let roster = Arc::new(Roster::default());
    println!(
        "Local world on {ADDRESS}, seed {seed} ({})",
        Path::new(&path).display()
    );
    for socket in listener.incoming() {
        let socket = socket?;
        // One owner in the listener plus one per worker, including pending handshakes.
        if Arc::strong_count(&roster) > MAX_CLIENTS {
            continue;
        }
        let roster = Arc::clone(&roster);
        thread::Builder::new()
            .name("local-world-client".into())
            .spawn(move || {
                if let Err(err) = serve(socket, seed, &roster) {
                    eprintln!("Local world client disconnected: {err}");
                }
            })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::{read_snapshot, read_welcome, send_hello, send_presence};

    #[test]
    fn seed_file_is_created_once_and_reused() {
        let dir = std::env::temp_dir().join(format!("aibodh-world-test-{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("seed");
        assert_eq!(world_seed(&path, 99).unwrap(), 99);
        assert_eq!(world_seed(&path, 17).unwrap(), 99);
        fs::write(&path, "invalid").unwrap();
        assert_eq!(
            world_seed(&path, 17).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn two_tcp_clients_share_seed_and_exchange_motion_and_presence() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let roster = Arc::new(Roster::default());
        let connect = |id| {
            let mut client = TcpStream::connect(address).unwrap();
            configure(&client).unwrap();
            let (socket, _) = listener.accept().unwrap();
            let roster = Arc::clone(&roster);
            let worker = thread::spawn(move || serve(socket, 123, &roster).unwrap());
            send_hello(&mut client, id).unwrap();
            let seed = read_welcome(&mut client);
            (client, worker, seed)
        };
        let (mut alice, alice_worker, alice_seed) = connect(1);
        let (mut bob, bob_worker, bob_seed) = connect(2);
        assert_eq!(alice_seed.unwrap(), 123);
        assert_eq!(bob_seed.unwrap(), 123);
        assert!(exchange(&mut alice, None).is_empty());
        let (duplicate, duplicate_worker, error) = connect(1);
        assert_eq!(error.unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        drop(duplicate);
        duplicate_worker.join().unwrap();

        let a = Pose {
            position: [5.0, -50.0],
            facing: [1.0, 0.0],
            lift: 0.0,
            selected: 2,
        };
        let b = Pose {
            position: [50.0, 24.0],
            facing: [0.0, 1.0],
            lift: 0.75,
            selected: 4,
        };
        assert!(exchange(&mut alice, Some(a)).is_empty());
        assert_eq!(exchange(&mut bob, Some(b)), [Peer { id: 1, pose: a }]);
        assert_eq!(exchange(&mut alice, Some(a)), [Peer { id: 2, pose: b }]);
        assert_eq!(exchange(&mut alice, None), [Peer { id: 2, pose: b }]);
        assert!(exchange(&mut bob, Some(b)).is_empty());

        drop(bob);
        bob_worker.join().unwrap();
        assert!(exchange(&mut alice, Some(a)).is_empty());
        let (mut bob, bob_worker, seed) = connect(2);
        assert_eq!(seed.unwrap(), 123);
        assert_eq!(exchange(&mut bob, Some(b)), [Peer { id: 1, pose: a }]);
        drop(alice);
        drop(bob);
        alice_worker.join().unwrap();
        bob_worker.join().unwrap();
    }

    fn exchange(client: &mut TcpStream, pose: Option<Pose>) -> Vec<Peer> {
        send_presence(client, pose).unwrap();
        read_snapshot(client).unwrap()
    }
}
