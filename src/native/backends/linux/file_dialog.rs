// ============================================================================
// platform/linux/file_dialog.rs — Linux file dialog (IFileDialog)
// ============================================================================
//
// Uses `zenity` (GNOME) or `kdialog` (KDE) for native file dialogs.
// Detection: prefer KDE_DESKTOP_SESSION / DESKTOP_SESSION env vars.
// Falls back to zenity if neither is detected or available.
// ============================================================================

use crate::native::IFileDialog;

// ════════════════════════════════════════════════════════════════════════════
// LinuxFileDialog
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct LinuxFileDialog;

impl LinuxFileDialog {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxFileDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl IFileDialog for LinuxFileDialog {
    fn open(&mut self, title: &str, filters: &str) -> Vec<String> {
        if use_kde() {
            kde_open_file(title, filters)
        } else {
            zenity_open_file(title, filters)
        }
    }

    fn save(&mut self, title: &str, filters: &str) -> String {
        if use_kde() {
            kde_save_file(title, filters)
        } else {
            zenity_save_file(title, filters)
        }
    }

    fn open_folder(&mut self, title: &str) -> String {
        if use_kde() {
            kde_open_folder(title)
        } else {
            zenity_open_folder(title)
        }
    }
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

fn zenity_open_file(title: &str, filters: &str) -> Vec<String> {
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

    let output = match cmd.output() {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    if !output.status.success() {
        return Vec::new();
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

fn zenity_save_file(title: &str, filters: &str) -> String {
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

    let output = match cmd.output() {
        Ok(o) => o,
        Err(_) => return String::new(),
    };
    if !output.status.success() {
        return String::new();
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.trim().to_string()
}

fn zenity_open_folder(title: &str) -> String {
    let output = match std::process::Command::new("zenity")
        .args(["--file-selection", "--title", title, "--directory"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return String::new(),
    };
    if !output.status.success() {
        return String::new();
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.trim().to_string()
}

// ════════════════════════════════════════════════════════════════════════════
// KDialog (KDE)
// ════════════════════════════════════════════════════════════════════════════

fn kde_open_file(title: &str, filters: &str) -> Vec<String> {
    let output = match std::process::Command::new("kdialog")
        .args(["--title", title, "--getopenfilename", ".", filters])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    if !output.status.success() {
        return Vec::new();
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

fn kde_save_file(title: &str, filters: &str) -> String {
    let output = match std::process::Command::new("kdialog")
        .args(["--title", title, "--getsavefilename", ".", filters])
        .output()
    {
        Ok(o) => o,
        Err(_) => return String::new(),
    };
    if !output.status.success() {
        return String::new();
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.trim().to_string()
}

fn kde_open_folder(title: &str) -> String {
    let output = match std::process::Command::new("kdialog")
        .args(["--title", title, "--getexistingdirectory", "."])
        .output()
    {
        Ok(o) => o,
        Err(_) => return String::new(),
    };
    if !output.status.success() {
        return String::new();
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.trim().to_string()
}
