//! Radio widget — 单选组，支持 horizontal/vertical、disabled、hover。

use crate::define_widget;
use crate::draw::painting::RenderContext;
use crate::ui::{EventResult, KeyCode, WidgetEvent, WidgetTree};
use crate::draw::{traits::GraphicsEngine, Radius};
use crate::native::{Point, Rect, Size};

/// 方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadioDirection {
    Horizontal,
    Vertical,
}

define_widget! {
    /// Radio — 单选按钮组。
    pub struct Radio {
        options: Vec<String>,
        selected: usize,
        disabled: bool,
        direction: RadioDirection,
        item_h: f32,
        hovered_idx: Option<usize>,
        focused: bool,
        on_change: Option<Box<dyn FnMut(usize) + 'static>>,
    }


    tab_index => (&self) -> i32 { 1 }
    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        let item_w = self.options.iter().map(|o| o.len() as f32 * 9.0 + 30.0).collect::<Vec<_>>();
        match self.direction {
            RadioDirection::Horizontal => {
                let w = item_w.iter().sum::<f32>().max(120.0);
                Size::new(w, self.item_h)
            }
            RadioDirection::Vertical => {
                let w = item_w.iter().cloned().max_by(|a,b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)).unwrap_or(120.0).max(120.0);
                Size::new(w, self.item_h * self.options.len() as f32)
            }
        }
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if self.disabled { return EventResult::NotHandled; }
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                if let Some(idx) = self.option_at(pos.x, pos.y) {
                    self.selected = idx;
                    if let Some(ref mut cb) = self.on_change { cb(idx); }
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            WidgetEvent::MouseMove { pos, .. } => {
                self.hovered_idx = self.option_at(pos.x, pos.y);
                EventResult::Handled
            }
            WidgetEvent::HoverEnter => { EventResult::Handled }
            WidgetEvent::HoverLeave => { self.hovered_idx = None; EventResult::Handled }
            WidgetEvent::FocusIn => { self.focused = true; EventResult::Handled }
            WidgetEvent::FocusOut => { self.focused = false; EventResult::Handled }
            WidgetEvent::KeyDown { key, .. } => {
            match key {
                KeyCode::Right | KeyCode::Down => {
                    let next = self.selected + 1;
                    if next < self.options.len() {
                        self.selected = next;
                        if let Some(ref mut cb) = self.on_change { cb(next); }
                    }
                    EventResult::Handled
                }
                KeyCode::Left | KeyCode::Up => {
                        if self.selected > 0 {
                            let prev = self.selected - 1;
                            self.selected = prev;
                            if let Some(ref mut cb) = self.on_change { cb(prev); }
                        }
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let cy = frame.y + self.item_h * 0.5;

        match self.direction {
            RadioDirection::Horizontal => {
                let mut x = frame.x;
                for (i, opt) in self.options.iter().enumerate() {
                    let w = opt.len() as f32 * 9.0 + 30.0;
                    self.render_radio_item(ctx, i, opt, x, cy, w);
                    x += w;
                }
            }
            RadioDirection::Vertical => {
                for (i, opt) in self.options.iter().enumerate() {
                    let y = frame.y + i as f32 * self.item_h + self.item_h * 0.5;
                    let w = frame.w;
                    self.render_radio_item(ctx, i, opt, frame.x, y, w);
                }
            }
        }
    }
}

impl Radio {
    fn option_at(&self, px: f32, py: f32) -> Option<usize> {
        match self.direction {
            RadioDirection::Horizontal => {
                if py < 0.0 || py > self.item_h {
                    return None;
                }
                let mut cum_x = 0.0f32;
                for (i, opt) in self.options.iter().enumerate() {
                    let w = opt.len() as f32 * 9.0 + 30.0;
                    if px >= cum_x && px <= cum_x + w {
                        return Some(i);
                    }
                    cum_x += w;
                }
                None
            }
            RadioDirection::Vertical => {
                let idx = (py / self.item_h) as usize;
                if idx < self.options.len() && py >= 0.0 {
                    Some(idx)
                } else {
                    None
                }
            }
        }
    }

    fn render_radio_item(
        &self,
        ctx: &mut RenderContext,
        i: usize,
        opt: &str,
        x: f32,
        cy: f32,
        _seg_w: f32,
    ) {
        let r = 6.0;
        let dot_r = 3.5;
        let selected = i == self.selected;
        let hovered = self.hovered_idx == Some(i);

        let (ring_color, dot_color, text_c) = if self.disabled {
            (
                ctx.tokens().color_border_secondary(),
                ctx.tokens().color_border_secondary(),
                ctx.tokens().color_text_quaternary(),
            )
        } else if selected {
            (
                ctx.tokens().color_primary(),
                ctx.tokens().color_primary(),
                ctx.tokens().color_text(),
            )
        } else if hovered {
            (
                ctx.tokens().color_primary_hover(),
                ctx.tokens().color_primary_hover(),
                ctx.tokens().color_text(),
            )
        } else {
            (
                ctx.tokens().color_border(),
                ctx.tokens().color_border(),
                ctx.tokens().color_text(),
            )
        };

        // 外圈
        let circle_rect = Rect::new(x + 1.0, cy - r, r * 2.0, r * 2.0);
        ctx.stroke_rect(circle_rect, ring_color, 1.5, Some(Radius::uniform(r)));

        // 选中填充点
        if selected {
            ctx.fill_circle(x + r + 1.0, cy, dot_r, dot_color);
        }
        // 使用 em-box 高度（font_size）垂直居中，而非字体度量高度
        let row_rect = Rect::new(x, cy - self.item_h * 0.5, _seg_w, self.item_h);
        let text_y = ctx.visual_center_y(row_rect, 13.0);
        ctx.draw_text(opt, Point::new(x + 20.0, text_y), text_c, 13.0);
    }
}

impl Default for Radio {
    fn default() -> Self {
        Self::new()
    }
}

impl Radio {
    pub fn new() -> Self {
        Self {
            options: Vec::new(),
            selected: 0,
            disabled: false,
            direction: RadioDirection::Horizontal,
            item_h: 24.0,
            hovered_idx: None,
            focused: false,
            on_change: None,
        }
    }
    pub fn options(mut self, opts: Vec<impl Into<String>>) -> Self {
        self.options = opts.into_iter().map(|s| s.into()).collect();
        self
    }
    pub fn selected(mut self, idx: usize) -> Self {
        self.selected = idx;
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn vertical(mut self) -> Self {
        self.direction = RadioDirection::Vertical;
        self
    }
    pub fn on_change<F: FnMut(usize) + 'static>(mut self, f: F) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }
}
