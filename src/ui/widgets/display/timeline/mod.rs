//! Timeline widget — 时间线组件，Ant Design 风格。
//!
//! 垂直时间轴展示事件序列，支持节点颜色、标签、描述。

use crate::core::{Constraints, Rect, Size};
use crate::draw::Color;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::ui::{SnapshotFields, ThemeTokens};
use crate::widget;

// 保存由 UIX 声明的时间线固有尺寸、行高与水平几何比例。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TimelineGeometryVisual {
    default_width: f32,
    item_height: f32,
    min_item_height: f32,
    gutter_frame_ratio: f32,
    gutter_max: f32,
    line_gutter_ratio: f32,
    right_padding_frame_ratio: f32,
    right_padding_max: f32,
    dot_radius: f32,
    dot_row_ratio: f32,
    dot_gutter_ratio: f32,
}

// 时间线文字使用的主题字号角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimelineFontRole {
    Body,
    Small,
}

impl TimelineFontRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Body => tokens.font_size(),
            Self::Small => tokens.font_size_sm(),
        }
    }
}

// 保存由 UIX 声明的时间线文字缩放、间距与垂直居中比例。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TimelineTypographyVisual {
    label_font: TimelineFontRole,
    description_font: TimelineFontRole,
    min_scale: f32,
    max_scale: f32,
    description_gap_scale: f32,
    center_ratio: f32,
}

// 保存由 UIX 声明的节点边框与连接线几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TimelineStrokeVisual {
    dot_border_max: f32,
    connector_half_width: f32,
    connector_width: f32,
}

// 保存由 UIX 声明的时间线主题语义色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TimelinePaletteVisual {
    text: ColorValue,
    text_secondary: ColorValue,
    border: ColorValue,
    dot_border: ColorValue,
    primary: ColorValue,
}

// 完整视觉配置由全部 Timeline 实例共享，实例只保存一个静态引用。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TimelineVisual {
    geometry: TimelineGeometryVisual,
    typography: TimelineTypographyVisual,
    stroke: TimelineStrokeVisual,
    palette: TimelinePaletteVisual,
}

// 同目录 UIX 生成四组视觉记录、根视觉记录及稳定静态借用。
crate::uix_items!("src/ui/widgets/display/timeline/timeline.uix");

// 保存 Timeline 每帧只解析一次的主题颜色与字号。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedTimelineVisual {
    text: Color,
    text_secondary: Color,
    border: Color,
    dot_border: Color,
    primary: Color,
    label_font: f32,
    description_font: f32,
}

impl TimelineVisual {
    fn resolve(self, tokens: &dyn ThemeTokens) -> ResolvedTimelineVisual {
        ResolvedTimelineVisual {
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            dot_border: self.palette.dot_border.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            label_font: self.typography.label_font.resolve(tokens),
            description_font: self.typography.description_font.resolve(tokens),
        }
    }
}

// 向 UIX 提供时间线主题字号与颜色角色。
const fn timeline_body_font() -> TimelineFontRole {
    TimelineFontRole::Body
}
const fn timeline_small_font() -> TimelineFontRole {
    TimelineFontRole::Small
}
const fn timeline_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
const fn timeline_secondary_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
const fn timeline_border_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BorderSecondary)
}
const fn timeline_dot_border_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
const fn timeline_primary_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}

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
widget! {
    /// 按声明顺序展示状态节点、标签与描述的时间线组件。
    pub struct Timeline {
        items: Vec<TimelineItem>,
        pending: bool,
        reverse: bool,
        #[snapshot(skip)]
        visual: &'static TimelineVisual,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let loc = crate::ui::widget_runtime::locale::use_locale();
        let row_count = self.row_count();
        let geometry = TimelineGeometry::new(frame, row_count, self.visual.geometry);
        if row_count == 0 || geometry.frame.w <= 0.0 || geometry.frame.h <= 0.0 {
            return;
        }
        // 整个可见时间线共享一次主题解析，不让行数放大 token 读取成本。
        let resolved = self.visual.resolve(ctx.tokens());
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
                    item.resolved_dot_color(resolved.primary),
                    false,
                )
            } else {
                (loc.timeline_pending, "", resolved.border, true)
            };
            let text_geometry = row_text_geometry(
                ctx,
                row,
                !description.is_empty(),
                self.visual,
                resolved,
            );
            let dot_y = text_geometry.dot_y;
            if let Some(previous_dot_y) = previous_dot_y {
                draw_connector(
                    ctx,
                    geometry.line_x,
                    previous_dot_y,
                    dot_y,
                    geometry.dot_radius,
                    resolved.border,
                    self.visual.stroke,
                );
            }
            if pending {
                ctx.stroke_circle(
                    geometry.line_x,
                    dot_y,
                    geometry.dot_radius,
                    resolved.border,
                    geometry
                        .dot_radius
                        .min(self.visual.stroke.dot_border_max),
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
                    resolved.dot_border,
                    geometry
                        .dot_radius
                        .min(self.visual.stroke.dot_border_max),
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
                if pending {
                    resolved.text_secondary
                } else {
                    resolved.text
                },
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
                    resolved.text_secondary,
                    text_geometry.description_font,
                );
            }
            previous_dot_y = Some(dot_y);
        }
        ctx.pop_clip();
    }
}

