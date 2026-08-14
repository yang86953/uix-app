//! Timeline widget — 时间线组件，Ant Design 风格。
//!
//! 垂直时间轴展示事件序列，支持节点颜色、标签、描述。

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::Color;
use crate::ui::SnapshotFields;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::component::widget::WidgetTree;

// 时间线条目高度（60.0）；time_picker/cascader 选择列项为 32.0，语境不同。
const ITEM_HEIGHT: f32 = 60.0;
const MIN_ITEM_HEIGHT: f32 = 28.0;
const DOT_RADIUS: f32 = 5.0;

#[derive(Debug, Clone, Copy)]
struct TimelineGeometry {
    frame: Rect,
    row_height: f32,
    line_x: f32,
    content_x: f32,
    content_width: f32,
    dot_radius: f32,
}

/// 时间线节点。
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineItem {
    /// 显式节点色；透明色作为未指定哨兵，由绘制阶段解析为当前主题主色。
    pub color: Color,
    /// 节点的主要标签文本。
    pub label: String,
    /// 节点标签下方的辅助描述文本。
    pub description: String,
}

// Timeline — 时间线组件。
component! {
    pub struct Timeline {
        items: Vec<TimelineItem>,
        pending: bool,
        reverse: bool,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let loc = crate::ui::component::locale::use_locale();
        let row_count = self.row_count();
        let geometry = TimelineGeometry::new(frame, row_count);
        if row_count == 0 || geometry.frame.w <= 0.0 || geometry.frame.h <= 0.0 {
            return;
        }
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let border = ctx.tokens().color_border_secondary();
        let dot_border = ctx.tokens().color_bg_container();
        // 未显式覆写的时间线节点使用当前主题主色。
        let primary = ctx.tokens().color_primary();
        let visible_rows = ((geometry.frame.h / geometry.row_height).ceil() as usize)
            .max(1)
            .min(row_count);
        let mut previous_dot_y = None;

        ctx.push_clip(geometry.frame);
        for index in 0..visible_rows {
            let row = geometry.row(index);
            let (label, description, dot_color, pending) = if index < self.items.len() {
                let item = self.item_at(index);
                (
                    item.label.as_str(),
                    item.description.as_str(),
                    // 在拥有主题上下文的绘制阶段解析最终节点色。
                    item.resolved_dot_color(primary),
                    false,
                )
            } else {
                (loc.timeline_pending, "", border, true)
            };
            let text_geometry = row_text_geometry(ctx, row, !description.is_empty());
            let dot_y = text_geometry.dot_y;
            if let Some(previous_dot_y) = previous_dot_y {
                draw_connector(
                    ctx,
                    geometry.line_x,
                    previous_dot_y,
                    dot_y,
                    geometry.dot_radius,
                    border,
                );
            }
            if pending {
                ctx.stroke_circle(
                    geometry.line_x,
                    dot_y,
                    geometry.dot_radius,
                    border,
                    geometry.dot_radius.min(2.0),
                );
            } else {
                ctx.fill_circle(
                    geometry.line_x,
                    dot_y,
                    geometry.dot_radius,
                    dot_color,
                );
                ctx.stroke_circle(
                    geometry.line_x,
                    dot_y,
                    geometry.dot_radius,
                    dot_border,
                    geometry.dot_radius.min(2.0),
                );
            }
            paint_single_line(
                ctx,
                label,
                Rect::new(
                    geometry.content_x,
                    text_geometry.label_y,
                    geometry.content_width,
                    text_geometry.label_height,
                ),
                if pending { text_sec } else { text },
                text_geometry.label_font,
            );
            if !description.is_empty() && text_geometry.description_height > 0.0 {
                paint_single_line(
                    ctx,
                    description,
                    Rect::new(
                        geometry.content_x,
                        text_geometry.description_y,
                        geometry.content_width,
                        text_geometry.description_height,
                    ),
                    text_sec,
                    text_geometry.description_font,
                );
            }
            previous_dot_y = Some(dot_y);
        }
        ctx.pop_clip();
    }
}

