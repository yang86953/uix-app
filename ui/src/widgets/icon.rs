//! Icon widget — renders icons from Lucide TTF font.
//!
//! Maps icon names to Unicode Private Use Area codepoints from Lucide
//! v1.17.0 (ISC license, 1981 icons).  The parent app is responsible for
//! loading `assets/fonts/lucide.ttf` into the engine via `load_font`.

use std::sync::OnceLock;

use uix_platform::{Rect, Size};
use crate::define_widget;
use uix_graphics::{FontHandle, GraphicsEngine};
use uix_graphics::font_service::FontService;
use crate::render_context::RenderContext;
use crate::widget::WidgetTree;

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
            log::info!("Lucide font loaded, handle={:?}", fh);
            let _ = LUCIDE_FONT.set(fh);
        }
        Err(e) => {
            log::warn!("Failed to load Lucide font: {}", e.short_what());
        }
    }
}

/// 获取 Lucide 字体句柄（若已加载）。
pub fn lucide_handle() -> Option<FontHandle> {
    LUCIDE_FONT.get().copied()
}

/// Map icon name → Lucide PUA codepoint character.
/// Generated from lucide-static v1.17.0 codepoints.json.
pub fn icon_char(name: &str) -> &'static str {
    match name {
        "search" => "\u{E151}",
        "home" => "\u{E0F5}",
        "settings" => "\u{E154}",
        "user" => "\u{E19F}",
        "menu" => "\u{E115}",
        "x" => "\u{E1B2}",
        "check" => "\u{E06C}",
        "chevron-left" => "\u{E06E}",
        "chevron-right" => "\u{E06F}",
        "chevron-down" => "\u{E06D}",
        "chevron-up" => "\u{E070}",
        "plus" => "\u{E13D}",
        "minus" => "\u{E11C}",
        "alert-circle" => "\u{E077}",
        "info" => "\u{E0F9}",
        "mail" => "\u{E10F}",
        "bell" => "\u{E059}",
        "clock" => "\u{E087}",
        "calendar" => "\u{E063}",
        "heart" => "\u{E0F2}",
        "star" => "\u{E176}",
        "github" => "\u{E0E6}",
        "sun" => "\u{E178}",
        "moon" => "\u{E11E}",
        "edit" => "\u{E172}",
        "trash-2" => "\u{E18E}",
        "trash" => "\u{E18D}",
        "external-link" => "\u{E0B9}",
        "arrow-left" => "\u{E048}",
        "arrow-right" => "\u{E049}",
        "arrow-down" => "\u{E042}",
        "arrow-up" => "\u{E04A}",
        "upload" => "\u{E19E}",
        "download" => "\u{E0B2}",
        "camera" => "\u{E064}",
        "image" => "\u{E0F6}",
        "video" => "\u{E1A5}",
        "music" => "\u{E122}",
        "phone" => "\u{E133}",
        "map-pin" => "\u{E111}",
        "map" => "\u{E110}",
        "lock" => "\u{E10B}",
        "unlock" => "\u{E10C}",
        "eye" => "\u{E0BA}",
        "eye-off" => "\u{E0BB}",
        "bookmark" => "\u{E060}",
        "tag" => "\u{E17F}",
        "share" => "\u{E156}",
        "send" => "\u{E152}",
        "flag" => "\u{E0D1}",
        "filter" => "\u{E0DC}",
        "refresh-cw" => "\u{E145}",
        "refresh-ccw" => "\u{E144}",
        "copy" => "\u{E09E}",
        "clipboard" => "\u{E085}",
        "printer" => "\u{E141}",
        "bluetooth" => "\u{E05C}",
        "bold" => "\u{E05D}",
        "book" => "\u{E05E}",
        "box" => "\u{E061}",
        "briefcase" => "\u{E062}",
        "chart-bar" => "\u{E2A2}",
        "chart-line" => "\u{E2A5}",
        "chart-pie" => "\u{E06B}",
        "check-circle" => "\u{E07C}",
        "check-square" => "\u{E16A}",
        "circle" => "\u{E076}",
        "database" => "\u{E0AD}",
        "delete" => "\u{E0AE}",
        "file" => "\u{E0C0}",
        "file-text" => "\u{E0CC}",
        "folder" => "\u{E0D7}",
        "gift" => "\u{E0E1}",
        "globe" => "\u{E0E8}",
        "grid" => "\u{E0E9}",
        "hard-drive" => "\u{E0ED}",
        "inbox" => "\u{E0F7}",
        "keyboard" => "\u{E284}",
        "layout" => "\u{E12C}",
        "life-buoy" => "\u{E101}",
        "link" => "\u{E102}",
        "link-2" => "\u{E103}",
        "list" => "\u{E106}",
        "loader" => "\u{E109}",
        "log-in" => "\u{E10D}",
        "log-out" => "\u{E10E}",
        "maximize" => "\u{E112}",
        "minimize" => "\u{E11A}",
        "message-circle" => "\u{E116}",
        "message-square" => "\u{E117}",
        "navigation" => "\u{E123}",
        "package" => "\u{E129}",
        "pause" => "\u{E12E}",
        "play" => "\u{E13C}",
        "power" => "\u{E140}",
        "repeat" => "\u{E146}",
        "scissors" => "\u{E14E}",
        "server" => "\u{E153}",
        "shield" => "\u{E158}",
        "shopping-cart" => "\u{E15C}",
        "sliders" => "\u{E162}",
        "square" => "\u{E167}",
        "table" => "\u{E17D}",
        "terminal" => "\u{E181}",
        "triangle" => "\u{E192}",
        "type" => "\u{E198}",
        "umbrella" => "\u{E199}",
        "volume" => "\u{E1A9}",
        "volume-1" => "\u{E1AA}",
        "volume-2" => "\u{E1AB}",
        "volume-x" => "\u{E1AC}",
        "watch" => "\u{E1AD}",
        "wifi" => "\u{E1AE}",
        "wind" => "\u{E1B0}",
        "zap" => "\u{E1B4}",
        "zoom-in" => "\u{E1B6}",
        "zoom-out" => "\u{E1B7}",
        "activity" => "\u{E038}",
        "award" => "\u{E04F}",
        "anchor" => "\u{E03F}",
        "palette" => "\u{E1DD}",
        "x-circle" => "\u{E084}",
        "x-square" => "\u{E175}",
        "stop-circle" => "\u{E083}",
        "maximize-2" => "\u{E113}",
        "minimize-2" => "\u{E11B}",
        "share-2" => "\u{E156}",
        _ => "\u{E151}", // fallback: search
    }
}

define_widget! {
    /// Icon widget — renders a single glyph from the Lucide icon font.
    pub struct Icon {
        name: String,
        size: f32,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        Size::new(self.size, self.size)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let icon_str = icon_char(&self.name);
        let color = ctx.tokens().color_text();
        let saved = *ctx.font();
        let has_lucide = lucide_handle().is_some();
        if let Some(fh) = lucide_handle() {
            ctx.set_font(fh);
        }
        ctx.text_center(icon_str, frame, color, self.size * 0.85);
        ctx.set_font(saved);

        if !has_lucide {
            let fallback = ctx.measure_text(icon_str, self.size * 0.85);
            if fallback.w < 1.0 {
                let label = &self.name[..self.name.len().min(2)];
                ctx.text_center(label, frame, color, self.size * 0.55);
            }
        }
    }
}

impl Default for Icon {
    fn default() -> Self {
        Self::new("search")
    }
}

impl Icon {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            size: 24.0,
        }
    }
    pub fn size(mut self, s: f32) -> Self {
        self.size = s;
        self
    }
}
