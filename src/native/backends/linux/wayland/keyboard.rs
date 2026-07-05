// ============================================================================
// platform/linux/wayland/keyboard.rs — IKeyboard impl for WaylandBackend
// ============================================================================

use crate::native::IKeyboard;
use crate::native::KeyCode;

use super::WaylandBackend;

impl IKeyboard for WaylandBackend {
    fn is_down(&self, key: KeyCode) -> bool {
        self.keys_down
            .lock()
            .map(|k| k.contains(&key))
            .unwrap_or(false)
    }
    fn idle_ms(&self) -> u32 {
        0
    }
    fn double_click_ms(&self) -> u32 {
        400
    }
}
