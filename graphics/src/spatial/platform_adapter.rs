//! 平台坐标适配——屏蔽不同操作系统之间的坐标方向差异。
//!
//! 应用层统一使用 y-down 坐标系（左上角原点），
//! 平台适配层自动处理 y-up 平台的坐标翻转。

/// 坐标方向。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Orientation {
    /// y 轴向下（屏幕坐标，Windows 默认）。
    /// 原点在左上角，y 增加向下。
    YDown,
    /// y 轴向上（数学坐标，Wayland/macOS Core Graphics 默认）。
    /// 原点在左下角，y 增加向上。
    YUp,
}

impl Orientation {
    /// 检测当前平台的坐标方向。
    pub fn detect() -> Self {
        #[cfg(target_os = "windows")]
        {
            Orientation::YDown
        }
        #[cfg(target_os = "linux")]
        {
            // Wayland 使用 y-up
            Orientation::YUp
        }
        #[cfg(target_os = "macos")]
        {
            // macOS Core Graphics 使用 y-up
            Orientation::YUp
        }
    }

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

impl Default for Orientation {
    fn default() -> Self {
        Self::YDown
    }
}

// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ydown_no_conversion() {
        let o = Orientation::YDown;
        let (x, y) = o.screen_to_native(600.0, 100.0, 200.0);
        assert!((x - 100.0).abs() < 1e-10);
        assert!((y - 200.0).abs() < 1e-10);
    }

    #[test]
    fn yup_flips_y() {
        let o = Orientation::YUp;
        let (x, y) = o.screen_to_native(600.0, 100.0, 200.0);
        assert!((x - 100.0).abs() < 1e-10);
        assert!((y - 400.0).abs() < 1e-10); // 600 - 200 = 400
    }

    #[test]
    fn roundtrip() {
        let o = Orientation::YUp;
        let (nx, ny) = o.screen_to_native(800.0, 150.0, 300.0);
        let (sx, sy) = o.native_to_screen(800.0, nx, ny);
        assert!((sx - 150.0).abs() < 1e-10);
        assert!((sy - 300.0).abs() < 1e-10);
    }

    #[test]
    fn detect_does_not_panic() {
        let _ = Orientation::detect();
    }
}