impl Timeline {
    /// 创建不含节点且未启用等待状态的时间线。
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            pending: false,
            reverse: false,
        }
    }
    /// 替换时间线的全部节点。
    pub fn items(mut self, items: Vec<TimelineItem>) -> Self {
        self.items = items;
        self
    }
    /// 向时间线末尾追加一个节点。
    #[allow(
        clippy::should_implement_trait,
        reason = "add is the established fluent builder API, not arithmetic addition"
    )]
    pub fn add(mut self, item: TimelineItem) -> Self {
        self.items.push(item);
        self
    }
    /// 设置是否在所有事件节点之后显示等待节点。
    pub fn pending(mut self, v: bool) -> Self {
        self.pending = v;
        self
    }

    /// 设置是否反向展示事件节点；等待节点仍位于末尾。
    pub fn reverse(mut self, v: bool) -> Self {
        self.reverse = v;
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(400.0, self.row_count().max(1) as f32 * ITEM_HEIGHT)
    }

    fn row_count(&self) -> usize {
        self.items.len() + usize::from(self.pending)
    }

    fn item_at(&self, index: usize) -> &TimelineItem {
        if self.reverse {
            &self.items[self.items.len() - 1 - index]
        } else {
            &self.items[index]
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.items = next.items;
        self.pending = next.pending;
        self.reverse = next.reverse;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Timeline {
            items: self.items.clone(),
            pending: self.pending,
            reverse: self.reverse,
        }
    }
}

impl TimelineGeometry {
    fn new(frame: Rect, row_count: usize) -> Self {
        let frame = Rect::new(
            frame.x,
            frame.y,
            if frame.w.is_finite() {
                frame.w.max(0.0)
            } else {
                0.0
            },
            if frame.h.is_finite() {
                frame.h.max(0.0)
            } else {
                0.0
            },
        );
        let fitted_height = if row_count == 0 {
            ITEM_HEIGHT
        } else {
            frame.h / row_count as f32
        };
        let row_height = fitted_height.clamp(MIN_ITEM_HEIGHT, ITEM_HEIGHT);
        let gutter = (frame.w * 0.5).min(40.0);
        let line_x = frame.x + gutter * 0.4;
        let content_x = frame.x + gutter;
        let right_padding = (frame.w * 0.1).min(8.0);
        let content_width = (frame.x + frame.w - right_padding - content_x).max(0.0);
        let dot_radius = DOT_RADIUS
            .min(row_height * 0.18)
            .min((gutter * 0.18).max(0.0));
        Self {
            frame,
            row_height,
            line_x,
            content_x,
            content_width,
            dot_radius,
        }
    }

