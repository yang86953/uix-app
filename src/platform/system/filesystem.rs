//! Platform System 的中立文件系统合同与共享核心。

use crate::core::{Errc, Error, Result};

/// 操作系统或当前进程拥有的特殊目录。
///
/// 本枚举同时服务公开 Platform facade 与内部低层文件系统端口；所有变体都表示
/// 可直接访问的目录位置，不承载目录创建或生命周期策略。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecialDir {
    /// 当前用户的主目录或配置文件目录。
    Home,
    /// 操作系统提供的临时文件目录。
    Temp,
    /// 平台约定的应用配置数据目录。
    AppData,
    /// 平台约定的设备本地应用数据目录。
    LocalAppData,
    /// 当前用户的文档目录。
    Documents,
    /// 当前用户的桌面目录。
    Desktop,
    /// 当前用户的下载目录。
    Downloads,
    /// 当前进程的工作目录。
    Current,
    /// 当前可执行文件所在目录。
    Executable,
}

/// Platform 根持有的同步文件系统端口。
pub(crate) trait IFileSystem {
    fn get_special_dir(&self, dir: SpecialDir) -> Result<String>;
    fn executable_path(&self) -> Result<String>;
    fn executable_dir(&self) -> Result<String>;
    fn read_file(&self, path: &str) -> Result<Vec<u8>, Error>;
}

/// 三平台特殊目录解析器实现的窄端口。
pub(crate) trait SpecialDirProvider {
    fn special_dir(&self, dir: SpecialDir) -> Result<String>;
}

/// 复用进程目录与文件读取语义的共享文件系统核心。
#[derive(Debug, Clone)]
pub(crate) struct FileSystemCore<P> {
    provider: P,
}

impl<P: Default> FileSystemCore<P> {
    pub(crate) fn new() -> Self {
        Self {
            provider: P::default(),
        }
    }
}

// 保留自定义 provider 注入与读取接口，供平台扩展和外部组装测试使用。
#[allow(dead_code)]
impl<P> FileSystemCore<P> {
    pub(crate) fn with_provider(provider: P) -> Self {
        Self { provider }
    }

    pub(crate) fn provider(&self) -> &P {
        &self.provider
    }
}

impl<P: Default> Default for FileSystemCore<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: SpecialDirProvider> IFileSystem for FileSystemCore<P> {
    fn get_special_dir(&self, dir: SpecialDir) -> Result<String> {
        match dir {
            SpecialDir::Current => current_dir(),
            SpecialDir::Executable => self.executable_dir(),
            _ => self.provider.special_dir(dir),
        }
    }

    fn executable_path(&self) -> Result<String> {
        executable_path()
    }

    fn executable_dir(&self) -> Result<String> {
        let path = std::env::current_exe().map_err(|err| {
            Error::new(
                Errc::IoError,
                format!("FileSystemCore::executable_dir: current_exe failed: {err}"),
            )
        })?;
        path.parent()
            .map(|dir| dir.to_string_lossy().to_string())
            .ok_or_else(|| {
                Error::new(
                    Errc::PlatformError,
                    "FileSystemCore::executable_dir: current_exe has no parent",
                )
            })
    }

    fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        if path.is_empty() {
            return Err(Error::new(
                Errc::InvalidArgument,
                "FileSystemCore::read_file: path is empty",
            ));
        }

        std::fs::read(path).map_err(|err| read_error(path, err))
    }
}

fn current_dir() -> Result<String> {
    std::env::current_dir()
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|err| {
            Error::new(
                Errc::IoError,
                format!("FileSystemCore::current_dir failed: {err}"),
            )
        })
}

fn executable_path() -> Result<String> {
    std::env::current_exe()
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|err| {
            Error::new(
                Errc::IoError,
                format!("FileSystemCore::executable_path failed: {err}"),
            )
        })
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
