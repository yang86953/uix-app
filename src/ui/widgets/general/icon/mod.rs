//! Icon widget — renders icons from Lucide TTF font.
//!
//! Maps icon names to Unicode Private Use Area codepoints from Lucide
//! v1.31.0 (ISC license); the full name → codepoint table lives in the
//! generated `icon_map` module. The parent app is responsible for loading
//! `assets/fonts/lucide.ttf` into the engine via `load_font`.

use std::sync::OnceLock;

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::resources::font::font_service::FontService;
use crate::draw::{Color, FontHandle};
use crate::ui::SnapshotFields;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::ColorValue;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::widget;
// 引入图标映射组件的封装查找入口。
use crate::ui::widgets::general::icon_map::find_icon;

/// 全局 Lucide 字体句柄（由 app 启动时加载）。
/// FontHandle 为 Copy 类型，无需 Mutex 保护——OnceLock 本身保证线程安全初始化。
static LUCIDE_FONT: OnceLock<FontHandle> = OnceLock::new();

// 保存由 UIX 声明、由 Rust 字体绘制内核消费的静态视觉值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct IconVisual {
    // 未被作者覆盖时使用的方形边长。
    default_size: f32,
    // Lucide 字形相对方形边长的缩放比例。
    glyph_scale: f32,
    // 字体尚未加载时文字后备相对边长的缩放比例。
    fallback_scale: f32,
    // 独立 Icon 使用的主题语义色。
    color: ColorValue,
}

// 同一份 UIX 在模块级生成静态视觉常量，也在 View::build 中声明组合结构。
crate::uix_items!("src/ui/widgets/general/icon/icon.uix");

// Lucide 字体缺失时使用固定栈缓冲生成首字符大写后备，避免逐帧 String 分配。
struct IconFallbackLabel {
    bytes: [u8; 16],
    len: u8,
}

impl IconFallbackLabel {
    // 保留原有“首字符完整 Unicode 大写展开；空名称显示问号”的语义。
    fn new(name: &str) -> Self {
        let mut label = Self {
            bytes: [0; 16],
            len: 0,
        };
        if let Some(character) = name.chars().next() {
            for uppercase in character.to_uppercase() {
                let start = usize::from(label.len);
                let required = uppercase.len_utf8();
                if start + required > label.bytes.len() {
                    break;
                }
                let written = uppercase.encode_utf8(&mut label.bytes[start..]).len();
                label.len += written as u8;
            }
        }
        if label.len == 0 {
            label.bytes[0] = b'?';
            label.len = 1;
        }
        label
    }

    // 缓冲只由 encode_utf8 与 ASCII 问号写入，因此转换始终有效。
    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..usize::from(self.len)])
            .expect("Icon 后备缓冲必须保持 UTF-8")
    }
}

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
    // 映射组件负责跨有序分片查找，Icon 只拥有未知名称的可见兜底。
    match find_icon(name) {
        // 命中时直接返回静态 PUA 字符。
        Some(character) => character,
        // 未命中时记录诊断并使用稳定的搜索图标。
        None => {
            // 未知图标名落入 search 兜底字形，记录一次以便诊断拼写错误。
            tracing::warn!(
                "icon_char: unknown icon name {:?}, falling back to search",
                name
            );
            "\u{E151}" // fallback: search
        }
    }
}

widget! {
    /// Icon widget — renders a single glyph from the Lucide icon font.
    pub struct Icon {
        name: Box<str>,
        #[snapshot(skip)]
        // 构建时一次解析的 Lucide 字形，避免独立 Icon 每帧重复查表。
        glyph: char,
        size: f32,
        #[snapshot(skip)]
        // 区分作者显式尺寸与 UIX 默认尺寸，声明刷新不能覆盖作者输入。
        size_authored: bool,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let color = ICON_VISUAL.color.resolve(ctx.tokens());
        if lucide_handle().is_some() {
            Self::paint_glyph_in_frame(ctx, self.glyph, frame, color, self.size * ICON_VISUAL.glyph_scale);
        } else {
            let fallback = IconFallbackLabel::new(&self.name);
            ctx.text_center(fallback.as_str(), frame, color, self.size * ICON_VISUAL.fallback_scale);
        }
    }
}

impl Icon {
    /// Paint an icon inside another widget while preserving the parent widget's
    /// font and line-box alignment. This is the embedded rendering entry point
    /// owned by the `Icon` widget; built-in widgets must not draw ad-hoc
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
        let glyph = icon_char(name).chars().next().unwrap_or('\u{E151}');
        Self::paint_glyph_in_frame(ctx, glyph, rect, color, font_size);
    }

    // 使用已解析字形完成居中绘制；独立 Icon 通过此入口跳过名称查表。
    fn paint_glyph_in_frame(
        ctx: &mut PaintContext,
        glyph: char,
        rect: Rect,
        color: Color,
        font_size: f32,
    ) {
        let line_h = ctx.line_box_height(font_size);
        let y = rect.y + (rect.h - line_h) * 0.5;
        let saved = *ctx.font();
        if let Some(fh) = lucide_handle() {
            ctx.set_font(fh);
        }
        let mut glyph_bytes = [0_u8; 4];
        let text = glyph.encode_utf8(&mut glyph_bytes);
        let width = ctx.measure_text(text, font_size).w;
        let x = rect.x + ((rect.w - width) * 0.5).max(0.0);
        ctx.draw_text(text, Point::new(x, y), color, font_size);
        ctx.set_font(saved);
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.name = next.name;
        self.glyph = next.glyph;
        self.size = next.size;
        self.size_authored = next.size_authored;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Icon {
            name: self.name.to_string(),
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
        let name = name.into().into_boxed_str();
        let glyph = icon_char(&name).chars().next().unwrap_or('\u{E151}');
        Self {
            name,
            glyph,
            size: ICON_VISUAL.default_size,
            size_authored: false,
        }
    }
    /// 设置图标的方形边长。
    pub fn size(mut self, s: f32) -> Self {
        self.size = s;
        self.size_authored = true;
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(self.size, self.size)
    }
}

// 向 UIX 静态模板提供零分配正文主题色角色。
const fn icon_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}

// 把 UIX 声明视觉融合进原有 Icon 叶内核，不创建包装节点。
fn build_icon_view(mut kernel: Icon, visual: IconVisual) -> ViewNode {
    if !kernel.size_authored {
        kernel.size = visual.default_size;
    }
    ViewNode::leaf(kernel)
}

impl View for Icon {
    fn build(self) -> ViewNode {
        // Rust 只交付名称、已解析字形与作者尺寸状态。
        let kernel = self;
        // 默认尺寸、绘制比例和主题色由同目录 UIX 声明。
        crate::uix!("src/ui/widgets/general/icon/icon.uix")
    }
}

#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/general/icon__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
