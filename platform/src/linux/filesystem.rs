// ============================================================================
// platform/linux/filesystem.rs — Linux filesystem implementation (IFileSystem)
// ============================================================================

use uix_diag::{Errc, Error};
use crate::types::SpecialDir;
use crate::IFileSystem;

// ════════════════════════════════════════════════════════════════════════════
// LinuxFileSystem
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct LinuxFileSystem;

impl LinuxFileSystem {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl IFileSystem for LinuxFileSystem {
    fn get_special_dir(&self, dir: SpecialDir) -> String {
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
                    // Respect freedesktop.org XDG_DESKTOP_DIR
                    Self::xdg_user_dir("DESKTOP")
                        .unwrap_or_else(|| format!("{}/Desktop", home))
                }
            }
            SpecialDir::Downloads => {
                let home = std::env::var("HOME").unwrap_or_default();
                if home.is_empty() {
                    String::new()
                } else {
                    Self::xdg_user_dir("DOWNLOAD")
                        .unwrap_or_else(|| format!("{}/Downloads", home))
                }
            }
            SpecialDir::Current => std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            SpecialDir::Executable => self.executable_dir(),
        }
    }

    fn executable_path(&self) -> String {
        std::env::current_exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    fn executable_dir(&self) -> String {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_string_lossy().to_string()))
            .unwrap_or_default()
    }

    fn read_file(&self, path: &str) -> Result<Vec<u8>, Error> {
        std::fs::read(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::new(
                    Errc::FileNotFound,
                    format!("cannot read file '{}': {}", path, e),
                )
            } else {
                Error::new(
                    Errc::ReadFailure,
                    format!("cannot read file '{}': {}", path, e),
                )
            }
        })
    }
}

// ════════════════════════════════════════════════════════════════════════════
// XDG user dirs support
// ════════════════════════════════════════════════════════════════════════════

impl LinuxFileSystem {
    /// Read the XDG user dirs config file (~/.config/user-dirs.dirs) and
    /// return the directory for the given key (e.g. "DESKTOP", "DOWNLOAD").
    fn xdg_user_dir(key: &str) -> Option<String> {
        let home = std::env::var("HOME").ok()?;
        let config_file = format!("{}/.config/user-dirs.dirs", home);
        let content = std::fs::read_to_string(config_file).ok()?;

        let pattern = format!("XDG_{}_DIR=\"$HOME/", key);
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with(&pattern) && line.ends_with('"') {
                let start = pattern.len();
                let end = line.len() - 1; // skip trailing "
                if end > start {
                    return Some(format!("{}/{}", home, &line[start..end]));
                }
            }
            // Also handle non-variable paths
            let alt_pattern = format!("XDG_{}_DIR=\"", key);
            if line.starts_with(&alt_pattern) && line.ends_with('"') {
                let start = alt_pattern.len();
                let end = line.len() - 1;
                if end > start {
                    let path = &line[start..end];
                    // Resolve $HOME if present
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
