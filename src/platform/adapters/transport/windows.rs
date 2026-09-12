use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_INSUFFICIENT_BUFFER, ERROR_PIPE_CONNECTED,
    GENERIC_READ, GENERIC_WRITE, HANDLE, HLOCAL, LocalFree,
};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom};
use windows::Win32::Security::{
    DACL_SECURITY_INFORMATION, GetTokenInformation, PROTECTED_DACL_SECURITY_INFORMATION,
    PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, SetFileSecurityW, TOKEN_QUERY, TOKEN_USER,
    TokenUser,
};
use windows::Win32::Storage::FileSystem::MoveFileExW;
use windows::Win32::Storage::FileSystem::{
    CREATE_NEW, CreateDirectoryW, CreateFileW, FILE_ATTRIBUTE_NORMAL,
    FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_SHARE_MODE, MOVEFILE_REPLACE_EXISTING,
    MOVEFILE_WRITE_THROUGH, PIPE_ACCESS_DUPLEX,
};
use windows::Win32::System::IO::CancelSynchronousIo;
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, NAMED_PIPE_MODE, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT, WaitNamedPipeW,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::{BOOL, HRESULT, PCWSTR, PWSTR};

use super::{AcceptedAgentStream, AgentStreamCancelIo};

const PIPE_BUFFER_BYTES: u32 = 64 * 1024;
const PIPE_MAX_INSTANCES: u32 = 255;

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
        let mut file = create_current_user_only_file(&temporary)?;
        let write_result = (|| {
            file.write_all(contents)?;
            file.sync_all()
        })();
        drop(file);
        let result =
            write_result.and_then(|()| replace_file_atomically(&temporary, &self.discovery_path));
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
        Err(io::Error::other(format!(
            "BCryptGenRandom failed with status 0x{:08x}",
            status.0 as u32
        )))
    }
}

/// 返回当前用户唯一的 Hub 命名管道；SID 同时避免不同登录用户争用名称。
pub(crate) fn agent_hub_endpoint_name() -> io::Result<String> {
    Ok(format!(
        r"\\.\pipe\uix-agent-hub-{}",
        current_process_user_sid_string()?
    ))
}

/// 建立应用到 Hub 或测试端点的同机字节流。
pub(crate) fn connect(endpoint: &str) -> io::Result<super::AgentStream> {
    Ok(Box::new(
        OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(endpoint)?,
    ))
}

fn create_pipe_instance(pipe_name: &str, first: bool) -> io::Result<File> {
    let security = CurrentUserOnlySecurity::new()?;
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
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let security = CurrentUserOnlySecurity::new()?;
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
    apply_current_user_only_dacl(&directory)?;
    Ok(directory)
}

fn replace_file_atomically(source: &Path, destination: &Path) -> io::Result<()> {
    let source = wide_null(source.as_os_str());
    let destination = wide_null(destination.as_os_str());
    unsafe {
        // SAFETY: 两个路径均为存活的 NUL 结尾 UTF-16；同目录替换不跨卷。
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(windows_error)
}

fn apply_current_user_only_dacl(path: &Path) -> io::Result<()> {
    let security = CurrentUserOnlySecurity::new()?;
    let wide = wide_null(path.as_os_str());
    unsafe {
        // SAFETY: 路径与安全描述符在同步调用期间有效；只替换并保护 DACL，不改所有者。
        SetFileSecurityW(
            PCWSTR(wide.as_ptr()),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            security.descriptor,
        )
    }
    .ok()
    .map_err(windows_error)
}

fn create_current_user_only_file(path: &Path) -> io::Result<File> {
    let security = CurrentUserOnlySecurity::new()?;
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

struct ProcessToken(HANDLE);

impl ProcessToken {
    fn current() -> io::Result<Self> {
        let mut token = HANDLE::default();
        unsafe {
            // SAFETY: GetCurrentProcess 返回当前进程伪句柄；输出指针有效，成功后由本对象接管 token。
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
        }
        .map_err(windows_error)?;
        if token.is_invalid() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "OpenProcessToken returned an invalid handle",
            ));
        }
        Ok(Self(token))
    }
}

