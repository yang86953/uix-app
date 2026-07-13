//! Grid widget - CSS Grid-like layout container.

use crate::component;
use crate::core::{Constraints, EdgeInsets, Rect, Size};
use crate::draw::compositor::PicturePolicy;
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::ui::layout::engine::{
    child_from_tree_with_constraints, BoxModel, GridLayout, LayoutChild,
};
use crate::ui::layout::{AlignItems, GridTrack, JustifyContent};
use crate::ui::style::{apply_style as paint_style, ColorValue, DisplayMode, Style};
use crate::ui::traits::layout::LayoutEngine;
use crate::ui::{ComponentId, WidgetTree};
use crate::ui::{SnapshotFields, SnapshotSource};

component! {
    /// Grid container widget.
    pub struct Grid {
        pub style: Style,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(
            self.style.width.unwrap_or(0.0),
            self.style.height.unwrap_or(0.0),
        ))
    }

    layout_margin => (&self) -> EdgeInsets { self.style.margin }

    flex_grow => (&self) -> f32 { self.style.flex_grow }

    flex_shrink => (&self) -> f32 { self.style.flex_shrink }

    align_self => (&self) -> Option<AlignItems> { self.style.align_self }

    grid_cell => (&self) -> Option<usize> { self.style.grid_cell }

    grid_column_span => (&self) -> u32 { self.style.grid_column_span }

    grid_row_span => (&self) -> u32 { self.style.grid_row_span }

    picture_policy => (&self) -> PicturePolicy { PicturePolicy::Eligible }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let visual = Rect::new(
            frame.x + self.style.margin.left,
            frame.y + self.style.margin.top,
            (frame.w - self.style.margin.horizontal()).max(0.0),
            (frame.h - self.style.margin.vertical()).max(0.0),
        );
        if visual.w <= 0.0 || visual.h <= 0.0 { return; }
        paint_style(ctx, visual, &self.style);
    }

    measure_children => (&self, frame: Rect, children: &[ComponentId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        if self.style.grid_template_columns.is_empty() || children.is_empty() {
            return Vec::new();
        }

        let content_rect = BoxModel {
            margin: self.style.margin,
            border_width: self.style.border_width,
            padding: self.style.padding,
        }
        .content_rect(frame);
        if content_rect.w <= 0.0 || content_rect.h <= 0.0 {
            return Vec::new();
        }

        let constraints = Constraints::loose(Size::new(content_rect.w, content_rect.h));
        children
            .iter()
            .copied()
            .map(|cid| child_from_tree_with_constraints(cid, tree, constraints))
            .collect()
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        if self.style.grid_template_columns.is_empty() || children.is_empty() {
            return Vec::new();
        }

        let box_model = BoxModel {
            margin: self.style.margin,
            border_width: self.style.border_width,
            padding: self.style.padding,
        };
        let content_rect = box_model.content_rect(frame);
        if content_rect.w <= 0.0 || content_rect.h <= 0.0 {
            return Vec::new();
        }

        let engine = GridLayout {
            columns: self.style.grid_template_columns.clone(),
            rows: self.style.grid_template_rows.clone(),
            col_gap: self.effective_col_gap(),
            row_gap: self.effective_row_gap(),
            align_items: self.style.align_items,
            justify_items: self.style.justify_content,
        };

        let output = engine.layout(content_rect, children);

        children
            .iter()
            .zip(output.positions)
            .map(|(child, rect)| (child.id, rect))
            .collect()
    }
}

impl Default for Grid {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotSource for Grid {
    fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Grid {
            style: self.style.clone(),
        }
    }
}

impl Grid {
    pub fn new() -> Self {
        Self {
            style: Style::default().with_display(DisplayMode::Grid),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.style = next.style;
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style.with_display(DisplayMode::Grid);
        self
    }

    pub fn columns(mut self, cols: Vec<GridTrack>) -> Self {
        self.style.grid_template_columns = cols;
        self
    }

    pub fn rows(mut self, rows: Vec<GridTrack>) -> Self {
        self.style.grid_template_rows = rows;
        self
    }

    pub fn col_gap(mut self, gap: f32) -> Self {
        self.style.grid_column_gap = gap;
        self
    }

    pub fn row_gap(mut self, gap: f32) -> Self {
        self.style.grid_row_gap = gap;
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.style.gap = gap;
        self.style.grid_column_gap = gap;
        self.style.grid_row_gap = gap;
        self
    }

    pub fn pad(mut self, padding: EdgeInsets) -> Self {
        self.style.padding = padding;
        self
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.style.background = Some(ColorValue::Custom(color));
        self
    }

    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.style.border_color = Some(ColorValue::Custom(color));
        self.style.border_width = EdgeInsets::uniform(width);
        self
    }

    pub fn rounded(mut self, radius: f32) -> Self {
        self.style.border_radius = radius;
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.style.width = Some(width);
        self.style.height = Some(height);
        self
    }

    pub fn align(mut self, align: AlignItems) -> Self {
        self.style.align_items = align;
        self
    }

    pub fn justify(mut self, justify: JustifyContent) -> Self {
        self.style.justify_content = justify;
        self
    }

    pub fn apply_style(&mut self, style: &Style) {
        self.style = self.style.clone().apply(style.clone());
        self.style.display = DisplayMode::Grid;
    }

    pub fn two_columns() -> Self {
        Self::new().columns(vec![GridTrack::Fr(1.0), GridTrack::Fr(1.0)])
    }

    pub fn three_columns() -> Self {
        Self::new().columns(vec![
            GridTrack::Fr(1.0),
            GridTrack::Fr(1.0),
            GridTrack::Fr(1.0),
        ])
    }

    fn effective_col_gap(&self) -> f32 {
        if self.style.grid_column_gap != 0.0 {
            self.style.grid_column_gap
        } else {
            self.style.gap
        }
    }

    fn effective_row_gap(&self) -> f32 {
        if self.style.grid_row_gap != 0.0 {
            self.style.grid_row_gap
        } else {
            self.style.gap
        }
    }
}