// 把时间线数据、索引策略与 UIX 视觉表融合为单一根节点。
fn build_timeline_view(mut kernel: Timeline, visual: &'static TimelineVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Timeline {
    fn build(self) -> ViewNode {
        // UIX 拥有时间线公开根与静态视觉；Rust 内核继续拥有数据、语言环境与绘制。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/timeline/timeline.uix")
    }
}

impl Timeline {
    /// 创建不含节点且未启用等待状态的时间线。
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            pending: false,
            reverse: false,
            visual: TIMELINE_VISUAL_REF,
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
        Size::new(
            self.visual.geometry.default_width,
            self.row_count().max(1) as f32 * self.visual.geometry.item_height,
        )
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
        self.visual = next.visual;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Timeline {
            items: self.items.clone(),
            pending: self.pending,
            reverse: self.reverse,
        }
    }

    // 测试目标观察 UIX 声明的关键视觉契约，不暴露到公开 API。
    #[cfg(test)]
    fn visual_contract_for_test(&self) -> (f32, f32, f32, f32, f32) {
        (
            self.visual.geometry.default_width,
            self.visual.geometry.item_height,
            self.visual.geometry.min_item_height,
            self.visual.geometry.dot_radius,
            self.visual.stroke.connector_width,
        )
    }

    // 测试目标确认实例共享同一份 UIX 视觉表。
    #[cfg(test)]
    fn shares_visual_with_for_test(&self, other: &Self) -> bool {
        std::ptr::eq(self.visual, other.visual)
    }
}

impl TimelineGeometry {
    fn new(frame: Rect, row_count: usize, visual: TimelineGeometryVisual) -> Self {
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
            visual.item_height
        } else {
            frame.h / row_count as f32
        };
        let row_height = fitted_height.clamp(visual.min_item_height, visual.item_height);
        let gutter = (frame.w * visual.gutter_frame_ratio).min(visual.gutter_max);
        let line_x = frame.x + gutter * visual.line_gutter_ratio;
        let content_x = frame.x + gutter;
        let right_padding =
            (frame.w * visual.right_padding_frame_ratio).min(visual.right_padding_max);
        let content_width = (frame.x + frame.w - right_padding - content_x).max(0.0);
        let dot_radius = visual
            .dot_radius
            .min(row_height * visual.dot_row_ratio)
            .min((gutter * visual.dot_gutter_ratio).max(0.0));
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

fn row_text_geometry(
    ctx: &mut PaintContext,
    row: Rect,
    has_description: bool,
    visual: &'static TimelineVisual,
    resolved: ResolvedTimelineVisual,
) -> RowTextGeometry {
    let scale = (row.h / visual.geometry.item_height)
        .clamp(visual.typography.min_scale, visual.typography.max_scale);
    // 标签字号从 UIX 声明的当前主题角色派生。
    let label_font = resolved.label_font * scale;
    // 描述字号从 UIX 声明的当前主题角色派生。
    let description_font = resolved.description_font * scale;
    let label_height = ctx.line_box_height(label_font).min(row.h);
    let gap = if has_description { scale } else { 0.0 };
    let gap = gap * visual.typography.description_gap_scale;
    let description_height = if has_description {
        ctx.line_box_height(description_font)
            .min((row.h - label_height - gap).max(0.0))
    } else {
        0.0
    };
    let block_height = label_height + gap + description_height;
    let label_y = row.y + ((row.h - block_height) * visual.typography.center_ratio).max(0.0);
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
    visual: TimelineStrokeVisual,
) {
    let start_y = previous_dot_y + dot_radius;
    let end_y = dot_y - dot_radius;
    if end_y > start_y {
        ctx.fill_rect(
            Rect::new(
                line_x - visual.connector_half_width,
                start_y,
                visual.connector_width,
                end_y - start_y,
            ),
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
    // 复用 UI 绘制上下文拥有的保守单行省略算法。
    let Some(value) = ctx.elide_single_line_cow(value, font_size, frame.w) else {
        return;
    };
    ctx.push_clip(frame);
    ctx.draw_text_in_frame(&value, frame, color, font_size);
    ctx.pop_clip();
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
