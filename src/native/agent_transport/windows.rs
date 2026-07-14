use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use windows::core::{BOOL, HRESULT, PCWSTR};
use windows::Win32::Foundation::{
    LocalFree, ERROR_ALREADY_EXISTS, ERROR_PIPE_CONNECTED, GENERIC_READ, GENERIC_WRITE, HLOCAL,
};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG};
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows::Win32::Storage::FileSystem::{
    CreateDirectoryW, CreateFileW, CREATE_NEW, FILE_ATTRIBUTE_NORMAL,
    FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_SHARE_MODE, PIPE_ACCESS_DUPLEX,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, WaitNamedPipeW, NAMED_PIPE_MODE, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows::Win32::System::IO::CancelSynchronousIo;

use super::{AcceptedAgentStream, AgentStreamCancelIo};

const PIPE_BUFFER_BYTES: u32 = 64 * 1024;
const PIPE_MAX_INSTANCES: u32 = 255;
const OWNER_ONLY_SDDL: &str = "D:P(A;;GA;;;OW)";

pub(crate) struct AgentEndpoint {
    pipe_name: String,
    pending: Option<File>,
    discovery_path: PathBuf,
    discovery_published: bool,
}

#[derive(Clone)]
pub(crate) struct AgentEndpointWake {
    pipe_name: String,
}

impl AgentEndpoint {
    pub(crate) fn bind(process_id: u32, nonce: &str) -> io::Result<Self> {
        let pipe_name = format!(r"\\.\pipe\uix-agent-{process_id}-{nonce}");
        let pending = create_pipe_instance(&pipe_name, true)?;
        let directory = private_discovery_directory()?;
        let discovery_path = directory.join(format!("uix-{process_id}.json"));
        Ok(Self {
            pipe_name,
            pending: Some(pending),
            discovery_path,
            discovery_published: false,
        })
    }

    pub(crate) fn endpoint_name(&self) -> String {
        self.pipe_name.clone()
    }

    pub(crate) fn discovery_path(&self) -> &Path {
        &self.discovery_path
    }

    pub(crate) fn waker(&self) -> AgentEndpointWake {
        AgentEndpointWake {
            pipe_name: self.pipe_name.clone(),
        }
    }

    pub(crate) fn publish_discovery(&mut self, contents: &[u8], nonce: &str) -> io::Result<()> {
        let temporary = self
            .discovery_path
            .with_extension(format!("json.tmp-{nonce}"));
        let mut file = create_owner_only_file(&temporary)?;
        let write_result = (|| {
            file.write_all(contents)?;
            file.sync_all()
        })();
        drop(file);
        let result = write_result.and_then(|()| {
            if self.discovery_path.exists() {
                fs::remove_file(&self.discovery_path)?;
            }
            fs::rename(&temporary, &self.discovery_path)
        });
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        } else {
            self.discovery_published = true;
        }
        result
    }

    pub(crate) fn accept(&mut self) -> io::Result<AcceptedAgentStream> {
        let connected = self
            .pending
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "pipe listener closed"))?;
        connect_pipe(&connected)?;
        self.pending = Some(create_pipe_instance(&self.pipe_name, false)?);
        let cancel = WindowsStreamCancel {
            worker_handle: Mutex::new(None),
            cancelled: AtomicBool::new(false),
        };
        Ok(AcceptedAgentStream {
            stream: Box::new(connected),
            cancel: Arc::new(cancel),
        })
    }
}

impl AgentEndpointWake {
    pub(crate) fn wake(&self) {
        let wide = wide_null(self.pipe_name.as_ref());
        unsafe {
            // SAFETY: the UTF-16 name is NUL terminated and remains live for
            // the duration of the bounded kernel wait.
            let _ = WaitNamedPipeW(PCWSTR(wide.as_ptr()), 1_000);
        }
        let _ = OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&self.pipe_name);
    }
}

struct WindowsStreamCancel {
    worker_handle: Mutex<Option<usize>>,
    cancelled: AtomicBool,
}

impl AgentStreamCancelIo for WindowsStreamCancel {
    fn bind_worker(&self, worker: &std::thread::JoinHandle<()>) {
        use std::os::windows::io::AsRawHandle as _;

        *self
            .worker_handle
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(worker.as_raw_handle() as usize);
        if self.cancelled.load(Ordering::Acquire) {
            self.cancel_worker();
        }
    }

    fn cancel(&self) {
        if self.cancelled.swap(true, Ordering::AcqRel) {
            return;
        }
        self.cancel_worker();
    }
}

impl WindowsStreamCancel {
    fn cancel_worker(&self) {
        let worker_handle = *self
            .worker_handle
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(worker_handle) = worker_handle {
            unsafe {
                // SAFETY: the listener retains the JoinHandle until after all
                // connection cancellation completes, so this borrowed native
                // thread handle remains live for the synchronous cancel call.
                let _ = CancelSynchronousIo(windows::Win32::Foundation::HANDLE(
                    worker_handle as *mut std::ffi::c_void,
                ));
            }
        }
    }
}

impl Drop for AgentEndpoint {
    fn drop(&mut self) {
        if self.discovery_published {
            let _ = fs::remove_file(&self.discovery_path);
        }
    }
}

