//! Space widget — Ant Design style flex container with uniform gap between children.
//!
//! Provides consistent spacing for a row or column of child widgets.

use std::cell::RefCell;

use crate::graphics::{AlignItems, FlexDirection, JustifyContent};
use crate::graphics::{Rect, Size};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{Widget, WidgetId, WidgetTree};

/// Predefined space sizes matching Ant Design.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpaceSize {
    Small,  // 8px
    Middle, // 16px
    Large,  // 24px
    Custom(f32),
}

impl SpaceSize {
    pub fn value(&self) -> f32 {
        match self {
            Self::Small => 8.0,
            Self::Middle => 16.0,
            Self::Large => 24.0,
            Self::Custom(v) => *v,
        }
    }
}

/// Space — a flex container that adds uniform gap between its children.
pub struct Space {
    children: RefCell<Option<Vec<Box<dyn Widget>>>>,
    direction: FlexDirection,
    space_size: SpaceSize,
    #[allow(dead_code)]
    wrap: bool,
    justify: JustifyContent,
    align: AlignItems,
    fixed_width: Option<f32>,
    fixed_height: Option<f32>,
}

impl Space {
    pub fn new() -> Self {
        Self {
            children: RefCell::new(None),
            direction: FlexDirection::Row,
            space_size: SpaceSize::Small,
            wrap: false,
            justify: JustifyContent::Start,
            align: AlignItems::Center,
            fixed_width: None,
            fixed_height: None,
        }
    }

    pub fn child(self, w: impl Widget + 'static) -> Self {
        if self.children.borrow().is_none() {
            *self.children.borrow_mut() = Some(Vec::new());
        }
        self.children
            .borrow_mut()
            .as_mut()
            .map(|v| v.push(Box::new(w)));
        self
    }

    pub fn children(self, widgets: Vec<Box<dyn Widget>>) -> Self {
        *self.children.borrow_mut() = Some(widgets);
        self
    }

    pub fn direction(mut self, d: FlexDirection) -> Self {
        self.direction = d;
        self
    }
    pub fn size(mut self, s: SpaceSize) -> Self {
        self.space_size = s;
        self
    }
    pub fn wrap(mut self) -> Self {
        self.wrap = true;
        self
    }
    pub fn justify(mut self, j: JustifyContent) -> Self {
        self.justify = j;
        self
    }
    pub fn align(mut self, a: AlignItems) -> Self {
        self.align = a;
        self
    }
    pub fn width(mut self, w: f32) -> Self {
        self.fixed_width = Some(w);
        self
    }
    pub fn height(mut self, h: f32) -> Self {
        self.fixed_height = Some(h);
        self
    }
    pub fn vertical(mut self) -> Self {
        self.direction = FlexDirection::Column;
        self
    }
}

impl Widget for Space {
    fn preferred_size(&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        Size::new(
            self.fixed_width.unwrap_or(0.0),
            self.fixed_height.unwrap_or(0.0),
        )
    }

    fn build(&self) -> Vec<Box<dyn Widget>> {
        self.children.borrow_mut().take().unwrap_or_default()
    }

    fn render(&self, _frame: Rect, _ctx: &mut RenderContext, _tree: &WidgetTree) {
        // Space itself is invisible; children are rendered by the tree.
    }

    fn layout_children(
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
    ) -> Vec<(WidgetId, Rect)> {
        let mut result = Vec::new();
        if children.is_empty() {
            return result;
        }

        let gap = self.space_size.value();
        let is_row = matches!(self.direction, FlexDirection::Row | FlexDirection::RowReverse);
        let is_reverse = matches!(self.direction, FlexDirection::RowReverse | FlexDirection::ColumnReverse);

        // Collect child preferred sizes along the main and cross axes
        let mut child_main_sizes: Vec<f32> = Vec::with_capacity(children.len());
        let mut child_cross_sizes: Vec<f32> = Vec::with_capacity(children.len());
        for &cid in children {
            if let Some(child) = tree.get(cid) {
                let ps = child.preferred_size(None);
                child_main_sizes.push(if is_row { ps.w } else { ps.h });
                child_cross_sizes.push(if is_row { ps.h } else { ps.w });
            } else {
                child_main_sizes.push(0.0);
                child_cross_sizes.push(0.0);
            }
        }

        let total_gap = gap * (children.len() - 1) as f32;
        let total_pref: f32 = child_main_sizes.iter().sum();
        let avail_main = if is_row { frame.w } else { frame.h } - total_gap;
        let avail_cross = if is_row { frame.h } else { frame.w };
        let remaining = (avail_main - total_pref).max(0.0);

        // Distribute remaining space evenly if justify is Stretch
        let adjusted_main_sizes = if remaining > 0.0 && self.justify == JustifyContent::Stretch {
            let extra_per_child = remaining / children.len() as f32;
            child_main_sizes.iter().map(|&s| s + extra_per_child).collect::<Vec<_>>()
        } else {
            child_main_sizes.clone()
        };

        let start_offset = match self.justify {
            JustifyContent::Center => remaining * 0.5,
            JustifyContent::End => remaining,
            JustifyContent::SpaceBetween => 0.0,
            JustifyContent::SpaceAround => remaining * 0.5 / children.len() as f32,
            JustifyContent::SpaceEvenly => remaining / (children.len() + 1) as f32,
            _ => 0.0, // Start, Stretch
        };

        // Effective gap for space-between/around/evenly
        let effective_gap = if remaining <= 0.0 || children.len() <= 1 {
            gap
        } else {
            match self.justify {
                JustifyContent::SpaceBetween => gap + remaining / (children.len() - 1) as f32,
                JustifyContent::SpaceAround => gap + remaining / children.len() as f32,
                JustifyContent::SpaceEvenly => gap + remaining / (children.len() + 1) as f32,
                _ => gap,
            }
        };

        let mut cursor = if is_reverse {
            let total_used: f32 = adjusted_main_sizes.iter().sum::<f32>() + total_gap;
            if is_row {
                frame.x + frame.w - total_used + start_offset
            } else {
                frame.y + frame.h - total_used + start_offset
            }
        } else {
            if is_row {
                frame.x + start_offset
            } else {
                frame.y + start_offset
            }
        };

        let last_idx = children.len().wrapping_sub(1);
        for (i, &cid) in children.iter().enumerate() {
            let ms = adjusted_main_sizes[i];

            // Cross-axis alignment
            let child_cross = match self.align {
                AlignItems::Start => child_cross_sizes[i].min(avail_cross),
                AlignItems::Center => child_cross_sizes[i].min(avail_cross),
                AlignItems::End => child_cross_sizes[i].min(avail_cross),
                AlignItems::Stretch => avail_cross,
            };
            let cross_offset = match self.align {
                AlignItems::Start => 0.0,
                AlignItems::Center => (avail_cross - child_cross) * 0.5,
                AlignItems::End => avail_cross - child_cross,
                AlignItems::Stretch => 0.0,
            };

            let (cw, ch) = if is_row { (ms, child_cross) } else { (child_cross, ms) };
            let (px, py) = if is_row {
                (cursor, frame.y + cross_offset)
            } else {
                (frame.x + cross_offset, cursor)
            };

            result.push((cid, Rect::new(px, py, cw, ch)));

            cursor += (if is_row { cw } else { ch }) + if i != last_idx { effective_gap } else { 0.0 };
        }

        result
    }
}
