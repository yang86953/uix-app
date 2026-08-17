//! Icon widget — renders icons from Lucide TTF font.
//!
//! Maps icon names to Unicode Private Use Area codepoints from Lucide
//! v1.31.0 (ISC license); the full name → codepoint table lives in the
//! generated `icon_map` module. The parent app is responsible for loading
//! `assets/fonts/lucide.ttf` into the engine via `load_font`.

use std::sync::OnceLock;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::resources::font::font_service::FontService;
use crate::draw::{Color, FontHandle};
use crate::ui::SnapshotFields;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::ui::widgets::general::icon_map::ICON_MAP;

/// 全局 Lucide 字体句柄（由 app 启动时加载）。
/// FontHandle 为 Copy 类型，无需 Mutex 保护——OnceLock 本身保证线程安全初始化。
static LUCIDE_FONT: OnceLock<FontHandle> = OnceLock::new();

/// 在 app 初始化时加载 Lucide TTF 字体，并注册全局句柄。
///
/// 直接通过 `FontService::load_font()` 加载字体数据。
/// 应在 app 初始化时、FontService 创建之后调用。
pub fn init_lucide_font(data: &[u8], font_service: &mut FontService) {
    match font_service.load_font(data) {
        Ok(fh) => {
            tracing::info!("Lucide font loaded, handle={:?}", fh);
            let _ = LUCIDE_FONT.set(fh);
        }
        Err(e) => {
            tracing::warn!("Failed to load Lucide font: {}", e.short_what());
        }
    }
}

/// 获取 Lucide 字体句柄（若已加载）。
pub fn lucide_handle() -> Option<FontHandle> {
    LUCIDE_FONT.get().copied()
}

/// Map icon name → Lucide PUA codepoint character.
/// 全量映射表见 icon_map 模块（生成自 lucide-static v1.31.0 codepoints.json）。
pub(crate) fn icon_char(name: &str) -> &'static str {
    // 表按名称升序排列，使用二分查找避免手写 match 的维护成本。
    match ICON_MAP.binary_search_by_key(&name, |entry| entry.0) {
        Ok(idx) => ICON_MAP[idx].1,
        Err(_) => {
            // 未知图标名落入 search 兜底字形，记录一次以便诊断拼写错误。
            tracing::warn!(
                "icon_char: unknown icon name {:?}, falling back to search",
                name
            );
            "\u{E151}" // fallback: search
        }
    }
}

component! {
    /// Icon widget — renders a single glyph from the Lucide icon font.
    pub struct Icon {
        name: String,
        size: f32,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let color = ctx.tokens().color_text();
        if lucide_handle().is_some() {
            Self::paint_in_frame(ctx, &self.name, frame, color, self.size * 0.85);
        } else {
            let fallback = self
                .name
                .chars()
                .next()
                .map(|character| character.to_uppercase().to_string())
                .unwrap_or_else(|| "?".to_string());
            ctx.text_center(&fallback, frame, color, self.size * 0.55);
        }
    }
}

impl Icon {
    /// Paint an icon inside another widget while preserving the parent widget's
    /// font and line-box alignment. This is the embedded rendering entry point
    /// owned by the `Icon` component; built-in widgets must not draw ad-hoc
    /// Unicode symbols or call `icon_char` directly.
    pub fn paint_in_frame(
        ctx: &mut PaintContext,
        name: &str,
        rect: Rect,
        color: Color,
        font_size: f32,
    ) {
        if name.is_empty() {
            return;
        }
        let line_h = ctx.line_box_height(font_size);
        let y = rect.y + (rect.h - line_h) * 0.5;
        let saved = *ctx.font();
        if let Some(fh) = lucide_handle() {
            ctx.set_font(fh);
        }
        let text = icon_char(name);
        let width = ctx.measure_text(text, font_size).w;
        let x = rect.x + ((rect.w - width) * 0.5).max(0.0);
        ctx.draw_text(text, Point::new(x, y), color, font_size);
        ctx.set_font(saved);
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.name = next.name;
        self.size = next.size;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Icon {
            name: self.name.clone(),
            size: self.size,
        }
    }
}

impl Default for Icon {
    fn default() -> Self {
        Self::new("search")
    }
}

impl Icon {
    /// 创建使用指定图标名称和默认尺寸的图标组件。
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            size: 24.0,
        }
    }
    /// 设置图标的方形边长。
    pub fn size(mut self, s: f32) -> Self {
        self.size = s;
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(self.size, self.size)
    }
}

#[cfg(test)]
mod tests {
    use super::icon_char;

    /// 全量表中已知名称应解析为具体字形（而非 fallback 的 search）。
    #[test]
    fn icon_char_resolves_known_names() {
        // 抽查常见与历史保留名称；search 本身字形即兜底字符，单独断言。
        assert_eq!(icon_char("search"), "\u{E151}");
        for name in ["home", "bell", "layout-dashboard", "terminal"] {
            let glyph = icon_char(name);
            assert_ne!(glyph, "\u{E151}", "{name} 不应落入 search 兜底");
        }
    }

    /// 未知名称应落入 search 兜底字形。
    #[test]
    fn icon_char_falls_back_for_unknown_names() {
        assert_eq!(icon_char("definitely-not-an-icon"), "\u{E151}");
    }
}
