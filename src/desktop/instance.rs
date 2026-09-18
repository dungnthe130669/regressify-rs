//! Workspace ownership and bounded, local-only activation between desktop processes.

use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use futures::channel::mpsc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::helpers::write_file_atomically;

const MESSAGE_LIMIT: u64 = 1024;
const IO_TIMEOUT: Duration = Duration::from_millis(300);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Endpoint {
    port: u16,
    token: String,
    process_id: u32,
}

impl Endpoint {
    fn address(&self) -> SocketAddr {
        (Ipv4Addr::LOCALHOST, self.port).into()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Activation {
    token: String,
}

#[derive(Default)]
struct Signal {
    requested: AtomicBool,
    closed: AtomicBool,
}

pub enum OpenInstance {
    Owned(DesktopInstance),
    Activated,
}

pub struct DesktopInstance {
    root: PathBuf,
    _lock: File,
    endpoint: Endpoint,
    endpoint_path: PathBuf,
    signal: Arc<Signal>,
    activations: Option<mpsc::Receiver<()>>,
    listener: Option<JoinHandle<()>>,
}

impl DesktopInstance {
    pub fn open(directory: &Path, root: &Path) -> Result<OpenInstance> {
        let root = root
            .canonicalize()
            .context("Cannot resolve workspace for desktop ownership")?;
        ensure!(root.is_dir(), "Workspace must be a directory");
        let lock_path = workspace_lock_path(directory, &root);
        fs::create_dir_all(directory).context("Cannot create workspace instance directory")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)?;
        let endpoint_path = lock_path.with_extension("json");
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        loop {
            match lock.try_lock() {
                Ok(()) => return Self::serve(root, lock, endpoint_path).map(OpenInstance::Owned),
                Err(TryLockError::WouldBlock) => {}
                Err(TryLockError::Error(error)) => {
                    return Err(error).context("Cannot lock the selected workspace");
                }
            }
            match activate_existing(&endpoint_path) {
                Ok(()) => return Ok(OpenInstance::Activated),
                Err(error) if Instant::now() >= deadline => {
                    return Err(error).context("This workspace is already open, but its window could not be activated. Try again after the other window responds");
                }
                Err(_) => thread::sleep(Duration::from_millis(30)),
            }
        }
    }

    fn serve(root: PathBuf, lock: File, endpoint_path: PathBuf) -> Result<Self> {
        let socket = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .context("Cannot start workspace activation listener")?;
        let endpoint = Endpoint {
            port: socket.local_addr()?.port(),
            token: uuid::Uuid::new_v4().simple().to_string(),
            process_id: std::process::id(),
        };
        let signal = Arc::new(Signal::default());
        let (mut notifications, activations) = mpsc::channel(1);
        let receiver = signal.clone();
        let token = endpoint.token.clone();
        let listener = thread::Builder::new()
            .name("regressify-activation".into())
            .spawn(move || {
                while !receiver.closed.load(Ordering::Acquire) {
                    match socket.accept() {
                        Ok((mut stream, _)) => {
                            if receiver.closed.load(Ordering::Acquire) {
                                break;
                            }
                            let _ = receive_activation(
                                &mut stream,
                                &token,
                                &receiver,
                                &mut notifications,
                            );
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    }
                }
            })?;
        let instance = Self {
            root,
            _lock: lock,
            endpoint,
            endpoint_path,
            signal,
            activations: Some(activations),
            listener: Some(listener),
        };
        write_file_atomically(
            &instance.endpoint_path,
            &serde_json::to_vec(&instance.endpoint)?,
        )
        .context("Cannot publish workspace activation endpoint")?;
        Ok(instance)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn owns_workspace(&self, root: &Path) -> Result<bool> {
        let canonical = root.canonicalize()?;
        Ok(workspace_lock_path(Path::new(""), &self.root)
            == workspace_lock_path(Path::new(""), &canonical))
    }

    pub fn take_activation(&self) -> bool {
        self.signal.requested.swap(false, Ordering::AcqRel)
    }

    pub fn activation_events(&mut self) -> Option<mpsc::Receiver<()>> {
        self.activations.take()
    }
}

impl Drop for DesktopInstance {
    fn drop(&mut self) {
        self.signal.closed.store(true, Ordering::Release);
        let _ = fs::remove_file(&self.endpoint_path);
        let _ = TcpStream::connect_timeout(&self.endpoint.address(), IO_TIMEOUT);
        if let Some(listener) = self.listener.take() {
            let _ = listener.join();
        }
    }
}

fn workspace_lock_path(directory: &Path, canonical: &Path) -> PathBuf {
    #[cfg(windows)]
    let identity = canonical.to_string_lossy().to_lowercase().into_bytes();
    #[cfg(not(windows))]
    let identity = canonical.as_os_str().as_encoded_bytes().to_vec();
    directory.join(format!("{:x}.lock", Sha256::digest(identity)))
}

fn activate_existing(path: &Path) -> Result<()> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MESSAGE_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MESSAGE_LIMIT,
        "Workspace activation endpoint is too large"
    );
    let endpoint: Endpoint = serde_json::from_slice(&bytes)?;
    ensure!(
        endpoint.port != 0 && endpoint.token.len() == 32,
        "Invalid workspace activation endpoint"
    );
    let mut stream = TcpStream::connect_timeout(&endpoint.address(), IO_TIMEOUT)?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    allow_foreground(endpoint.process_id);
    let mut message = serde_json::to_vec(&Activation {
        token: endpoint.token,
    })?;
    message.push(b'\n');
    stream.write_all(&message)?;
    let mut response = [0_u8; 3];
    stream.read_exact(&mut response)?;
    ensure!(
        &response == b"ok\n",
        "Workspace activation was not acknowledged"
    );
    Ok(())
}

fn receive_activation(
    stream: &mut TcpStream,
    token: &str,
    signal: &Signal,
    notifications: &mut mpsc::Sender<()>,
) -> Result<()> {
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let deadline = Instant::now() + IO_TIMEOUT;
    let mut message = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        ensure!(!remaining.is_zero(), "Activation request timed out");
        stream.set_read_timeout(Some(remaining))?;
        let mut buffer = [0_u8; 256];
        let count = stream.read(&mut buffer)?;
        ensure!(count > 0, "Activation request ended before completion");
        message.extend_from_slice(&buffer[..count]);
        ensure!(
            message.len() as u64 <= MESSAGE_LIMIT,
            "Activation request is too large"
        );
        if message.last() == Some(&b'\n') {
            break;
        }
    }
    let activation: Activation = serde_json::from_slice(&message)?;
    ensure!(
        activation.token == token,
        "Activation token does not match the workspace owner"
    );
    signal.requested.store(true, Ordering::Release);
    let _ = notifications.try_send(());
    stream.write_all(b"ok\n")?;
    Ok(())
}

