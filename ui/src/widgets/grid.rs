//! Grid widget — CSS Grid-like layout container.

use crate::define_widget;
use uix_graphics::{Color, Radius};
use crate::{compute_grid_layout, AlignItems, GridChild, GridInput, GridTrack, JustifyContent};
use uix_core::{EdgeInsets, Rect, Size};
use crate::render_context::RenderContext;
use crate::widget::{WidgetId, WidgetTree};

define_widget! {
    /// Grid container widget.
    pub struct Grid {
        columns: Vec<GridTrack>,
        rows: Vec<GridTrack>,
        col_gap: f32,
        row_gap: f32,
        padding: EdgeInsets,
        bg_color: Option<Color>,
        border_color: Option<Color>,
        border_width: f32,
        border_radius: f32,
        align_items: AlignItems,
        justify_items: JustifyContent,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        Size::new(self.fixed_width.unwrap_or(0.0), self.fixed_height.unwrap_or(0.0))
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if let Some(c) = self.bg_color {
            let r = if self.border_radius > 0.0 { Some(Radius::uniform(self.border_radius)) } else { None };
            ctx.fill_rect(frame, c, r);
        }
        if let Some(c) = self.border_color {
            let r = if self.border_radius > 0.0 { Some(Radius::uniform(self.border_radius)) } else { None };
            ctx.stroke_rect(frame, c, self.border_width, r);
        }
    }

    layout_children => (&self, frame: Rect, children: &[WidgetId], tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        let mut result = Vec::new();
        if self.columns.is_empty() || children.is_empty() { return result; }

        // 自动计算行数：如果 rows 未设置，根据子节点数和列数推算
        let rows: Vec<GridTrack> = if self.rows.is_empty() {
            let n_cols = self.columns.len();
            let n_rows = (children.len() + n_cols - 1) / n_cols; // ceil division
            vec![GridTrack::Auto; n_rows.max(1)]
        } else {
            self.rows.clone()
        };

        let grid_children: Vec<GridChild> = children.iter().map(|&cid| {
            GridChild { preferred_size: tree.get(cid).map(|c| c.preferred_size(None)).unwrap_or_default(), ..Default::default() }
        }).collect();

        let output = compute_grid_layout(&GridInput {
            container: frame,
            columns: self.columns.clone(),
            rows,
            col_gap: self.col_gap,
            row_gap: self.row_gap,
            padding: self.padding,
            children: grid_children,
            align_items: self.align_items,
            justify_items: self.justify_items,
        });

        for (i, &cid) in children.iter().enumerate() {
            if i < output.child_rects.len() { result.push((cid, output.child_rects[i])); }
        }
        result
    }
}

impl Default for Grid {
    fn default() -> Self { Self::new() }
}

impl Grid {
    pub fn new() -> Self {
        Self {
            columns: Vec::new(), rows: Vec::new(),
            col_gap: 0.0, row_gap: 0.0,
            padding: EdgeInsets::zero(),
            bg_color: None, border_color: None, border_width: 0.0, border_radius: 0.0,
            align_items: AlignItems::Stretch, justify_items: JustifyContent::Start,
            fixed_width: None, fixed_height: None,
        }
    }

    pub fn columns(mut self, cols: Vec<GridTrack>) -> Self { self.columns = cols; self }
    pub fn rows(mut self, rows: Vec<GridTrack>) -> Self { self.rows = rows; self }
    pub fn col_gap(mut self, gap: f32) -> Self { self.col_gap = gap; self }
    pub fn row_gap(mut self, gap: f32) -> Self { self.row_gap = gap; self }
    pub fn gap(mut self, g: f32) -> Self { self.col_gap = g; self.row_gap = g; self }
    pub fn pad(mut self, p: EdgeInsets) -> Self { self.padding = p; self }
    pub fn bg(mut self, c: Color) -> Self { self.bg_color = Some(c); self }
    pub fn border(mut self, c: Color, w: f32) -> Self { self.border_color = Some(c); self.border_width = w; self }
    pub fn rounded(mut self, r: f32) -> Self { self.border_radius = r; self }
    pub fn size(mut self, w: f32, h: f32) -> Self { self.fixed_width = Some(w); self.fixed_height = Some(h); self }
    pub fn align(mut self, a: AlignItems) -> Self { self.align_items = a; self }
    pub fn justify(mut self, j: JustifyContent) -> Self { self.justify_items = j; self }

    pub fn two_columns() -> Self {
        Self::new().columns(vec![GridTrack::Fr(1.0), GridTrack::Fr(1.0)])
    }
    pub fn three_columns() -> Self {
        Self::new().columns(vec![GridTrack::Fr(1.0), GridTrack::Fr(1.0), GridTrack::Fr(1.0)])
    }
}
