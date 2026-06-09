// ============================================================================
// platform/linux/text_input.rs — Linux text/IME input (ITextInput)
// ============================================================================
//
// Uses X11 XIM (X Input Method) for international text input.
// Basic keyboard input works without XIM.
// ============================================================================

use crate::platform::ITextInput;

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
        // XIM integration would open an input method connection here.
        // For now, basic keyboard input works without XIM.
    }

    fn stop(&mut self) {
        // Close the input method context.
    }
}
