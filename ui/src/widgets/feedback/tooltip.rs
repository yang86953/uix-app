use crate::define_widget;
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};
use ::uix_graphics::{Color, FillRule, PathBuilder, Radius};
use ::uix_platform::{Rect, Size};

/// Tooltip 弹出位置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TooltipPlacement {
    Top,
    Bottom,
    Left,
    Right,
}

/// Tooltip 触发方式。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TriggerMode {
    Hover,
    Click,
    Focus,
}

define_widget! {
    pub struct Tooltip {
        text: String,
        placement: TooltipPlacement,
        trigger: TriggerMode,
        bg_color: Option<Color>,
        text_color: Option<Color>,
        visible: bool,
        timer: f32,
        arrow: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::traits::GraphicsEngine>) -> Size {
        Size::new(0.0, 0.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match (self.trigger, event) {
            (TriggerMode::Hover, WidgetEvent::HoverEnter) => {
                self.visible = true;
                self.timer = 0.0;
                EventResult::Handled
            }
            (TriggerMode::Hover, WidgetEvent::HoverLeave) => {
                self.visible = false;
                self.timer = 0.0;
                EventResult::Handled
            }
            (TriggerMode::Click, WidgetEvent::MouseDown { .. }) => {
                self.visible = !self.visible;
                EventResult::Handled
            }
            (TriggerMode::Focus, WidgetEvent::FocusIn) => {
                self.visible = true;
                EventResult::Handled
            }
            (TriggerMode::Focus, WidgetEvent::FocusOut) => {
                self.visible = false;
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    on_update => (&mut self, dt: f64) {
        if self.visible && self.trigger == TriggerMode::Hover {
            self.timer += dt as f32;
        }
    }

    needs_continuous_update => (&self) -> bool {
        matches!(self.trigger, TriggerMode::Hover)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if !self.visible { return; }
        if self.trigger == TriggerMode::Hover && self.timer < 0.5 { return; }

        let bg = self.bg_color.unwrap_or(Color::from_rgba(50, 50, 50, 230));
        let txt_color = self.text_color.unwrap_or(Color::white());
        let text_w = self.text.len() as f32 * 7.5 + 16.0;
        let text_h = 26.0;
        let arrow_sz = 6.0;
        let gap = if self.arrow { arrow_sz + 2.0 } else { 4.0 };
        let r = Some(Radius::uniform(4.0));

        let (tx, ty) = match self.placement {
            TooltipPlacement::Top => (frame.x + frame.w * 0.5 - text_w * 0.5, frame.y - text_h - gap),
            TooltipPlacement::Bottom => (frame.x + frame.w * 0.5 - text_w * 0.5, frame.y + frame.h + gap),
            TooltipPlacement::Left => (frame.x - text_w - gap, frame.y + frame.h * 0.5 - text_h * 0.5),
            TooltipPlacement::Right => (frame.x + frame.w + gap, frame.y + frame.h * 0.5 - text_h * 0.5),
        };

        let tip_frame = Rect::new(tx, ty, text_w, text_h);
        ctx.fill_rect(tip_frame, bg, r);

        // 箭头
        if self.arrow {
            let (ax, ay, aw, ah) = match self.placement {
                TooltipPlacement::Top => (tx + text_w * 0.5 - arrow_sz, ty + text_h - 1.0, arrow_sz * 2.0, arrow_sz),
                TooltipPlacement::Bottom => (tx + text_w * 0.5 - arrow_sz, ty - arrow_sz + 1.0, arrow_sz * 2.0, arrow_sz),
                TooltipPlacement::Left => (tx + text_w - 1.0, ty + text_h * 0.5 - arrow_sz, arrow_sz, arrow_sz * 2.0),
                TooltipPlacement::Right => (tx - arrow_sz + 1.0, ty + text_h * 0.5 - arrow_sz, arrow_sz, arrow_sz * 2.0),
            };
            draw_arrow(ctx, ax, ay, aw, ah, self.placement, bg);
        }

        ctx.text_center(&self.text, tip_frame, txt_color, 12.0);
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        let text_w = self.text.len() as f32 * 7.5 + 20.0;
        let text_h = 26.0;
        let gap = if self.arrow { 8.0 } else { 4.0 };
        let (tx, ty) = match self.placement {
            TooltipPlacement::Top => (frame.x + frame.w * 0.5 - text_w * 0.5, frame.y - text_h - gap),
            TooltipPlacement::Bottom => (frame.x + frame.w * 0.5 - text_w * 0.5, frame.y + frame.h + gap),
            TooltipPlacement::Left => (frame.x - text_w - gap, frame.y + frame.h * 0.5 - text_h * 0.5),
            TooltipPlacement::Right => (frame.x + frame.w + gap, frame.y + frame.h * 0.5 - text_h * 0.5),
        };
        let tip = Rect::new(tx, ty, text_w, text_h);
        frame.union(&tip)
    }
}

/// 绘制三角形箭头。
fn draw_arrow(
    ctx: &mut RenderContext,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    dir: TooltipPlacement,
    color: Color,
) {
    let (x1, y1, x2, y2, x3, y3) = match dir {
        TooltipPlacement::Top => (x, y, x + w, y, x + w / 2.0, y + h),
        TooltipPlacement::Bottom => (x, y + h, x + w, y + h, x + w / 2.0, y),
        TooltipPlacement::Left => (x, y, x, y + h, x + w, y + h / 2.0),
        TooltipPlacement::Right => (x + w, y, x + w, y + h, x, y + h / 2.0),
    };
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1);
    pb.line_to(x2, y2);
    pb.line_to(x3, y3);
    pb.close();
    ctx.fill_path(&pb.build(), color, FillRule::NonZero);
}

impl Default for Tooltip {
    fn default() -> Self {
        Self::new("")
    }
}

impl Tooltip {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            placement: TooltipPlacement::Top,
            trigger: TriggerMode::Hover,
            bg_color: None,
            text_color: None,
            visible: false,
            timer: 0.0,
            arrow: true,
        }
    }
    pub fn placement(mut self, p: TooltipPlacement) -> Self {
        self.placement = p;
        self
    }
    pub fn trigger(mut self, t: TriggerMode) -> Self {
        self.trigger = t;
        self
    }
    pub fn bg_color(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }
    pub fn text_color(mut self, c: Color) -> Self {
        self.text_color = Some(c);
        self
    }
    pub fn arrow(mut self, v: bool) -> Self {
        self.arrow = v;
        self
    }
}
