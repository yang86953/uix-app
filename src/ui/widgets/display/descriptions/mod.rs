//! Descriptions widget — 描述列表，Ant Design 风格。
//!
//! 用于只读展示多条字段信息，支持 bordered、column 布局、label/value 键值对。

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Radius;
use crate::platform::windowing::ControlSize;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{SnapshotFields, WidgetTree};
use crate::widget;

// 组件默认尺寸（本组件设计值）；其他组件同名常量值不同，属各自设计。
const DEFAULT_WIDTH: f32 = 600.0;
const MIN_COLUMN_WIDTH: f32 = 140.0;
// 描述项标题行高（32.0）；calendar 标题行为 24.0，组件独立设计。
const TITLE_HEIGHT: f32 = 32.0;
// 描述项标题字号（15.0）；breadcrumb 为 13.0，组件独立设计。
const TITLE_FONT_SIZE: f32 = 15.0;
const ITEM_FONT_SIZE: f32 = 13.0;
const TEXT_LINE_HEIGHT: f32 = 1.5;
// 描述项水平内边距（8.0）；empty 为 16.0，组件独立设计。
const HORIZONTAL_PADDING: f32 = 8.0;
// 描述项垂直内边距（6.0）；empty 为 12.0，组件独立设计。
const VERTICAL_PADDING: f32 = 6.0;
const MAX_LABEL_FRACTION: f32 = 0.45;

#[derive(Debug, Clone, Copy)]
struct ItemPlacement {
    item_index: usize,
    row: usize,
    column: usize,
    span: usize,
}

/// 单个描述项。
#[derive(Debug, Clone, PartialEq)]
pub struct DescriptionsItem {
    /// 字段标签文本。
    pub label: String,
    /// 字段值文本。
    pub value: String,
    /// 该项占用的布局列数。
    pub span: usize,
}

// Descriptions — 描述列表。
widget! {
    /// 以标签和值组成的网格展示只读字段信息。
    pub struct Descriptions {
        title: String,
        items: Vec<DescriptionsItem>,
        bordered: bool,
        column: usize,
        label_width: f32,
        size: ControlSize,
    }

    measure => (&self, constraints: Constraints) -> Size {
        let width = constraints.clamp(Size::new(DEFAULT_WIDTH, 0.0)).w;
        let row_heights = self.row_heights(width);
        constraints.clamp(Size::new(
            width,
            self.title_height() + row_heights.iter().sum::<f32>(),
        ))
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let frame = Rect::new(frame.x, frame.y, frame.w.max(0.0), frame.h.max(0.0));
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border_secondary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let fill = ctx.tokens().color_fill_quaternary();
        let r = Radius::uniform(ctx.tokens().border_radius());
        let mut y = frame.y;
        let columns = self.effective_columns(frame.w);
        let col_w = frame.w / columns as f32;
        let row_heights = self.row_heights_for(frame.w, columns);
        let body_height = row_heights.iter().sum::<f32>();
        ctx.push_clip(frame);

        // 标题
        if !self.title.is_empty() {
            let title_rect = Rect::new(frame.x, y, frame.w, TITLE_HEIGHT.min(frame.h));
            let text_rect = Rect::new(
                title_rect.x + 12.0,
                title_rect.y,
                (title_rect.w - 24.0).max(0.0),
                title_rect.h,
            );
            // 复用 UI 绘制上下文拥有的保守单行省略算法。
            if let Some(title) = ctx.elide_single_line(&self.title, TITLE_FONT_SIZE, text_rect.w) {
                let ty = ctx.visual_center_y(title_rect, TITLE_FONT_SIZE);
                ctx.push_clip(text_rect);
                ctx.draw_text(&title, Point::new(text_rect.x, ty), text, TITLE_FONT_SIZE);
                ctx.pop_clip();
            }
            y += TITLE_HEIGHT;
        }

        // 主体背景
        if self.bordered {
            let body = Rect::new(
                frame.x,
                y,
                frame.w,
                body_height.min((frame.y + frame.h - y).max(0.0)),
            );
            ctx.fill_rect(body, bg, Some(r));
            ctx.stroke_rect(body, border, 1.0, Some(r));
        }

        // 按 span 顺序装箱；放不下的条目从下一行开始。
        let mut current_row = 0usize;
        let mut row_y = y;
        for placement in self.item_placements(columns) {
            while current_row < placement.row {
                row_y += row_heights[current_row];
                current_row += 1;
            }
            let item = &self.items[placement.item_index];
            let item_x = frame.x + placement.column as f32 * col_w;
            let item_w = placement.span as f32 * col_w;
            let item_h = row_heights[placement.row];
            let label_width = self.effective_label_width(item_w);
            let row_rect = Rect::new(item_x, row_y, item_w, item_h);
            let label_rect = Rect::new(item_x, row_y, label_width, item_h);
            let value_rect = Rect::new(
                item_x + label_width,
                row_y,
                (item_w - label_width).max(0.0),
                item_h,
            );
            if self.bordered {
                ctx.fill_rect(label_rect, fill, None);
                ctx.stroke_rect(row_rect, border, 1.0, None);
            }
            draw_wrapped_cell_text(ctx, label_rect, &item.label, text_sec);
            draw_wrapped_cell_text(ctx, value_rect, &item.value, text);
        }
        ctx.pop_clip();
    }
}

