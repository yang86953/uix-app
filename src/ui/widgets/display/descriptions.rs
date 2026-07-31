//! Descriptions widget — 描述列表，Ant Design 风格。
//!
//! 用于只读展示多条字段信息，支持 bordered、column 布局、label/value 键值对。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Radius;
use crate::native::traits::input::ControlSize;
use crate::ui::core::paint_context::PaintContext;
use crate::ui::{SnapshotFields, WidgetTree};

const DEFAULT_WIDTH: f32 = 600.0;
const MIN_COLUMN_WIDTH: f32 = 140.0;
const TITLE_HEIGHT: f32 = 32.0;
const TITLE_FONT_SIZE: f32 = 15.0;
const ITEM_FONT_SIZE: f32 = 13.0;
const TEXT_LINE_HEIGHT: f32 = 1.5;
const HORIZONTAL_PADDING: f32 = 8.0;
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
    pub label: String,
    pub value: String,
    pub span: usize,
}

// Descriptions — 描述列表。
component! {
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
        let placements = self.item_placements(columns);
        let row_heights = self.row_heights_for(frame.w, columns, &placements);
        let mut body_height = 0.0;
        let row_offsets = row_heights
            .iter()
            .map(|height| {
                let offset = body_height;
                body_height += *height;
                offset
            })
            .collect::<Vec<_>>();
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
            if let Some(title) = elide_single_line(ctx, &self.title, TITLE_FONT_SIZE, text_rect.w) {
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
        for placement in placements {
            let item = &self.items[placement.item_index];
            let item_x = frame.x + placement.column as f32 * col_w;
            let item_y = y + row_offsets[placement.row];
            let item_w = placement.span as f32 * col_w;
            let item_h = row_heights[placement.row];
            let label_width = self.effective_label_width(item_w);
            let row_rect = Rect::new(item_x, item_y, item_w, item_h);
            let label_rect = Rect::new(item_x, item_y, label_width, item_h);
            let value_rect = Rect::new(
                item_x + label_width,
                item_y,
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

impl Descriptions {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            items: Vec::new(),
            bordered: false,
            column: 3,
            label_width: 100.0,
            size: crate::ui::config::use_config().size,
        }
    }
    pub fn title(mut self, t: &str) -> Self {
        self.title = t.to_string();
        self
    }
    pub fn items(mut self, items: Vec<DescriptionsItem>) -> Self {
        self.items = items;
        self
    }
    #[allow(
        clippy::should_implement_trait,
        reason = "add is the established fluent builder API, not arithmetic addition"
    )]
    pub fn add(mut self, item: DescriptionsItem) -> Self {
        self.items.push(item);
        self
    }
    pub fn bordered(mut self, v: bool) -> Self {
        self.bordered = v;
        self
    }
    pub fn column(mut self, v: usize) -> Self {
        self.column = v.max(1);
        self
    }
    pub fn label_width(mut self, w: f32) -> Self {
        self.label_width = Self::normalize_dimension(w);
        self
    }
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

    fn item_placements(&self, columns: usize) -> Vec<ItemPlacement> {
        let columns = columns.max(1);
        let mut row = 0usize;
        let mut used_columns = 0usize;
        let mut placements = Vec::with_capacity(self.items.len());

        for (item_index, item) in self.items.iter().enumerate() {
            let span = item.span.clamp(1, columns);
            if used_columns == columns || used_columns + span > columns {
                row += 1;
                used_columns = 0;
            }
            placements.push(ItemPlacement {
                item_index,
                row,
                column: used_columns,
                span,
            });
            used_columns += span;
        }

        placements
    }

    fn row_heights(&self, width: f32) -> Vec<f32> {
        let columns = self.effective_columns(width);
        let placements = self.item_placements(columns);
        self.row_heights_for(width, columns, &placements)
    }

    fn row_heights_for(
        &self,
        width: f32,
        columns: usize,
        placements: &[ItemPlacement],
    ) -> Vec<f32> {
        let Some(last) = placements.last() else {
            return Vec::new();
        };
        let mut heights = vec![self.base_item_height(); last.row + 1];
        let col_w = width.max(0.0) / columns.max(1) as f32;
        for placement in placements {
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
    pub fn new(label: &str, value: &str) -> Self {
        Self {
            label: label.to_string(),
            value: value.to_string(),
            span: 1,
        }
    }
    pub fn span(mut self, s: usize) -> Self {
        self.span = s.max(1);
        self
    }
}

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

fn conservative_text_width(ctx: &mut PaintContext, text: &str, font_size: f32) -> f32 {
    ctx.measure_text(text, font_size).w.max(
        crate::draw::resources::font::text_backend::estimate_text_metrics(
            text,
            f32::INFINITY,
            font_size,
        )
        .max_line_width,
    )
}

fn elide_single_line(
    ctx: &mut PaintContext,
    text: &str,
    font_size: f32,
    max_width: f32,
) -> Option<String> {
    if !max_width.is_finite() || max_width <= 0.0 {
        return None;
    }
    let text = text.replace(['\r', '\n'], " ");
    if conservative_text_width(ctx, &text, font_size) <= max_width {
        return Some(text);
    }
    const ELLIPSIS: &str = "…";
    if conservative_text_width(ctx, ELLIPSIS, font_size) > max_width {
        return None;
    }
    let mut visible = String::new();
    for ch in text.chars() {
        visible.push(ch);
        visible.push_str(ELLIPSIS);
        let fits = conservative_text_width(ctx, &visible, font_size) <= max_width;
        visible.pop();
        if !fits {
            visible.pop();
            break;
        }
    }
    visible.push_str(ELLIPSIS);
    Some(visible)
}
