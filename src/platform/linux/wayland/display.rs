// ============================================================================
// platform/linux/wayland/display.rs — IDisplay impl for WaylandBackend
// ============================================================================

use crate::base::Rect;
use crate::platform::types::DisplayInfo;
use crate::platform::IDisplay;

use super::WaylandBackend;

impl IDisplay for WaylandBackend {
    fn dpi_scale(&self) -> f32 {
        1.0
    }
    fn is_dark_mode(&self) -> bool {
        std::env::var("GTK_THEME")
            .map(|t| t.contains("dark"))
            .unwrap_or(false)
    }
    fn count(&self) -> i32 {
        1
    }
    fn info(&self, _: i32) -> DisplayInfo {
        DisplayInfo {
            bounds: Rect::new(0.0, 0.0, 1920.0, 1080.0),
            dpi_scale: 1.0,
            is_primary: true,
        }
    }
}
