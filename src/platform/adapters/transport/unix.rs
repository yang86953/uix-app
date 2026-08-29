use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::user_identity::UnixUserId;
use super::{AcceptedAgentStream, AgentStreamCancelIo};

const DISCOVERY_DIR_PREFIX: &str = "uix-agent";

pub(crate) struct AgentEndpoint {
    listener: UnixListener,
    socket_path: PathBuf,
    discovery_path: PathBuf,
    discovery_published: bool,
}

#[derive(Clone)]
pub(crate) struct AgentEndpointWake {
    socket_path: PathBuf,
}

impl AgentEndpoint {
    pub(crate) fn bind(process_id: u32, nonce: &str) -> io::Result<Self> {
        let directory = private_discovery_directory()?;
        let socket_path = directory.join(format!("uix-{process_id}-{nonce}.sock"));
        if fs::symlink_metadata(&socket_path).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "agent socket path already exists",
            ));
        }
        let listener = UnixListener::bind(&socket_path)?;
        fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))?;
        let discovery_path = directory.join(format!("uix-{process_id}.json"));
        Ok(Self {
            listener,
            socket_path,
            discovery_path,
            discovery_published: false,
        })
    }

    pub(crate) fn endpoint_name(&self) -> String {
        self.socket_path.to_string_lossy().into_owned()
    }

    pub(crate) fn discovery_path(&self) -> &Path {
        &self.discovery_path
    }

    pub(crate) fn waker(&self) -> AgentEndpointWake {
        AgentEndpointWake {
            socket_path: self.socket_path.clone(),
        }
    }

    pub(crate) fn publish_discovery(&mut self, contents: &[u8], nonce: &str) -> io::Result<()> {
        let temporary = self
            .discovery_path
            .with_extension(format!("json.tmp-{nonce}"));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        let write_result = (|| {
            file.write_all(contents)?;
            file.sync_all()?;
            fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))
        })();
        drop(file);
        let result = write_result.and_then(|()| fs::rename(&temporary, &self.discovery_path));
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        } else {
            self.discovery_published = true;
        }
        result
    }

    pub(crate) fn accept(&mut self) -> io::Result<AcceptedAgentStream> {
        loop {
            let (stream, _) = self.listener.accept()?;
            if peer_is_current_user(&stream)? {
                let cancel = UnixStreamCancel(stream.try_clone()?);
                return Ok(AcceptedAgentStream {
                    stream: Box::new(stream),
                    cancel: Arc::new(cancel),
                });
            }
        }
    }
}

impl AgentEndpointWake {
    pub(crate) fn wake(&self) {
        let _ = UnixStream::connect(&self.socket_path);
    }
}

struct UnixStreamCancel(UnixStream);

impl AgentStreamCancelIo for UnixStreamCancel {
    fn cancel(&self) {
        let _ = self.0.shutdown(std::net::Shutdown::Both);
    }
}

impl Drop for AgentEndpoint {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.socket_path);
        if self.discovery_published {
            let _ = fs::remove_file(&self.discovery_path);
        }
    }
}

pub(crate) fn fill_secure_random(output: &mut [u8]) -> io::Result<()> {
    File::open("/dev/urandom")?.read_exact(output)
}

/// 返回当前用户唯一的 Hub 控制面地址；应用只向该固定入口登记自身实例。
pub(crate) fn agent_hub_endpoint_name() -> io::Result<String> {
    Ok(private_discovery_directory()?
        .join("hub-v1.sock")
        .to_string_lossy()
        .into_owned())
}

/// 建立应用到 Hub 或测试端点的同机字节流。
pub(crate) fn connect(endpoint: &str) -> io::Result<super::AgentStream> {
    Ok(Box::new(UnixStream::connect(endpoint)?))
}

#[cfg(test)]
pub(super) fn discovery_permissions_are_private_for_test(path: &Path) -> io::Result<Option<bool>> {
    Ok(Some(fs::metadata(path)?.permissions().mode() & 0o077 == 0))
}

#[cfg(test)]
pub(super) fn endpoint_permissions_are_private_for_test(
    endpoint: &str,
) -> io::Result<Option<bool>> {
    discovery_permissions_are_private_for_test(Path::new(endpoint))
}

fn private_discovery_directory() -> io::Result<PathBuf> {
    let effective_uid = unsafe {
        // SAFETY: `geteuid` has no arguments and no memory safety preconditions.
        libc::geteuid()
    };
    let base = env::var_os("XDG_RUNTIME_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir);
    let directory = base.join(format!("{DISCOVERY_DIR_PREFIX}-{effective_uid}"));
    match fs::symlink_metadata(&directory) {
        Ok(metadata) => {
            if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "agent discovery path is not a private directory",
                ));
            }
            if metadata.uid() != effective_uid {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "agent discovery directory has a different owner",
                ));
            }
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::DirBuilder::new().mode(0o700).create(&directory)?;
        }
        Err(error) => return Err(error),
    }
    let metadata = fs::symlink_metadata(&directory)?;
    if metadata.mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "agent discovery directory is accessible by another user",
        ));
    }
    Ok(directory)
}

#[cfg(target_os = "linux")]
fn peer_is_current_user(stream: &UnixStream) -> io::Result<bool> {
    let mut credentials = unsafe {
        // SAFETY: `ucred` is a plain C value whose all-zero bit pattern is valid.
        std::mem::zeroed::<libc::ucred>()
    };
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let result = unsafe {
        // SAFETY: the socket fd is live for the call and both output pointers
        // refer to initialized, correctly sized writable storage.
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut credentials as *mut libc::ucred).cast(),
            &mut length,
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    let effective_uid = unsafe {
        // SAFETY: `geteuid` has no arguments and no memory safety preconditions.
        libc::geteuid()
    };
    Ok(UnixUserId::from_raw(effective_uid).admits(UnixUserId::from_raw(credentials.uid)))
}

#[cfg(target_os = "macos")]
fn peer_is_current_user(stream: &UnixStream) -> io::Result<bool> {
    let mut uid = 0;
    let mut gid = 0;
    let result = unsafe {
        // SAFETY: the socket fd is live and both output pointers refer to
        // initialized writable uid/gid storage for the duration of the call.
        libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid)
    };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    let effective_uid = unsafe {
        // SAFETY: `geteuid` has no arguments and no memory safety preconditions.
        libc::geteuid()
    };
    Ok(UnixUserId::from_raw(effective_uid).admits(UnixUserId::from_raw(uid)))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn peer_is_current_user(_stream: &UnixStream) -> io::Result<bool> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "agent peer credentials are unavailable on this Unix target",
    ))
}
