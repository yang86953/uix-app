// ============================================================================
// platform/linux/file_dialog.rs — Linux file dialog (IFileDialog)
// ============================================================================
//
// Uses `zenity` (GNOME) or `kdialog` (KDE) for native file dialogs.
// Detection: prefer KDE_DESKTOP_SESSION / DESKTOP_SESSION env vars.
// Falls back to zenity if neither is detected or available.
// ============================================================================

use crate::native::capabilities::system::IFileDialog;
use crate::native::{Errc, Error, Result};

// ════════════════════════════════════════════════════════════════════════════
// LinuxFileDialog
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug)]
pub(crate) struct LinuxFileDialog;

impl LinuxFileDialog {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for LinuxFileDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl IFileDialog for LinuxFileDialog {
    fn open(&mut self, title: &str, filters: &str) -> Result<Option<Vec<String>>> {
        if use_kde() {
            kde_open_file(title, filters)
        } else {
            zenity_open_file(title, filters)
        }
    }

    fn save(&mut self, title: &str, filters: &str) -> Result<Option<String>> {
        if use_kde() {
            kde_save_file(title, filters)
        } else {
            zenity_save_file(title, filters)
        }
    }

    fn open_folder(&mut self, title: &str) -> Result<Option<String>> {
        if use_kde() {
            kde_open_folder(title)
        } else {
            zenity_open_folder(title)
        }
    }
}

// 通过 Platform System 窄 Adapter 打开多选文件面板。
pub(crate) fn choose_files(title: &str, filters: &str) -> Result<Option<Vec<String>>> {
    // 每次同步调用创建无状态 Linux 对话框组件。
    let mut dialog = LinuxFileDialog::new();
    // 复用既有桌面环境探测与取消语义。
    dialog.open(title, filters)
}

// 通过 Platform System 窄 Adapter 打开保存面板。
pub(crate) fn choose_save_file(title: &str, filters: &str) -> Result<Option<String>> {
    // 每次同步调用创建无状态 Linux 对话框组件。
    let mut dialog = LinuxFileDialog::new();
    // 复用既有桌面环境探测与取消语义。
    dialog.save(title, filters)
}

// 通过 Platform System 窄 Adapter 打开目录面板。
pub(crate) fn choose_folder(title: &str) -> Result<Option<String>> {
    // 每次同步调用创建无状态 Linux 对话框组件。
    let mut dialog = LinuxFileDialog::new();
    // 复用既有桌面环境探测与取消语义。
    dialog.open_folder(title)
}

// ════════════════════════════════════════════════════════════════════════════
// Desktop environment detection
// ════════════════════════════════════════════════════════════════════════════

fn use_kde() -> bool {
    std::env::var("KDE_DESKTOP_SESSION").is_ok()
        || std::env::var("KDE_SESSION_VERSION").is_ok()
        || std::env::var("DESKTOP_SESSION")
            .map(|s| s.to_lowercase().contains("kde"))
            .unwrap_or(false)
}

// ════════════════════════════════════════════════════════════════════════════
// Zenity (GNOME / generic GTK)
// ════════════════════════════════════════════════════════════════════════════

fn zenity_open_file(title: &str, filters: &str) -> Result<Option<Vec<String>>> {
    let mut cmd = std::process::Command::new("zenity");
    cmd.args([
        "--file-selection",
        "--title",
        title,
        "--multiple",
        "--separator",
        "\n",
    ]);

    if !filters.is_empty() {
        for part in filters.split(';') {
            let part = part.trim();
            if !part.is_empty() {
                cmd.arg("--file-filter");
                cmd.arg(part);
            }
        }
    }

    let output = spawn_dialog("zenity", &mut cmd)?;
    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(Some(
        stdout
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
    ))
}

fn zenity_save_file(title: &str, filters: &str) -> Result<Option<String>> {
    let mut cmd = std::process::Command::new("zenity");
    cmd.args([
        "--file-selection",
        "--title",
        title,
        "--save",
        "--confirm-overwrite",
    ]);

    if !filters.is_empty() {
        for part in filters.split(';') {
            let part = part.trim();
            if !part.is_empty() {
                cmd.arg("--file-filter");
                cmd.arg(part);
            }
        }
    }

    let output = spawn_dialog("zenity", &mut cmd)?;
    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(Some(stdout.trim().to_string()))
}

fn zenity_open_folder(title: &str) -> Result<Option<String>> {
    let mut cmd = std::process::Command::new("zenity");
    cmd.args(["--file-selection", "--title", title, "--directory"]);
    let output = spawn_dialog("zenity", &mut cmd)?;
    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(Some(stdout.trim().to_string()))
}

// ════════════════════════════════════════════════════════════════════════════
// KDialog (KDE)
// ════════════════════════════════════════════════════════════════════════════

fn kde_open_file(title: &str, filters: &str) -> Result<Option<Vec<String>>> {
    let mut cmd = std::process::Command::new("kdialog");
    cmd.args(["--title", title, "--getopenfilename", ".", filters]);
    let output = spawn_dialog("kdialog", &mut cmd)?;
    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(Some(
        stdout
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
    ))
}

fn kde_save_file(title: &str, filters: &str) -> Result<Option<String>> {
    let mut cmd = std::process::Command::new("kdialog");
    cmd.args(["--title", title, "--getsavefilename", ".", filters]);
    let output = spawn_dialog("kdialog", &mut cmd)?;
    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(Some(stdout.trim().to_string()))
}

fn kde_open_folder(title: &str) -> Result<Option<String>> {
    let mut cmd = std::process::Command::new("kdialog");
    cmd.args(["--title", title, "--getexistingdirectory", "."]);
    let output = spawn_dialog("kdialog", &mut cmd)?;
    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(Some(stdout.trim().to_string()))
}

/// 启动对话框进程;进程无法启动(程序缺失等)视为对话框本身失败。
fn spawn_dialog(program: &str, cmd: &mut std::process::Command) -> Result<std::process::Output> {
    cmd.output().map_err(|err| {
        Error::new(
            Errc::IoError,
            format!("LinuxFileDialog: failed to launch {program}: {err}"),
        )
    })
}
