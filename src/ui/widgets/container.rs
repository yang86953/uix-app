//! Container widget — flexbox layout container with background/border.

use crate::graphics::{AlignItems, FlexDirection, JustifyContent, Radius};
use crate::graphics::{Color, EdgeInsets, Rect, Size};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{Widget, WidgetId, WidgetTree};

pub struct Container {
    pub bg_color: Option<Color>,
    pub border_color: Option<Color>,
    pub border_width: f32,
    pub border_radius: f32,
    pub padding: EdgeInsets,
    pub gap: f32,
    pub direction: FlexDirection,
    pub justify: JustifyContent,
    pub align: AlignItems,
    pub fixed_width: Option<f32>,
    pub fixed_height: Option<f32>,
}

impl Container {
    pub fn new() -> Self {
        Self {
            bg_color: None,
            border_color: None,
            border_width: 0.0,
            border_radius: 0.0,
            padding: EdgeInsets::zero(),
            gap: 0.0,
            direction: FlexDirection::Row,
            justify: JustifyContent::Start,
            align: AlignItems::Stretch,
            fixed_width: None,
            fixed_height: None,
        }
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
    pub fn pad(mut self, p: EdgeInsets) -> Self {
        self.padding = p;
        self
    }
    pub fn gap(mut self, g: f32) -> Self {
        self.gap = g;
        self
    }
    pub fn dir(mut self, d: FlexDirection) -> Self {
        self.direction = d;
        self
    }
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.fixed_width = Some(w);
        self.fixed_height = Some(h);
        self
    }
}

impl Widget for Container {
    fn preferred_size(&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        // When no fixed size is set, return zero so the parent layout can
        // compute the actual size based on available space and children.
        Size::new(
            self.fixed_width.unwrap_or(0.0),
            self.fixed_height.unwrap_or(0.0),
        )
    }

    fn render(&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        // Background
        if let Some(c) = self.bg_color {
            let r = if self.border_radius > 0.0 {
                Some(Radius::uniform(self.border_radius))
            } else {
                None
            };
            ctx.fill_rect(frame, c, r);
        }
        // Border
        if let Some(c) = self.border_color {
            let r = if self.border_radius > 0.0 {
                Some(Radius::uniform(self.border_radius))
            } else {
                None
            };
            ctx.stroke_rect(frame, c, self.border_width, r);
        }
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

        let inner = Rect::new(
            frame.x + self.padding.left,
            frame.y + self.padding.top,
            (frame.w - self.padding.horizontal()).max(0.0),
            (frame.h - self.padding.vertical()).max(0.0),
        );
        if inner.w <= 0.0 || inner.h <= 0.0 {
            return result;
        }

        let is_row = matches!(self.direction, FlexDirection::Row);
        let total_gap = self.gap * (children.len() as f32 - 1.0);
        let avail_main = if is_row { inner.w } else { inner.h } - total_gap;
        let avail_cross = if is_row { inner.h } else { inner.w };

        // Collect child preferred sizes along the main axis
        let mut child_main_sizes: Vec<f32> = Vec::with_capacity(children.len());
        for &cid in children {
            if let Some(child) = tree.get(cid) {
                let ps = child.preferred_size(None);
                child_main_sizes.push(if is_row { ps.w } else { ps.h });
            } else {
                child_main_sizes.push(0.0);
            }
        }

        let total_pref: f32 = child_main_sizes.iter().sum();
        let remaining = avail_main - total_pref;

        // Determine start offset along main axis based on JustifyContent
        let start_offset = if remaining <= 0.0 {
            0.0
        } else {
            match self.justify {
                JustifyContent::Start | JustifyContent::Stretch => 0.0,
                JustifyContent::Center => remaining * 0.5,
                JustifyContent::End => remaining,
                JustifyContent::SpaceBetween => 0.0,
                JustifyContent::SpaceAround => remaining * 0.5 / children.len() as f32,
                JustifyContent::SpaceEvenly => remaining / (children.len() + 1) as f32,
            }
        };

        // Determine effective gap between children.
        // `remaining` is the space left over after subtracting preferred sizes
        // and the base gap (self.gap * (n-1)) from the available main-axis space.
        // For SpaceBetween/SpaceAround/SpaceEvenly, this remaining space is
        // distributed as additional gap between items.
        let effective_gap = if remaining <= 0.0 {
            self.gap
        } else {
            match self.justify {
                JustifyContent::SpaceBetween => self.gap + remaining / (children.len() - 1) as f32,
                JustifyContent::SpaceAround => self.gap + remaining / children.len() as f32,
                JustifyContent::SpaceEvenly => self.gap + remaining / (children.len() + 1) as f32,
                _ => self.gap,
            }
        };

        let mut cursor = if is_row {
            inner.x + start_offset
        } else {
            inner.y + start_offset
        };

        let last_idx = children.len().wrapping_sub(1);
        for (i, &cid) in children.iter().enumerate() {
            let main_size = child_main_sizes[i];

            // Cross-axis: stretch children to fill available space
            let cw = if is_row { main_size } else { avail_cross };
            let ch = if is_row { avail_cross } else { main_size };

            let rect = if is_row {
                Rect::new(cursor, inner.y, cw, ch)
            } else {
                Rect::new(inner.x, cursor, cw, ch)
            };
            result.push((cid, rect));

            cursor += if is_row { cw } else { ch };

            // Add gap only between children, not after the last one.
            if i != last_idx {
                cursor += effective_gap;
            }
        }

        result
    }
}
