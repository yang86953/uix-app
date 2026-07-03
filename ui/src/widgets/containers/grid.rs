//! Grid widget — CSS Grid-like layout container.

use crate::api::traits::LayoutEngine;
use crate::define_widget;
use crate::layout::engine::{child_from_tree, GridLayout, LayoutChild};
use crate::layout::{AlignItems, GridTrack, JustifyContent};
use crate::render_context::RenderContext;
use crate::widget::{WidgetId, WidgetTree};
use uix_graphics::{Color, Radius};
use uix_platform::{EdgeInsets, Rect, Size};

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
        if self.columns.is_empty() || children.is_empty() { return Vec::new(); }

        // 构建统一子节点信息
        let layout_children: Vec<LayoutChild> = children
            .iter()
            .map(|&cid| child_from_tree(cid, tree))
            .collect();

        // 委托给统一的 GridLayout 布局引擎
        let engine = GridLayout {
            columns: self.columns.clone(),
            rows: self.rows.clone(),
            col_gap: self.col_gap,
            row_gap: self.row_gap,
            align_items: self.align_items,
            justify_items: self.justify_items,
        };

        // 内容区域（Grid 无 margin/border，仅扣除 padding）
        let content_rect = Rect::new(
            frame.x + self.padding.left,
            frame.y + self.padding.top,
            (frame.w - self.padding.horizontal()).max(0.0),
            (frame.h - self.padding.vertical()).max(0.0),
        );

        let output = engine.layout(content_rect, &layout_children);

        children.iter()
            .zip(output.positions)
            .map(|(&cid, rect)| (cid, rect))
            .collect()
    }
}

impl Default for Grid {
    fn default() -> Self {
        Self::new()
    }
}

impl Grid {
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            col_gap: 0.0,
            row_gap: 0.0,
            padding: EdgeInsets::zero(),
            bg_color: None,
            border_color: None,
            border_width: 0.0,
            border_radius: 0.0,
            align_items: AlignItems::Stretch,
            justify_items: JustifyContent::Start,
            fixed_width: None,
            fixed_height: None,
        }
    }

    pub fn columns(mut self, cols: Vec<GridTrack>) -> Self {
        self.columns = cols;
        self
    }
    pub fn rows(mut self, rows: Vec<GridTrack>) -> Self {
        self.rows = rows;
        self
    }
    pub fn col_gap(mut self, gap: f32) -> Self {
        self.col_gap = gap;
        self
    }
    pub fn row_gap(mut self, gap: f32) -> Self {
        self.row_gap = gap;
        self
    }
    pub fn gap(mut self, g: f32) -> Self {
        self.col_gap = g;
        self.row_gap = g;
        self
    }
    pub fn pad(mut self, p: EdgeInsets) -> Self {
        self.padding = p;
        self
    }
    pub fn bg(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }
    pub fn border(mut self, c: Color, w: f32) -> Self {
        self.border_color = Some(c);
        self.border_width = w;
        self
    }
    pub fn rounded(mut self, r: f32) -> Self {
        self.border_radius = r;
        self
    }
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.fixed_width = Some(w);
        self.fixed_height = Some(h);
        self
    }
    pub fn align(mut self, a: AlignItems) -> Self {
        self.align_items = a;
        self
    }
    pub fn justify(mut self, j: JustifyContent) -> Self {
        self.justify_items = j;
        self
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
}
