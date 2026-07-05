// ============================================================================
// platform/linux/filesystem.rs — Linux 特殊目录解析
// ============================================================================

use crate::platform::shared::{FileSystemCore, SpecialDirProvider};
use crate::platform::api::system::SpecialDir;

// ════════════════════════════════════════════════════════════════════════════
// LinuxFileSystem
// ════════════════════════════════════════════════════════════════════════════

pub type LinuxFileSystem = FileSystemCore<LinuxSpecialDirs>;

#[derive(Debug, Clone, Default)]
pub struct LinuxSpecialDirs;

impl SpecialDirProvider for LinuxSpecialDirs {
    fn special_dir(&self, dir: SpecialDir) -> String {
        match dir {
            SpecialDir::Home => std::env::var("HOME").unwrap_or_default(),
            SpecialDir::Temp => std::env::var("TMPDIR")
                .or_else(|_| std::env::var("TEMP"))
                .unwrap_or_else(|_| "/tmp".to_string()),
            SpecialDir::AppData => {
                let home = std::env::var("HOME").unwrap_or_default();
                if home.is_empty() {
                    String::new()
                } else {
                    format!("{}/.config", home)
                }
            }
            SpecialDir::LocalAppData => {
                let home = std::env::var("HOME").unwrap_or_default();
                if home.is_empty() {
                    String::new()
                } else {
                    format!("{}/.local/share", home)
                }
            }
            SpecialDir::Documents => {
                let home = std::env::var("HOME").unwrap_or_default();
                if home.is_empty() {
                    String::new()
                } else {
                    format!("{}/Documents", home)
                }
            }
            SpecialDir::Desktop => {
                let home = std::env::var("HOME").unwrap_or_default();
                if home.is_empty() {
                    String::new()
                } else {
                    // 优先遵循 freedesktop.org XDG_DESKTOP_DIR。
                    Self::xdg_user_dir("DESKTOP").unwrap_or_else(|| format!("{}/Desktop", home))
                }
            }
            SpecialDir::Downloads => {
                let home = std::env::var("HOME").unwrap_or_default();
                if home.is_empty() {
                    String::new()
                } else {
                    Self::xdg_user_dir("DOWNLOAD").unwrap_or_else(|| format!("{}/Downloads", home))
                }
            }
            SpecialDir::Current | SpecialDir::Executable => String::new(),
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// XDG 用户目录支持
// ════════════════════════════════════════════════════════════════════════════

impl LinuxSpecialDirs {
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
