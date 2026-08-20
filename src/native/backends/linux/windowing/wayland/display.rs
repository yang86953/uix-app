// ============================================================================
// platform/linux/wayland/display.rs — IDisplay impl for WaylandBackend
//
// 使用 wl_output 协议收集的显示器几何和缩放因子，提供真实查询。
// 回退到硬编码值（1920×1080, scale=1）当 wl_output 未绑定或零输出时。
// ============================================================================

use crate::native::capabilities::display::DisplayInfo;
use crate::native::capabilities::display::IDisplay;
use crate::native::{Errc, Error, Result};

use super::WaylandBackend;

impl IDisplay for WaylandBackend {
    fn dpi_scale(&self) -> Result<f32> {
        let list = self.outputs.lock().map_err(|_| {
            Error::new(
                Errc::InvalidState,
                "WaylandBackend::dpi_scale: outputs lock poisoned",
            )
        })?;
        list.iter()
            .find(|o| o.is_primary)
            .or_else(|| list.first())
            .map(|o| o.scale as f32)
            .ok_or_else(|| Error::new(Errc::NotFound, "WaylandBackend::dpi_scale: no outputs"))
    }

    fn is_dark_mode(&self) -> Result<bool> {
        Ok(std::env::var("GTK_THEME")
            .map(|t| t.contains("dark"))
            .unwrap_or(false))
    }

    fn count(&self) -> Result<i32> {
        let list = self.outputs.lock().map_err(|_| {
            Error::new(
                Errc::InvalidState,
                "WaylandBackend::count: outputs lock poisoned",
            )
        })?;
        Ok(list.len() as i32)
    }

    fn info(&self, index: i32) -> Result<DisplayInfo> {
        let idx = index.max(0) as usize;
        let list = self.outputs.lock().map_err(|_| {
            Error::new(
                Errc::InvalidState,
                "WaylandBackend::info: outputs lock poisoned",
            )
        })?;
        list.get(idx).map(|o| o.to_display_info()).ok_or_else(|| {
            Error::new(
                Errc::NotFound,
                format!("WaylandBackend::info: no output at index {idx}"),
            )
        })
    }
}
