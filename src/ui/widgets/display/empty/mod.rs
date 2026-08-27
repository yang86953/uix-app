//! Empty widget — 空状态占位（图标 + 描述居中）。

use crate::core::{Constraints, Rect, Size};
use crate::ui::SnapshotFields;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::ColorValue;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::widget;

// 保存由 UIX 声明、由 Rust 本地化与测量内核消费的静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct EmptyVisual {
    min_width: f32,
    max_width: f32,
    min_height: f32,
    text_font_size: f32,
    text_line_height: f32,
    horizontal_padding: f32,
    vertical_padding: f32,
    icon_size: f32,
    icon_text_gap: f32,
    icon_max_height_ratio: f32,
    text_color: ColorValue,
    icon_color: ColorValue,
}

// 同目录 UIX 生成唯一视觉值及静态借用。
crate::uix_items!("src/ui/widgets/display/empty/empty.uix");

widget! {
    /// Empty — 空状态展示。
    pub struct Empty {
        description: String,
        icon_name: String,
        image: String,
        #[snapshot(skip)]
        visual: &'static EmptyVisual,
    }

    measure => (&self, constraints: Constraints) -> Size {
        let loc = crate::ui::widget_runtime::locale::use_locale();
        let description = if self.description.is_empty() {
            loc.empty_description
        } else {
            &self.description
        };
        let width = constraints
            .clamp(Size::new(self.preferred_width(description), 0.0))
            .w;
        constraints.clamp(Size::new(width, self.intrinsic_height(description, width)))
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let frame = Rect::new(frame.x, frame.y, frame.w.max(0.0), frame.h.max(0.0));
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let loc = crate::ui::widget_runtime::locale::use_locale();
        let desc = if self.description.is_empty() { loc.empty_description } else { &self.description };
        let text_color = self.visual.text_color.resolve(ctx.tokens());
        let icon_color = self.visual.icon_color.resolve(ctx.tokens());
        let icon_name = self.visual_icon_name();
        let text_width = (frame.w - self.visual.horizontal_padding * 2.0).max(1.0);
        // 一次文本测量同时取得行数与高度，避免同帧重复估算。
        let (line_count, text_height) = self.description_layout(desc, text_width);
        let icon_size = icon_name
            .map(|_| {
                self.visual
                    .icon_size
                    .min((frame.w - self.visual.horizontal_padding * 2.0).max(0.0))
                    .min(frame.h * self.visual.icon_max_height_ratio.clamp(0.0, 1.0))
            })
            .unwrap_or(0.0);
        let gap = if icon_size > 0.0 {
            self.visual
                .icon_text_gap
                .min((frame.h - icon_size).max(0.0))
        } else {
            0.0
        };
        let content_height = icon_size + gap + text_height;
        let mut y = frame.y + ((frame.h - content_height).max(0.0) * 0.5);

        ctx.push_clip(frame);
        if let Some(icon_name) = icon_name.filter(|_| icon_size > 0.0) {
            let icon_frame = Rect::new(frame.x, y, frame.w, icon_size);
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                icon_name,
                icon_frame,
                icon_color,
                icon_size,
            );
            y += icon_size + gap;
        }

        let text_frame = Rect::new(
            frame.x + self.visual.horizontal_padding,
            y,
            text_width,
            text_height.min((frame.y + frame.h - y).max(0.0)),
        );
        if text_frame.h > 0.0 {
            if line_count <= 1 {
                ctx.text_center(desc, text_frame, text_color, self.visual.text_font_size);
            } else {
                ctx.push_clip(text_frame);
                ctx.draw_text_wrapped(desc, text_frame, text_color, self.visual.text_font_size);
                ctx.pop_clip();
            }
        }
        ctx.pop_clip();
    }
}

impl Default for Empty {
    fn default() -> Self {
        Self::new()
    }
}

// 向 UIX 提供空状态说明文字主题角色。
const fn empty_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}

// 向 UIX 提供空状态图标主题角色。
const fn empty_text_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextTertiary)
}

// 把 UIX 声明的静态视觉融合进空状态本地化与测量内核。
fn build_empty_view(mut kernel: Empty, visual: &'static EmptyVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Empty {
    fn build(self) -> ViewNode {
        // UIX 拥有公开组件根，Rust 保留本地化、测量和绘制机制。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/empty/empty.uix")
    }
}

impl Empty {
    /// 创建不带说明文字和视觉资源的空状态组件。
    pub fn new() -> Self {
        Self {
            description: String::new(),
            icon_name: String::new(),
            image: String::new(),
            visual: EMPTY_VISUAL_REF,
        }
    }
    /// 设置空状态的说明文字。
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = d.into();
        self
    }
    /// 设置空状态使用的图标名称。
    pub fn icon(mut self, name: impl Into<String>) -> Self {
        self.icon_name = name.into();
        self
    }
    /// 设置优先于图标展示的图片资源名称。
    pub fn image(mut self, name: impl Into<String>) -> Self {
        self.image = name.into();
        self
    }

    fn preferred_width(&self, description: &str) -> f32 {
        let text_width = crate::draw::resources::font::text_backend::estimate_text_metrics(
            description,
            f32::INFINITY,
            self.visual.text_font_size,
        )
        .max_line_width;
        (text_width + self.visual.horizontal_padding * 2.0)
            .clamp(self.visual.min_width, self.visual.max_width)
    }

    fn intrinsic_height(&self, description: &str, width: f32) -> f32 {
        let text_width = (width - self.visual.horizontal_padding * 2.0).max(1.0);
        let visual_height = if self.visual_icon_name().is_some() {
            self.visual.icon_size + self.visual.icon_text_gap
        } else {
            0.0
        };
        let (_, description_height) = self.description_layout(description, text_width);
        (self.visual.vertical_padding * 2.0 + visual_height + description_height)
            .max(self.visual.min_height)
    }

    fn description_layout(&self, description: &str, width: f32) -> (usize, f32) {
        let line_count = crate::draw::resources::font::text_backend::estimate_text_metrics(
            description,
            width.max(1.0),
            self.visual.text_font_size,
        )
        .line_count
        .max(1);
        (
            line_count,
            line_count as f32 * self.visual.text_font_size * self.visual.text_line_height,
        )
    }

    fn visual_icon_name(&self) -> Option<&str> {
        if !self.image.is_empty() {
            Some(match self.image.as_str() {
                "default" => "package",
                "search" => "search",
                "file" => "file",
                "folder" => "folder",
                "network" => "globe",
                _ => "package",
            })
        } else if self.icon_name.is_empty() {
            None
        } else {
            Some(&self.icon_name)
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Empty {
            description: self.description.clone(),
            icon_name: self.icon_name.clone(),
            image: self.image.clone(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.description = next.description;
        self.icon_name = next.icon_name;
        self.image = next.image;
        self.visual = next.visual;
    }
}
