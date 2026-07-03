use crate::define_widget;

use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};
use uix_graphics::{Color, FillRule, PathBuilder, Radius};
use uix_platform::{Point, Rect, Size};

/// Popover 弹出位置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PopoverPlacement {
    Top,
    TopLeft,
    TopRight,
    Bottom,
    BottomLeft,
    BottomRight,
    Left,
    LeftTop,
    LeftBottom,
    Right,
    RightTop,
    RightBottom,
}

/// 触发方式。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PopoverTrigger {
    Click,
    Hover,
    Focus,
}

define_widget! {
    pub struct Popover {
        title: String,
        content: String,
        visible: bool,
        placement: PopoverPlacement,
        trigger: PopoverTrigger,
        arrow: bool,
        timer: f32,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        Size::new(80.0, 28.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match self.trigger {
            PopoverTrigger::Click => {
                if let WidgetEvent::MouseDown { pos, .. } = event {
                    if pos.x >= 0.0 && pos.x <= 80.0 && pos.y >= 0.0 && pos.y <= 28.0 {
                        self.visible = !self.visible;
                        return EventResult::Handled;
                    }
                    let popup = self.popup_rect(80.0, 28.0);
                    if self.visible && !popup.contains(*pos) && !(pos.x >= 0.0 && pos.x <= 80.0 && pos.y >= 0.0 && pos.y <= 28.0) {
                        self.visible = false;
                    }
                }
            }
            PopoverTrigger::Hover => {
                match event {
                    WidgetEvent::HoverEnter => { self.visible = true; self.timer = 0.0; return EventResult::Handled; }
                    WidgetEvent::HoverLeave => { self.visible = false; return EventResult::Handled; }
                    _ => {}
                }
            }
            PopoverTrigger::Focus => {
                match event {
                    WidgetEvent::FocusIn => { self.visible = true; return EventResult::Handled; }
                    WidgetEvent::FocusOut => { self.visible = false; return EventResult::Handled; }
                    _ => {}
                }
            }
        }
        EventResult::NotHandled
    }

    on_update => (&mut self, dt: f64) {
        self.timer += dt as f32;
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        ctx.stroke_rect(frame, border, 1.0, r);
        ctx.text_center("Popover", frame, text_secondary, 12.0);

        if self.visible {
            let (pw, ph) = (220.0, 100.0);
            let (px, py) = self.popup_position(frame, pw, ph);
            let pop_rect = Rect::new(px, py, pw, ph);
            ctx.fill_rect(pop_rect, bg, r);
            ctx.stroke_rect(pop_rect, border, 1.0, r);

            if self.arrow {
                draw_popover_arrow(ctx, frame, pop_rect, self.placement, bg);
            }

            if !self.title.is_empty() {
                let title_rect = Rect::new(px, py, pw, 32.0);
                let title_y = ctx.visual_center_y(title_rect, 14.0);
                ctx.draw_text(&self.title, Point::new(px + 12.0, title_y), text_color, 14.0);
                ctx.fill_rect(Rect::new(px + 12.0, py + 32.0, pw - 24.0, 1.0), border, None);
            }
            let content_y = py + if self.title.is_empty() { 12.0 } else { 40.0 };
            ctx.draw_text(&self.content, Point::new(px + 12.0, content_y), text_secondary, 12.0);
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        let (pw, ph) = (220.0, 100.0);
        let (px, py) = self.popup_position(frame, pw, ph);
        let pop = Rect::new(px, py, pw, ph);
        frame.union(&pop)
    }
}

impl Default for Popover {
    fn default() -> Self {
        Self::new("")
    }
}

impl Popover {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            title: String::new(),
            content: content.into(),
            visible: false,
            placement: PopoverPlacement::Top,
            trigger: PopoverTrigger::Click,
            arrow: true,
            timer: 0.0,
        }
    }
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = t.into();
        self
    }
    pub fn placement(mut self, p: PopoverPlacement) -> Self {
        self.placement = p;
        self
    }
    pub fn trigger(mut self, t: PopoverTrigger) -> Self {
        self.trigger = t;
        self
    }
    pub fn arrow(mut self, v: bool) -> Self {
        self.arrow = v;
        self
    }

    fn popup_rect(&self, _fw: f32, _fh: f32) -> Rect {
        let (pw, ph) = (220.0, 100.0);
        let (px, py) = (0.0, -ph - 8.0);
        Rect::new(px, py, pw, ph)
    }

    fn popup_position(&self, frame: Rect, pw: f32, ph: f32) -> (f32, f32) {
        let gap = if self.arrow { 10.0 } else { 4.0 };
        match self.placement {
            PopoverPlacement::Top => (frame.x, frame.y - ph - gap),
            PopoverPlacement::TopLeft => (frame.x, frame.y - ph - gap),
            PopoverPlacement::TopRight => (frame.x + frame.w - pw, frame.y - ph - gap),
            PopoverPlacement::Bottom => (frame.x, frame.y + frame.h + gap),
            PopoverPlacement::BottomLeft => (frame.x, frame.y + frame.h + gap),
            PopoverPlacement::BottomRight => (frame.x + frame.w - pw, frame.y + frame.h + gap),
            PopoverPlacement::Left => (frame.x - pw - gap, frame.y + frame.h * 0.5 - ph * 0.5),
            PopoverPlacement::LeftTop => (frame.x - pw - gap, frame.y),
            PopoverPlacement::LeftBottom => (frame.x - pw - gap, frame.y + frame.h - ph),
            PopoverPlacement::Right => {
                (frame.x + frame.w + gap, frame.y + frame.h * 0.5 - ph * 0.5)
            }
            PopoverPlacement::RightTop => (frame.x + frame.w + gap, frame.y),
            PopoverPlacement::RightBottom => (frame.x + frame.w + gap, frame.y + frame.h - ph),
        }
    }
}

