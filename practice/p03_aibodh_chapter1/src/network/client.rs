use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    net::{Shutdown, TcpStream},
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};

use bevy::prelude::*;
use rand::random;

use super::protocol::{
    ADDRESS, Identity, Peer, Pose, TICK, TIMEOUT, configure, invalid, read_snapshot, read_welcome,
    send_hello, send_presence,
};

#[derive(Default)]
struct Mailbox {
    pose: Option<Pose>,
    incoming: Option<io::Result<Vec<Peer>>>,
}

/// Bevy systems never read or write the socket. Both directions keep only the latest value.
#[derive(Resource)]
pub(super) struct Connection {
    mailbox: Arc<Mutex<Mailbox>>,
    socket: TcpStream,
}

impl Connection {
    pub fn connect(path: &Path) -> io::Result<(u64, Self)> {
        let id = read_or_create_identity(path)?;
        let mut stream = TcpStream::connect_timeout(&ADDRESS.into(), TIMEOUT)?;
        configure(&stream)?;
        send_hello(&mut stream, id)?;
        let seed = read_welcome(&mut stream)?;
        let socket = stream.try_clone()?;
        let mailbox = Arc::new(Mutex::new(Mailbox::default()));
        let worker_mailbox = Arc::clone(&mailbox);
        thread::Builder::new()
            .name("local-world-connection".into())
            .spawn(move || {
                loop {
                    let start = Instant::now();
                    let pose = worker_mailbox.lock().unwrap().pose;
                    let incoming =
                        send_presence(&mut stream, pose).and_then(|()| read_snapshot(&mut stream));
                    let failed = incoming.is_err();
                    worker_mailbox.lock().unwrap().incoming = Some(incoming);
                    if failed {
                        break;
                    }
                    thread::sleep(TICK.saturating_sub(start.elapsed()));
                }
            })?;
        Ok((seed, Self { mailbox, socket }))
    }

    pub fn set_pose(&self, pose: Option<Pose>) {
        self.mailbox.lock().unwrap().pose = pose;
    }

    pub fn receive(&self) -> Option<io::Result<Vec<Peer>>> {
        self.mailbox.lock().unwrap().incoming.take()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        let _ = self.socket.shutdown(Shutdown::Both);
    }
}

fn read_or_create_identity(path: &Path) -> io::Result<Identity> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            let id = loop {
                let id: Identity = random();
                if id != 0 {
                    break id;
                }
            };
            writeln!(file, "{id:032x}")?;
            file.sync_all()?;
            Ok(id)
        }
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
            let text = fs::read_to_string(path)?;
            let text = text.trim();
            if text.len() == 32
                && let Ok(id) = Identity::from_str_radix(text, 16)
                && id != 0
            {
                return Ok(id);
            }
            Err(invalid(
                "identity file must contain one nonzero 32-digit hexadecimal ID",
            ))
        }
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_persisted_and_corruption_is_not_silently_overwritten() {
        let dir = std::env::temp_dir().join(format!("aibodh-identity-test-{:x}", random::<u128>()));
        fs::create_dir(&dir).unwrap();
        let alice = dir.join("alice");
        let bob = dir.join("bob");
        let id = read_or_create_identity(&alice).unwrap();
        assert_eq!(read_or_create_identity(&alice).unwrap(), id);
        assert_ne!(read_or_create_identity(&bob).unwrap(), id);
        fs::write(&alice, "corrupt").unwrap();
        assert_eq!(
            read_or_create_identity(&alice).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(fs::read_to_string(&alice).unwrap(), "corrupt");
        fs::remove_dir_all(dir).unwrap();
    }
}
