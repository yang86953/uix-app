// ============================================================================
// platform/shared/filesystem.rs — 文件系统共享核心
//
// 共享部分负责当前目录、可执行文件路径与读文件错误映射。
// 平台只实现 SpecialDirProvider，提供真正有平台差异的特殊目录解析。
// ============================================================================

use crate::platform::api::system::IFileSystem;
use crate::platform::api::system::SpecialDir;
use crate::platform::{Errc, Error};

/// 平台特殊目录解析接口。
pub trait SpecialDirProvider {
    fn special_dir(&self, dir: SpecialDir) -> String;
}

/// 文件系统共享实现。
#[derive(Debug, Clone)]
pub struct FileSystemCore<P> {
    provider: P,
}

impl<P: Default> FileSystemCore<P> {
    pub fn new() -> Self {
        Self {
            provider: P::default(),
        }
    }
}

impl<P> FileSystemCore<P> {
    pub fn with_provider(provider: P) -> Self {
        Self { provider }
    }

    pub fn provider(&self) -> &P {
        &self.provider
    }
}

impl<P: Default> Default for FileSystemCore<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: SpecialDirProvider> IFileSystem for FileSystemCore<P> {
    fn get_special_dir(&self, dir: SpecialDir) -> String {
        match dir {
            SpecialDir::Current => current_dir(),
            SpecialDir::Executable => self.executable_dir(),
            _ => self.provider.special_dir(dir),
        }
    }

    fn executable_path(&self) -> String {
        executable_path()
    }

    fn executable_dir(&self) -> String {
        std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(|dir| dir.to_string_lossy().to_string()))
            .unwrap_or_default()
    }

    fn read_file(&self, path: &str) -> Result<Vec<u8>, Error> {
        if path.is_empty() {
            return Err(Error::new(
                Errc::InvalidArgument,
                "FileSystemCore::read_file: path is empty",
            ));
        }

        std::fs::read(path).map_err(|err| read_error(path, err))
    }
}

fn current_dir() -> String {
    std::env::current_dir()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn executable_path() -> String {
    std::env::current_exe()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn read_error(path: &str, err: std::io::Error) -> Error {
    let code = match err.kind() {
        std::io::ErrorKind::NotFound => Errc::FileNotFound,
        std::io::ErrorKind::PermissionDenied => Errc::AccessDenied,
        _ => Errc::ReadFailure,
    };
    Error::new(
        code,
        format!(
            "FileSystemCore::read_file: cannot read file '{}': {}",
            path, err
        ),
    )
}