impl Drop for ProcessToken {
    fn drop(&mut self) {
        unsafe {
            // SAFETY: 句柄由 OpenProcessToken 成功返回，所有权未转移且只在此关闭一次。
            let _ = CloseHandle(self.0);
        }
    }
}

fn current_process_user_sid_string() -> io::Result<String> {
    let token = ProcessToken::current()?;
    let mut required = 0u32;
    let size_query = unsafe {
        // SAFETY: token 有 TOKEN_QUERY；空缓冲用于查询 TokenUser 所需字节数。
        GetTokenInformation(token.0, TokenUser, None, 0, &mut required)
    };
    match size_query {
        Ok(()) if required == 0 => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "TokenUser size query returned zero bytes",
            ));
        }
        Ok(()) => {}
        Err(error)
            if error.code() == HRESULT::from_win32(ERROR_INSUFFICIENT_BUFFER.0) && required > 0 => {
        }
        Err(error) => return Err(windows_error(error)),
    }
    if required < std::mem::size_of::<TOKEN_USER>() as u32 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "TokenUser buffer is smaller than TOKEN_USER",
        ));
    }

    let word_bytes = std::mem::size_of::<usize>();
    let word_count = (required as usize).div_ceil(word_bytes);
    let mut buffer = vec![0usize; word_count];
    let buffer_bytes = u32::try_from(buffer.len().saturating_mul(word_bytes)).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "TokenUser buffer length overflows Win32 DWORD",
        )
    })?;
    let mut written = 0u32;
    unsafe {
        // SAFETY: 缓冲按 usize 对齐且容量至少为 required；token 在调用期间有效。
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            buffer_bytes,
            &mut written,
        )
    }
    .map_err(windows_error)?;
    if written < std::mem::size_of::<TOKEN_USER>() as u32 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "TokenUser query returned a truncated structure",
        ));
    }
    let token_user = unsafe {
        // SAFETY: GetTokenInformation 已在对齐且足够大的缓冲中写入完整 TOKEN_USER。
        &*buffer.as_ptr().cast::<TOKEN_USER>()
    };
    if token_user.User.Sid.is_invalid() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "TokenUser query returned an invalid SID",
        ));
    }

    let mut sid_text = PWSTR::null();
    unsafe {
        // SAFETY: SID 指向仍存活的 TokenUser 缓冲；输出指针有效并接收 LocalAlloc 字符串。
        ConvertSidToStringSidW(token_user.User.Sid, &mut sid_text)
    }
    .map_err(windows_error)?;
    if sid_text.0.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "ConvertSidToStringSidW returned a null string",
        ));
    }
    let result = unsafe {
        // SAFETY: ConvertSidToStringSidW 返回存活且 NUL 结尾的 UTF-16 字符串。
        sid_text.to_string()
    }
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error));
    unsafe {
        // SAFETY: 字符串由 ConvertSidToStringSidW 分配，尚未转移且只在此释放一次。
        let _ = LocalFree(Some(HLOCAL(sid_text.0.cast())));
    }
    result
}

struct CurrentUserOnlySecurity {
    descriptor: PSECURITY_DESCRIPTOR,
}

impl CurrentUserOnlySecurity {
    fn new() -> io::Result<Self> {
        let user_sid = current_process_user_sid_string()?;
        let sddl = wide_null(format!("D:P(A;;GA;;;{user_sid})").as_ref());
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            // SAFETY: SDDL 为存活的 NUL 结尾 UTF-16，输出指针指向已初始化存储；
            // 返回的 LocalAlloc 缓冲由本对象持有并在 Drop 中释放一次。
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

impl Drop for CurrentUserOnlySecurity {
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
    let raw = error.code().0 & 0xffff;
    io::Error::from_raw_os_error(raw)
}

// Windows 权限验收辅助（Authz 访问检查与 SID/描述符封装）整体位于 tests-src，
// 仅测试构建编译；生产路径保留 wide_null/windows_error/current_process_user_sid_string。
#[cfg(test)]
#[path = "../../../../tests-src/platform/adapters/transport/windows_tests.rs"]
mod tests;