// 把描述项数据、网格算法与绘制内核融合为 UIX 声明的单一叶节点。
fn build_descriptions_view(kernel: Descriptions) -> ViewNode {
    ViewNode::leaf(kernel)
}

impl View for Descriptions {
    fn build(self) -> ViewNode {
        // UIX 拥有公开组件根，Rust 内核继续独占数据布局、换行与绘制。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/descriptions/descriptions.uix")
    }
}

impl Descriptions {
    /// 创建三列、无边框且使用全局控件尺寸的空描述列表。
    pub fn new() -> Self {
        Self {
            title: String::new(),
            items: Vec::new(),
            bordered: false,
            column: 3,
            label_width: 100.0,
            size: crate::ui::widget_runtime::config::use_config().size,
        }
    }
    /// 设置描述列表标题。
    pub fn title(mut self, t: &str) -> Self {
        self.title = t.to_string();
        self
    }
    /// 替换描述列表中的全部项目。
    pub fn items(mut self, items: Vec<DescriptionsItem>) -> Self {
        self.items = items;
        self
    }
    #[allow(
        clippy::should_implement_trait,
        reason = "add is the established fluent builder API, not arithmetic addition"
    )]
    /// 在描述列表末尾追加一个项目。
    pub fn add(mut self, item: DescriptionsItem) -> Self {
        self.items.push(item);
        self
    }
    /// 设置是否绘制单元格边框。
    pub fn bordered(mut self, v: bool) -> Self {
        self.bordered = v;
        self
    }
    /// 设置每行列数；零会被规范化为一列。
    pub fn column(mut self, v: usize) -> Self {
        self.column = v.max(1);
        self
    }
    /// 设置标签区域宽度；非有限值归零，负值截断为零。
    pub fn label_width(mut self, w: f32) -> Self {
        self.label_width = Self::normalize_dimension(w);
        self
    }
    /// 设置描述列表的控件尺寸。
    pub fn size(mut self, s: ControlSize) -> Self {
        self.size = s;
        self
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Descriptions {
            title: self.title.clone(),
            items: self.items.clone(),
            bordered: self.bordered,
            column: self.column,
            label_width: self.label_width,
            descriptions_size: self.size,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.title = next.title;
        self.items = next.items;
        self.bordered = next.bordered;
        self.column = next.column.max(1);
        self.label_width = Self::normalize_dimension(next.label_width);
        self.size = next.size;
    }

    fn title_height(&self) -> f32 {
        if self.title.is_empty() {
            0.0
        } else {
            TITLE_HEIGHT
        }
    }

    fn base_item_height(&self) -> f32 {
        match self.size {
            ControlSize::Small => 28.0,
            ControlSize::Medium => 36.0,
            ControlSize::Large => 44.0,
        }
    }

    fn effective_columns(&self, width: f32) -> usize {
        let available = if width.is_finite() {
            width.max(0.0)
        } else {
            0.0
        };
        let fitting = (available / MIN_COLUMN_WIDTH).floor() as usize;
        self.column.min(fitting.max(1)).max(1)
    }

    fn item_placements(&self, columns: usize) -> impl Iterator<Item = ItemPlacement> + '_ {
        let columns = columns.max(1);
        let mut row = 0usize;
        let mut used_columns = 0usize;
        self.items
            .iter()
            .enumerate()
            .map(move |(item_index, item)| {
                let span = item.span.clamp(1, columns);
                if used_columns == columns || used_columns + span > columns {
                    row += 1;
                    used_columns = 0;
                }
                let placement = ItemPlacement {
                    item_index,
                    row,
                    column: used_columns,
                    span,
                };
                used_columns += span;
                placement
            })
    }

    fn row_heights(&self, width: f32) -> Vec<f32> {
        let columns = self.effective_columns(width);
        self.row_heights_for(width, columns)
    }

    fn row_heights_for(&self, width: f32, columns: usize) -> Vec<f32> {
        let mut heights: Vec<f32> = Vec::new();
        let col_w = width.max(0.0) / columns.max(1) as f32;
        for placement in self.item_placements(columns) {
            if placement.row == heights.len() {
                heights.push(self.base_item_height());
            }
            let item = &self.items[placement.item_index];
            let item_w = placement.span as f32 * col_w;
            let label_width = self.effective_label_width(item_w);
            let value_width = (item_w - label_width).max(0.0);
            let label_height = wrapped_text_height(&item.label, label_width);
            let value_height = wrapped_text_height(&item.value, value_width);
            heights[placement.row] =
                heights[placement.row].max(label_height.max(value_height) + VERTICAL_PADDING * 2.0);
        }
        heights
    }

    fn effective_label_width(&self, item_width: f32) -> f32 {
        self.label_width
            .min(item_width.max(0.0) * MAX_LABEL_FRACTION)
            .max(0.0)
    }

    fn normalize_dimension(value: f32) -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    }
}