    fn row(self, index: usize) -> Rect {
        Rect::new(
            self.frame.x,
            self.frame.y + index as f32 * self.row_height,
            self.frame.w,
            self.row_height,
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct RowTextGeometry {
    label_y: f32,
    label_height: f32,
    label_font: f32,
    description_y: f32,
    description_height: f32,
    description_font: f32,
    dot_y: f32,
}

fn row_text_geometry(ctx: &mut PaintContext, row: Rect, has_description: bool) -> RowTextGeometry {
    let scale = (row.h / ITEM_HEIGHT).clamp(0.75, 1.0);
    // 标签字号从当前主题正文 token 派生。
    let label_font = ctx.tokens().font_size() * scale;
    // 描述字号从当前主题小号正文 token 派生。
    let description_font = ctx.tokens().font_size_sm() * scale;
    let label_height = ctx.line_box_height(label_font).min(row.h);
    let gap = if has_description { scale } else { 0.0 };
    let description_height = if has_description {
        ctx.line_box_height(description_font)
            .min((row.h - label_height - gap).max(0.0))
    } else {
        0.0
    };
    let block_height = label_height + gap + description_height;
    let label_y = row.y + ((row.h - block_height) * 0.5).max(0.0);
    let description_y = label_y + label_height + gap;
    RowTextGeometry {
        label_y,
        label_height,
        label_font,
        description_y,
        description_height,
        description_font,
        dot_y: (label_y + label_height * 0.5).clamp(row.y, row.y + row.h),
    }
}

fn draw_connector(
    ctx: &mut PaintContext,
    line_x: f32,
    previous_dot_y: f32,
    dot_y: f32,
    dot_radius: f32,
    color: Color,
) {
    let start_y = previous_dot_y + dot_radius;
    let end_y = dot_y - dot_radius;
    if end_y > start_y {
        ctx.fill_rect(
            Rect::new(line_x - 1.0, start_y, 2.0, end_y - start_y),
            color,
            None,
        );
    }
}

fn paint_single_line(
    ctx: &mut PaintContext,
    value: &str,
    frame: Rect,
    color: Color,
    font_size: f32,
) {
    if frame.w <= 0.0 || frame.h <= 0.0 {
        return;
    }
    let Some(value) = elide_single_line(ctx, value, font_size, frame.w) else {
        return;
    };
    ctx.push_clip(frame);
    ctx.draw_text_in_frame(&value, frame, color, font_size);
    ctx.pop_clip();
}

fn elide_single_line(
    ctx: &mut PaintContext,
    value: &str,
    font_size: f32,
    max_width: f32,
) -> Option<String> {
    if !max_width.is_finite() || max_width <= 0.0 {
        return None;
    }
    let value = value.replace(['\r', '\n'], " ");
    if text_width(ctx, &value, font_size) <= max_width {
        return Some(value);
    }
    const ELLIPSIS: &str = "…";
    if text_width(ctx, ELLIPSIS, font_size) > max_width {
        return None;
    }
    let mut visible = String::new();
    for ch in value.chars() {
        visible.push(ch);
        visible.push_str(ELLIPSIS);
        let fits = text_width(ctx, &visible, font_size) <= max_width;
        visible.pop();
        if !fits {
            visible.pop();
            break;
        }
    }
    visible.push_str(ELLIPSIS);
    Some(visible)
}

fn text_width(ctx: &mut PaintContext, value: &str, font_size: f32) -> f32 {
    ctx.measure_text(value, font_size).w.max(
        crate::draw::resources::font::text_backend::estimate_text_metrics(
            value,
            f32::INFINITY,
            font_size,
        )
        .max_line_width,
    )
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

impl TimelineItem {
    /// 使用主要标签创建跟随当前主题主色的节点。
    pub fn new(label: &str) -> Self {
        Self {
            // 构造阶段不拥有主题上下文，透明哨兵由绘制阶段解析为 color_primary。
            color: Color::TRANSPARENT,
            label: label.to_string(),
            description: String::new(),
        }
    }
    /// 设置节点的辅助描述文本。
    pub fn description(mut self, d: &str) -> Self {
        self.description = d.to_string();
        self
    }

    /// 设置节点的显式绘制颜色。
    pub fn color(mut self, c: Color) -> Self {
        // 显式颜色保持高于主题默认值的优先级。
        self.color = c;
        self
    }

    // 解析当前节点最终绘制颜色。
    fn resolved_dot_color(&self, theme_primary: Color) -> Color {
        // 透明哨兵表示调用方没有覆写节点颜色。
        if self.color == Color::TRANSPARENT {
            // 未覆写时返回绘制上下文解析的当前主题主色。
            theme_primary
        // 非透明颜色均视为调用方显式覆写。
        } else {
            // 显式覆写保持最高优先级。
            self.color
            // 结束颜色解析分支。
        }
    }
}

impl Default for TimelineItem {
    fn default() -> Self {
        Self::new("")
    }
}

// 验证时间线节点颜色优先级。
#[cfg(test)]
mod tests {
    // 引入被测节点构建器。
    use super::TimelineItem;
    // 引入最终绘制颜色值。
    use crate::draw::Color;

    // 默认节点跟随主题，显式颜色保持最高优先级。
    #[test]
    fn timeline_item_resolves_theme_and_explicit_colors() {
        // 使用与默认主题无关的测试主色。
        let theme_primary = Color::from_rgb(1, 2, 3);
        // 未覆写节点必须使用调用方提供的当前主题主色。
        let themed = TimelineItem::new("主题节点");
        // 核对主题默认路径。
        assert_eq!(themed.resolved_dot_color(theme_primary), theme_primary);
        // 构造明确不同的显式节点色。
        let explicit = Color::from_rgb(4, 5, 6);
        // 使用公开构建器登记显式颜色。
        let customized = TimelineItem::new("自定义节点").color(explicit);
        // 核对显式覆写优先于主题主色。
        assert_eq!(customized.resolved_dot_color(theme_primary), explicit);
    }
}