fn draw_popover_arrow(
    ctx: &mut RenderContext,
    _trigger: Rect,
    popup: Rect,
    placement: PopoverPlacement,
    color: Color,
) {
    let arrow_sz = 8.0;
    let (x1, y1, x2, y2, x3, y3) = match placement {
        PopoverPlacement::Top | PopoverPlacement::TopLeft | PopoverPlacement::TopRight => {
            let cx = popup.x + popup.w / 2.0;
            (
                cx - arrow_sz,
                popup.y + popup.h,
                cx + arrow_sz,
                popup.y + popup.h,
                cx,
                popup.y + popup.h + arrow_sz,
            )
        }
        PopoverPlacement::Bottom | PopoverPlacement::BottomLeft | PopoverPlacement::BottomRight => {
            let cx = popup.x + popup.w / 2.0;
            (
                cx - arrow_sz,
                popup.y,
                cx + arrow_sz,
                popup.y,
                cx,
                popup.y - arrow_sz,
            )
        }
        PopoverPlacement::Left | PopoverPlacement::LeftTop | PopoverPlacement::LeftBottom => {
            let cy = popup.y + popup.h / 2.0;
            (
                popup.x + popup.w,
                cy - arrow_sz,
                popup.x + popup.w,
                cy + arrow_sz,
                popup.x + popup.w + arrow_sz,
                cy,
            )
        }
        PopoverPlacement::Right | PopoverPlacement::RightTop | PopoverPlacement::RightBottom => {
            let cy = popup.y + popup.h / 2.0;
            (
                popup.x,
                cy - arrow_sz,
                popup.x,
                cy + arrow_sz,
                popup.x - arrow_sz,
                cy,
            )
        }
    };
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1);
    pb.line_to(x2, y2);
    pb.line_to(x3, y3);
    pb.close();
    ctx.fill_path(&pb.build(), color, FillRule::NonZero);
}
