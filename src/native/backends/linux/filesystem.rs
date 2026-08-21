// ============================================================================
// platform/linux/filesystem.rs — Linux 特殊目录解析
// ============================================================================

use crate::native::{Errc, Error, Result};
use crate::platform::system::filesystem::{FileSystemCore, SpecialDir, SpecialDirProvider};

// ════════════════════════════════════════════════════════════════════════════
// LinuxFileSystem
// ════════════════════════════════════════════════════════════════════════════

pub(crate) type LinuxFileSystem = FileSystemCore<LinuxSpecialDirs>;

#[derive(Debug, Clone, Default)]
pub(crate) struct LinuxSpecialDirs;

impl SpecialDirProvider for LinuxSpecialDirs {
    fn special_dir(&self, dir: SpecialDir) -> Result<String> {
        match dir {
            SpecialDir::Home => home_dir()
                .ok_or_else(|| Error::new(Errc::NotFound, "LinuxSpecialDirs: HOME is not set")),
            SpecialDir::Temp => Ok(std::env::var("TMPDIR")
                .or_else(|_| std::env::var("TEMP"))
                .unwrap_or_else(|_| "/tmp".to_string())),
            SpecialDir::AppData => Self::home_joined(".config", dir),
            SpecialDir::LocalAppData => Self::home_joined(".local/share", dir),
            SpecialDir::Documents => Self::home_joined("Documents", dir),
            SpecialDir::Desktop => {
                let home = home_dir().ok_or_else(|| {
                    Error::new(Errc::NotFound, "LinuxSpecialDirs: HOME is not set")
                })?;
                // 优先遵循 freedesktop.org XDG_DESKTOP_DIR。
                Ok(Self::xdg_user_dir("DESKTOP").unwrap_or_else(|| format!("{home}/Desktop")))
            }
            SpecialDir::Downloads => {
                let home = home_dir().ok_or_else(|| {
                    Error::new(Errc::NotFound, "LinuxSpecialDirs: HOME is not set")
                })?;
                Ok(Self::xdg_user_dir("DOWNLOAD").unwrap_or_else(|| format!("{home}/Downloads")))
            }
            SpecialDir::Current | SpecialDir::Executable => Err(Error::new(
                Errc::NotImplemented,
                "LinuxSpecialDirs: Current/Executable are handled by FileSystemCore",
            )),
        }
    }
}

fn home_dir() -> Option<String> {
    std::env::var("HOME").ok()
}

// ════════════════════════════════════════════════════════════════════════════
// XDG 用户目录支持
// ════════════════════════════════════════════════════════════════════════════

impl LinuxSpecialDirs {
    /// 以 HOME 为基座拼接子目录;HOME 缺失视为解析失败。
    fn home_joined(child: &str, dir: SpecialDir) -> Result<String> {
        let home = home_dir().ok_or_else(|| {
            Error::new(
                Errc::NotFound,
                format!("LinuxSpecialDirs: HOME is not set for {dir:?}"),
            )
        })?;
        Ok(format!("{home}/{child}"))
    }

    /// 读取 XDG 用户目录配置，返回指定键对应的目录。
    fn xdg_user_dir(key: &str) -> Option<String> {
        let home = std::env::var("HOME").ok()?;
        let config_file = format!("{}/.config/user-dirs.dirs", home);
        let content = std::fs::read_to_string(config_file).ok()?;

        let pattern = format!("XDG_{}_DIR=\"$HOME/", key);
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with(&pattern) && line.ends_with('"') {
                let start = pattern.len();
                let end = line.len() - 1; // 跳过结尾引号
                if end > start {
                    return Some(format!("{}/{}", home, &line[start..end]));
                }
            }
            // 同时处理不使用变量的绝对路径。
            let alt_pattern = format!("XDG_{}_DIR=\"", key);
            if line.starts_with(&alt_pattern) && line.ends_with('"') {
                let start = alt_pattern.len();
                let end = line.len() - 1;
                if end > start {
                    let path = &line[start..end];
                    // 如路径包含 $HOME，则解析为实际 home 目录。
                    if path.starts_with("$HOME/") {
                        return Some(format!("{}/{}", home, &path[6..]));
                    }
                    return Some(path.to_string());
                }
            }
        }
        None
    }
}
