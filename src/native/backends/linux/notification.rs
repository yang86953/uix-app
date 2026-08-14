// ============================================================================
// platform/linux/notification.rs — Linux notification (INotification)
// ============================================================================
//
// Uses the `notify-send` command (freedesktop.org Desktop Notifications
// Specification). Falls back silently if notify-send is not available.
// ============================================================================

use crate::core::error::Errc;
use crate::core::error::{Error, Result};
use crate::native::capabilities::system::INotification;
use std::process::Command;

// ════════════════════════════════════════════════════════════════════════════
// LinuxNotification
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug)]
pub(crate) struct LinuxNotification;

impl LinuxNotification {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for LinuxNotification {
    fn default() -> Self {
        Self::new()
    }
}

impl INotification for LinuxNotification {
    fn show(&mut self, title: &str, message: &str) -> Result<()> {
        // notify-send is the standard freedesktop notification tool.
        // It is available on most Linux desktop environments.
        let output = Command::new("notify-send")
            .arg(title)
            .arg(message)
            .arg("--app-name")
            .arg("UIX")
            .output()
            .map_err(|error| {
                Error::new(
                    Errc::PlatformError,
                    format!("LinuxNotification::show: notify-send spawn failed: {error}"),
                )
            })?;
        if !output.status.success() {
            return Err(Error::new(
                Errc::PlatformError,
                format!(
                    "LinuxNotification::show: notify-send exited with {}",
                    output.status
                ),
            ));
        }
        Ok(())
    }
}