pub(crate) fn fill_secure_random(output: &mut [u8]) -> io::Result<()> {
    let status = unsafe {
        // SAFETY: BCrypt writes exactly within the mutable output slice and no
        // algorithm handle is required with SYSTEM_PREFERRED_RNG.
        BCryptGenRandom(None, output, BCRYPT_USE_SYSTEM_PREFERRED_RNG)
    };
    if status.is_ok() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "BCryptGenRandom failed with status 0x{:08x}",
                status.0 as u32
            ),
        ))
    }
}

#[cfg(test)]
pub(super) fn connect_for_test(endpoint: &str) -> io::Result<super::AgentStream> {
    Ok(Box::new(
        OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(endpoint)?,
    ))
}

fn create_pipe_instance(pipe_name: &str, first: bool) -> io::Result<File> {
    let security = OwnerOnlySecurity::new()?;
    let attributes = security.attributes();
    let wide = wide_null(pipe_name.as_ref());
    let mut open_mode = PIPE_ACCESS_DUPLEX;
    if first {
        open_mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
    }
    let pipe_mode = NAMED_PIPE_MODE(
        PIPE_TYPE_BYTE.0 | PIPE_READMODE_BYTE.0 | PIPE_WAIT.0 | PIPE_REJECT_REMOTE_CLIENTS.0,
    );
    let handle = unsafe {
        // SAFETY: all pointers are valid for this synchronous call. Windows
        // copies the supplied security descriptor into the new pipe object.
        CreateNamedPipeW(
            PCWSTR(wide.as_ptr()),
            open_mode,
            pipe_mode,
            PIPE_MAX_INSTANCES,
            PIPE_BUFFER_BYTES,
            PIPE_BUFFER_BYTES,
            0,
            Some(&attributes),
        )
    };
    if handle.is_invalid() {
        return Err(io::Error::last_os_error());
    }
    let file = unsafe {
        // SAFETY: `handle` is a newly owned file handle and ownership is
        // transferred exactly once to `File`.
        File::from_raw_handle(handle.0 as _)
    };
    Ok(file)
}

fn connect_pipe(pipe: &File) -> io::Result<()> {
    let handle = windows::Win32::Foundation::HANDLE(pipe.as_raw_handle());
    match unsafe {
        // SAFETY: the handle owns a live named-pipe server instance and this
        // thread performs the only synchronous connect operation on it.
        ConnectNamedPipe(handle, None)
    } {
        Ok(()) => Ok(()),
        Err(error) if error.code() == HRESULT::from_win32(ERROR_PIPE_CONNECTED.0) => Ok(()),
        Err(error) => Err(windows_error(error)),
    }
}

fn private_discovery_directory() -> io::Result<PathBuf> {
    let local_app_data = env::var_os("LOCALAPPDATA").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "LOCALAPPDATA is unavailable for agent discovery",
        )
    })?;
    let directory = PathBuf::from(local_app_data).join("uix-agent");
    match fs::symlink_metadata(&directory) {
        Ok(metadata) if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "agent discovery path is not a private directory",
            ))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let security = OwnerOnlySecurity::new()?;
            let attributes = security.attributes();
            let wide = wide_null(directory.as_os_str());
            match unsafe {
                // SAFETY: the path and security attributes are valid for this
                // synchronous call; Windows copies the descriptor.
                CreateDirectoryW(PCWSTR(wide.as_ptr()), Some(&attributes))
            } {
                Ok(()) => {}
                Err(error) if error.code() == HRESULT::from_win32(ERROR_ALREADY_EXISTS.0) => {}
                Err(error) => return Err(windows_error(error)),
            }
        }
        Err(error) => return Err(error),
    }
    Ok(directory)
}

fn create_owner_only_file(path: &Path) -> io::Result<File> {
    let security = OwnerOnlySecurity::new()?;
    let attributes = security.attributes();
    let wide = wide_null(path.as_os_str());
    let handle = unsafe {
        // SAFETY: the path is NUL terminated, the security attributes live
        // through the call, and the returned handle is checked before use.
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_MODE(0),
            Some(&attributes),
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }
    .map_err(windows_error)?;
    Ok(unsafe {
        // SAFETY: this successful CreateFileW call returned one owned handle,
        // which is transferred exactly once to `File`.
        File::from_raw_handle(handle.0 as _)
    })
}

struct OwnerOnlySecurity {
    descriptor: PSECURITY_DESCRIPTOR,
}

impl OwnerOnlySecurity {
    fn new() -> io::Result<Self> {
        let sddl = wide_null(OWNER_ONLY_SDDL.as_ref());
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            // SAFETY: the SDDL string is NUL terminated and the output pointer
            // refers to initialized storage. The returned LocalAlloc buffer is
            // retained by this wrapper and freed exactly once in Drop.
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        }
        .map_err(windows_error)?;
        Ok(Self { descriptor })
    }

    fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.descriptor.0,
            bInheritHandle: BOOL(0),
        }
    }
}

impl Drop for OwnerOnlySecurity {
    fn drop(&mut self) {
        if !self.descriptor.0.is_null() {
            unsafe {
                // SAFETY: this pointer came from the SDDL conversion function
                // and has not been freed or transferred elsewhere.
                let _ = LocalFree(Some(HLOCAL(self.descriptor.0)));
            }
        }
    }
}

fn wide_null(value: &std::ffi::OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

fn windows_error(error: windows::core::Error) -> io::Error {
    let raw = (error.code().0 & 0xffff) as i32;
    io::Error::from_raw_os_error(raw)
}
