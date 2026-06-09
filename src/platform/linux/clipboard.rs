// ============================================================================
// platform/linux/clipboard.rs — Linux X11 clipboard (IClipboard)
// ============================================================================
//
// Implements the X11 selection-based clipboard (CLIPBOARD selection).
// Primary selection is not exposed through this interface.
//
// Uses the owner window from LinuxPlatform to claim/release selections.
// Selection requests from other applications are handled via the
// SelectionRequest event in the main event loop.
// ============================================================================

use crate::platform::IClipboard;

// ════════════════════════════════════════════════════════════════════════════
// LinuxClipboard
// ════════════════════════════════════════════════════════════════════════════
//
// NOTE: The actual X11 selection handling requires integration with the
// main event loop. This implementation provides the interface; the
// platform combines clipboard operations with the event dispatch.
//
// For reading clipboard content, we use XConvertSelection + a blocking
// wait (with a timeout). For writing, we claim ownership and store the
// text in an Arc<Mutex> for responding to SelectionRequest events.
// ============================================================================

use std::sync::{Arc, Mutex, PoisonError};

/// Shared clipboard state between LinuxPlatform and LinuxClipboard.
pub struct ClipboardState {
    /// The current clipboard text content that we own.
    pub text: String,
    /// Whether we currently own the clipboard selection.
    pub owns_clipboard: bool,
}

impl ClipboardState {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            owns_clipboard: false,
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// LinuxClipboard (lightweight accessor around shared state)
// ════════════════════════════════════════════════════════════════════════════

pub struct LinuxClipboard {
    state: Arc<Mutex<ClipboardState>>,
}

impl LinuxClipboard {
    pub fn new(state: Arc<Mutex<ClipboardState>>) -> Self {
        Self { state }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ClipboardState> {
        self.state.lock().unwrap_or_else(|e: PoisonError<_>| {
            log::error!("Clipboard lock poisoned: {}", e);
            e.into_inner()
        })
    }

    fn lock_mut(&mut self) -> std::sync::MutexGuard<'_, ClipboardState> {
        self.state.lock().unwrap_or_else(|e: PoisonError<_>| {
            log::error!("Clipboard lock poisoned: {}", e);
            e.into_inner()
        })
    }
}

impl IClipboard for LinuxClipboard {
    fn text(&self) -> String {
        self.lock().text.clone()
    }

    fn set_text(&mut self, text: &str) {
        let mut state = self.lock_mut();
        state.text = text.to_string();
        state.owns_clipboard = true;
        // The actual X11 SetSelectionOwner call is performed by
        // LinuxPlatform after this method completes.
    }

    fn has_text(&self) -> bool {
        !self.lock().text.is_empty()
    }
}
