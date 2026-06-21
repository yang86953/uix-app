// ============================================================================
// platform/linux/notification.rs — Linux notification (INotification)
// ============================================================================
//
// Uses the `notify-send` command (freedesktop.org Desktop Notifications
// Specification). Falls back silently if notify-send is not available.
// ============================================================================

use crate::INotification;
use std::process::Command;

// ════════════════════════════════════════════════════════════════════════════
// LinuxNotification
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct LinuxNotification;

impl LinuxNotification {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxNotification {
    fn default() -> Self {
        Self::new()
    }
}

impl INotification for LinuxNotification {
    fn show(&mut self, title: &str, message: &str) {
        // notify-send is the standard freedesktop notification tool.
        // It is available on most Linux desktop environments.
        let _ = Command::new("notify-send")
            .arg(title)
            .arg(message)
            .arg("--app-name")
            .arg("UIX")
            .output();
    }
}
