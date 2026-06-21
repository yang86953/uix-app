// ============================================================================
// platform/linux/text_input.rs — Linux text/IME input (ITextInput)
// ============================================================================
//
// Uses X11 XIM (X Input Method) for international text input.
// Basic keyboard input works without XIM.
// ============================================================================

use crate::ITextInput;

// ════════════════════════════════════════════════════════════════════════════
// LinuxTextInput
// ════════════════════════════════════════════════════════════════════════════

pub struct LinuxTextInput;

impl LinuxTextInput {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxTextInput {
    fn default() -> Self {
        Self::new()
    }
}

impl ITextInput for LinuxTextInput {
    fn start(&mut self) {
        log::warn!("Linux: text_input (IME) not implemented — no zwp_text_input_v3 protocol");
    }

    fn stop(&mut self) {
    }
}
