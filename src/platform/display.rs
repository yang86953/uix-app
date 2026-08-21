//! 平台中立显示协议：显示器即时信息、DPI 与主题查询。

use crate::core::{Rect, Result};

/// 单个显示器的边界、缩放与主屏事实。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DisplayInfo {
    pub bounds: Rect,
    pub dpi_scale: f32,
    pub is_primary: bool,
}

impl Default for DisplayInfo {
    fn default() -> Self {
        Self {
            bounds: Rect::default(),
            dpi_scale: 1.0,
            is_primary: false,
        }
    }
}

/// 平台显示能力端口；具体 OS 与测试替身只实现本合同。
pub(crate) trait IDisplay {
    fn dpi_scale(&self) -> Result<f32>;
    fn is_dark_mode(&self) -> Result<bool>;
    fn count(&self) -> Result<i32>;
    fn info(&self, index: i32) -> Result<DisplayInfo>;
}
