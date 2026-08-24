//! Empty widget — 空状态占位（图标 + 描述居中）。

use crate::core::{Constraints, Rect, Size};
use crate::ui::SnapshotFields;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::widget;

const MIN_WIDTH: f32 = 160.0;
const MAX_WIDTH: f32 = 320.0;
const MIN_HEIGHT: f32 = 100.0;
const TEXT_FONT_SIZE: f32 = 13.0;
const TEXT_LINE_HEIGHT: f32 = 1.5;
// 空状态水平内边距（16.0）；descriptions 为 8.0，组件独立设计。
const HORIZONTAL_PADDING: f32 = 16.0;
// 空状态垂直内边距（12.0）；descriptions 为 6.0，组件独立设计。
const VERTICAL_PADDING: f32 = 12.0;
// 空状态插图（32.0）；breadcrumb 导航图标为 14.0，语境不同。
const ICON_SIZE: f32 = 32.0;
// 空状态图标与文字间距（12.0）；breadcrumb 为 4.0，语境不同。
const ICON_TEXT_GAP: f32 = 12.0;

widget! {
    /// Empty — 空状态展示。
    pub struct Empty {
        description: String,
        icon_name: String,
        image: String,
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
        let text_color = ctx.tokens().color_text_secondary();
        let icon_color = ctx.tokens().color_text_tertiary();
        let icon_name = self.visual_icon_name();
        let text_width = (frame.w - HORIZONTAL_PADDING * 2.0).max(1.0);
        // 一次文本测量同时取得行数与高度，避免同帧重复估算。
        let (line_count, text_height) = Self::description_layout(desc, text_width);
        let icon_size = icon_name
            .map(|_| {
                ICON_SIZE
                    .min((frame.w - HORIZONTAL_PADDING * 2.0).max(0.0))
                    .min(frame.h * 0.4)
            })
            .unwrap_or(0.0);
        let gap = if icon_size > 0.0 {
            ICON_TEXT_GAP.min((frame.h - icon_size).max(0.0))
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
            frame.x + HORIZONTAL_PADDING,
            y,
            text_width,
            text_height.min((frame.y + frame.h - y).max(0.0)),
        );
        if text_frame.h > 0.0 {
            if line_count <= 1 {
                ctx.text_center(desc, text_frame, text_color, TEXT_FONT_SIZE);
            } else {
                ctx.push_clip(text_frame);
                ctx.draw_text_wrapped(desc, text_frame, text_color, TEXT_FONT_SIZE);
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

// 把空状态 Rust 绘制内核融合为 UIX 声明的单一叶节点。
fn build_empty_view(kernel: Empty) -> ViewNode {
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
            TEXT_FONT_SIZE,
        )
        .max_line_width;
        (text_width + HORIZONTAL_PADDING * 2.0).clamp(MIN_WIDTH, MAX_WIDTH)
    }

    fn intrinsic_height(&self, description: &str, width: f32) -> f32 {
        let text_width = (width - HORIZONTAL_PADDING * 2.0).max(1.0);
        let visual_height = if self.visual_icon_name().is_some() {
            ICON_SIZE + ICON_TEXT_GAP
        } else {
            0.0
        };
        let (_, description_height) = Self::description_layout(description, text_width);
        (VERTICAL_PADDING * 2.0 + visual_height + description_height)
            .max(MIN_HEIGHT)
    }

    fn description_layout(description: &str, width: f32) -> (usize, f32) {
        let line_count = crate::draw::resources::font::text_backend::estimate_text_metrics(
            description,
            width.max(1.0),
            TEXT_FONT_SIZE,
        )
        .line_count
        .max(1);
        (
            line_count,
            line_count as f32 * TEXT_FONT_SIZE * TEXT_LINE_HEIGHT,
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
    }
}

// 集中验证 UIX 声明壳与 Rust 内核的单节点契约。
#[cfg(test)]
#[path = "../../../../../tests/unit/ui/widgets/display/empty__tests.rs"]
mod tests;
