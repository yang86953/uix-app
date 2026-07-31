//! Icon widget — renders icons from Lucide TTF font.
//!
//! Maps the release-supported icon names to Unicode Private Use Area
//! codepoints from Lucide v1.17.0 (ISC license). The parent app is responsible for
//! loading `assets/fonts/lucide.ttf` into the engine via `load_font`.

use std::sync::OnceLock;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::api::PaintContext;
use crate::draw::resources::font::font_service::FontService;
use crate::draw::{Color, FontHandle};
use crate::ui::core::widget::WidgetTree;
use crate::ui::SnapshotFields;

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
/// Generated from lucide-static v1.17.0 codepoints.json.
pub(crate) fn icon_char(name: &str) -> &'static str {
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
        "share" => "\u{E155}",
        "send" => "\u{E152}",
        "flag" => "\u{E0D1}",
        "filter" => "\u{E0DC}",
        "refresh-cw" => "\u{E145}",
        "refresh-ccw" => "\u{E144}",
        "copy" => "\u{E09E}",
        "clipboard" => "\u{E085}",
        "credit-card" => "\u{E0AA}",
        "printer" => "\u{E141}",
        "bluetooth" => "\u{E05C}",
        "bold" => "\u{E05D}",
        "book" => "\u{E05E}",
        "box" => "\u{E061}",
        "briefcase" => "\u{E062}",
        "bar-chart" => "\u{E06A}",
        "chart-bar" => "\u{E2A2}",
        "chart-line" => "\u{E2A5}",
        "chart-pie" => "\u{E06B}",
        "cpu" => "\u{E0A9}",
        "layers" => "\u{E529}",
        "alert-triangle" => "\u{E193}",
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
        "grip-horizontal" => "\u{E0EA}",
        "grip-vertical" => "\u{E0EB}",
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
        "mouse-pointer" => "\u{E11F}",
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

    fn intrinsic_size(&self) -> Size {
        Size::new(self.size, self.size)
    }
}