impl Default for Descriptions {
    fn default() -> Self {
        Self::new()
    }
}

impl DescriptionsItem {
    /// 创建占一列的标签和值描述项。
    pub fn new(label: &str, value: &str) -> Self {
        Self {
            label: label.to_string(),
            value: value.to_string(),
            span: 1,
        }
    }
    /// 设置项目占用的列数；零会被规范化为一列。
    pub fn span(mut self, s: usize) -> Self {
        self.span = s.max(1);
        self
    }
}

// 集中验证 UIX 声明壳与描述列表 Rust 内核的单叶契约。
#[cfg(test)]
#[path = "../../../../../tests/unit/ui/widgets/display/descriptions_tests.rs"]
mod tests;

fn wrapped_text_height(text: &str, region_width: f32) -> f32 {
    let text_width = region_width - HORIZONTAL_PADDING * 2.0;
    if text_width <= 0.0 {
        return 0.0;
    }
    crate::draw::resources::font::text_backend::estimate_text_metrics(
        text,
        text_width,
        ITEM_FONT_SIZE,
    )
    .line_count
    .max(1) as f32
        * ITEM_FONT_SIZE
        * TEXT_LINE_HEIGHT
}

fn draw_wrapped_cell_text(
    ctx: &mut PaintContext,
    region: Rect,
    text: &str,
    color: crate::draw::Color,
) {
    let text_rect = Rect::new(
        region.x + HORIZONTAL_PADDING,
        region.y + VERTICAL_PADDING,
        (region.w - HORIZONTAL_PADDING * 2.0).max(0.0),
        (region.h - VERTICAL_PADDING * 2.0).max(0.0),
    );
    if text_rect.w <= 0.0 || text_rect.h <= 0.0 {
        return;
    }
    ctx.push_clip(text_rect);
    ctx.draw_text_wrapped(text, text_rect, color, ITEM_FONT_SIZE);
    ctx.pop_clip();
}