#[cfg(windows)]
fn allow_foreground(process_id: u32) {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn AllowSetForegroundWindow(process_id: u32) -> i32;
    }
    unsafe {
        AllowSetForegroundWindow(process_id);
    }
}

#[cfg(not(windows))]
fn allow_foreground(_: u32) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(directory: &Path, root: &Path) -> DesktopInstance {
        match DesktopInstance::open(directory, root).unwrap() {
            OpenInstance::Owned(owner) => owner,
            OpenInstance::Activated => panic!("Expected a new workspace owner"),
        }
    }

    #[test]
    fn desktop_ownership_is_per_canonical_workspace_and_releases_on_close() {
        let directory = tempfile::tempdir().unwrap();
        let locks = directory.path().join("instances");
        let first = directory.path().join("first");
        let second = directory.path().join("second");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        let first_owner = owner(&locks, &first);
        let second_owner = owner(&locks, &second);
        assert!(matches!(
            DesktopInstance::open(&locks, &first.join(".")).unwrap(),
            OpenInstance::Activated
        ));
        assert!(first_owner.take_activation());
        assert!(!second_owner.take_activation());
        assert!(!first_owner.take_activation());
        #[cfg(windows)]
        assert!(matches!(
            DesktopInstance::open(
                &locks,
                &PathBuf::from(first.to_string_lossy().to_uppercase())
            )
            .unwrap(),
            OpenInstance::Activated
        ));
        drop(first_owner);
        let replacement = owner(&locks, &first);
        assert!(!replacement.take_activation());
        drop(second_owner);
        let _second_replacement = owner(&locks, &second);
    }

    #[test]
    fn concurrent_launches_choose_one_owner_and_activate_it() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let locks = root.join("instances");
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let launches = (0..2)
            .map(|_| {
                let barrier = barrier.clone();
                let root = root.clone();
                let locks = locks.clone();
                thread::spawn(move || {
                    barrier.wait();
                    DesktopInstance::open(&locks, &root).unwrap()
                })
            })
            .collect::<Vec<_>>();
        let mut owners = Vec::new();
        let mut activated = 0;
        for launch in launches {
            match launch.join().unwrap() {
                OpenInstance::Owned(owner) => owners.push(owner),
                OpenInstance::Activated => activated += 1,
            }
        }
        assert_eq!(owners.len(), 1);
        assert_eq!(activated, 1);
        assert!(owners[0].take_activation());
    }

    #[test]
    fn stale_endpoint_is_replaced_when_no_process_holds_the_lock() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let locks = root.join("instances");
        fs::create_dir_all(&locks).unwrap();
        let path = workspace_lock_path(&locks, &root).with_extension("json");
        fs::write(path, "stale endpoint").unwrap();
        let instance = owner(&locks, &root);
        assert!(matches!(
            DesktopInstance::open(&locks, &root).unwrap(),
            OpenInstance::Activated
        ));
        assert!(instance.take_activation());
    }

    #[test]
    fn activation_rejects_foreign_tokens_and_oversized_messages() {
        let directory = tempfile::tempdir().unwrap();
        let instance = owner(&directory.path().join("instances"), directory.path());
        for message in [
            b"{\"token\":\"wrong\"}\n".to_vec(),
            vec![b'x'; MESSAGE_LIMIT as usize + 2],
        ] {
            let mut stream =
                TcpStream::connect_timeout(&instance.endpoint.address(), IO_TIMEOUT).unwrap();
            stream.set_read_timeout(Some(IO_TIMEOUT)).unwrap();
            stream.write_all(&message).unwrap();
            stream.shutdown(std::net::Shutdown::Write).unwrap();
            let mut response = [0_u8; 3];
            assert!(stream.read_exact(&mut response).is_err());
            assert!(!instance.take_activation());
        }
        assert!(matches!(
            DesktopInstance::open(&directory.path().join("instances"), directory.path()).unwrap(),
            OpenInstance::Activated
        ));
        assert!(instance.take_activation());
    }
}
