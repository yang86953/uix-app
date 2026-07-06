// ============================================================================
// platform/linux/wayland/keyboard.rs — IKeyboard impl for WaylandBackend
// ============================================================================

use crate::native::traits::input::IKeyboard;
use crate::native::traits::input::KeyCode;

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
