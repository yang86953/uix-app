//! 平台坐标适配——屏蔽不同操作系统之间的坐标方向差异。
//!
//! 应用层统一使用 y-down 坐标系（左上角原点），
//! 平台适配层自动处理 y-up 平台的坐标翻转。

/// 坐标方向。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Orientation {
    /// y 轴向下（屏幕坐标，Windows 默认）。
    /// 原点在左上角，y 增加向下。
    #[default]
    YDown,
    /// y 轴向上（数学坐标，Wayland/macOS Core Graphics 默认）。
    /// 原点在左下角，y 增加向上。
    YUp,
}

impl Orientation {
    /// 将屏幕坐标（y-down）转换为平台原生坐标。
    ///
    /// `screen_height`：表面高度（像素），用于 y-up 平台的翻转。
    #[inline(always)]
    pub fn screen_to_native(&self, screen_height: f32, x: f32, y: f32) -> (f32, f32) {
        match self {
            Orientation::YDown => (x, y),
            Orientation::YUp => (x, screen_height - y),
        }
    }

    /// 将平台原生坐标转换为屏幕坐标（y-down）。
    #[inline(always)]
    pub fn native_to_screen(&self, screen_height: f32, x: f32, y: f32) -> (f32, f32) {
        match self {
            Orientation::YDown => (x, y),
            Orientation::YUp => (x, screen_height - y),
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
#[path = "../../tests/draw/spatial/platform_adapter.rs"]
mod tests;

