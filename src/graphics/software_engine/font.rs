//! SoftwareEngine — 字体相关方法（全部委托给独立的 FontService）。

use super::engine::SoftwareEngine;

impl SoftwareEngine {
    /// 设置首选字体族名称。
    ///
    /// 必须在调用 `initialize()` **之前**调用才能生效。
    /// 字体系列名可以是具体字体名（如 "Noto Sans SC"、"Segoe UI"），
    /// 也可以是 CSS 通用家族名（"sans-serif"、"serif"、"monospace"）。
    pub fn set_font_family(&mut self, family: impl Into<String>) {
        self.font_service.set_font_family(family);
    }
}
