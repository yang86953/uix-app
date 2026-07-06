use crate::core::{Rect, Size};
use crate::define_widget;
use crate::draw::painting::PaintContext;
use crate::draw::{Color, FillRule, PathBuilder, Radius};
use crate::ui::{EventResult, SystemEvent, WidgetTree};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TooltipPlacement {
    Top,
    Bottom,
    Left,
    Right,
}

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
        pending: bool,
        delay_ms: u32,
        timer_id: u32,
        arrow: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
        Size::new(0.0, 0.0)
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match (self.trigger, event) {
            (TriggerMode::Hover, SystemEvent::PointerEnter) => {
                if self.delay_ms == 0 {
                    self.visible = true;
                    self.pending = false;
                } else {
                    self.visible = false;
                    self.pending = true;
                }
                EventResult::Handled
            }
            (TriggerMode::Hover, SystemEvent::PointerLeave) => {
                self.visible = false;
                self.pending = false;
                EventResult::Handled
            }
            (TriggerMode::Click, SystemEvent::PointerDown { .. }) => {
                self.visible = !self.visible;
                self.pending = false;
                EventResult::Handled
            }
            (TriggerMode::Focus, SystemEvent::FocusIn) => {
                if self.delay_ms == 0 {
                    self.visible = true;
                    self.pending = false;
                } else {
                    self.visible = false;
                    self.pending = true;
                }
                EventResult::Handled
            }
            (TriggerMode::Focus, SystemEvent::FocusOut) => {
                self.visible = false;
                self.pending = false;
                EventResult::Handled
            }
            (_, SystemEvent::Timer { id }) if self.pending && *id == self.timer_id => {
                self.visible = true;
                self.pending = false;
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    active_timer => (&self) -> Option<(u64, std::time::Duration)> {
        (self.pending && self.delay_ms > 0).then_some((
            u64::from(self.timer_id),
            std::time::Duration::from_millis(u64::from(self.delay_ms)),
        ))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if !self.visible {
            return;
        }

        let bg = self.bg_color.unwrap_or(Color::from_rgba(50, 50, 50, 230));
        let txt_color = self.text_color.unwrap_or(Color::white());
        let text_w = self.text.len() as f32 * 7.5 + 16.0;
        let text_h = 26.0;
        let arrow_sz = 6.0;
        let gap = if self.arrow { arrow_sz + 2.0 } else { 4.0 };
        let r = Some(Radius::uniform(4.0));

        let (tx, ty) = tooltip_origin(frame, self.placement, text_w, text_h, gap);
        let tip_frame = Rect::new(tx, ty, text_w, text_h);
        ctx.fill_rect(tip_frame, bg, r);

        if self.arrow {
            let (ax, ay, aw, ah) = match self.placement {
                TooltipPlacement::Top => (
                    tx + text_w * 0.5 - arrow_sz,
                    ty + text_h - 1.0,
                    arrow_sz * 2.0,
                    arrow_sz,
                ),
                TooltipPlacement::Bottom => (
                    tx + text_w * 0.5 - arrow_sz,
                    ty - arrow_sz + 1.0,
                    arrow_sz * 2.0,
                    arrow_sz,
                ),
                TooltipPlacement::Left => (
                    tx + text_w - 1.0,
                    ty + text_h * 0.5 - arrow_sz,
                    arrow_sz,
                    arrow_sz * 2.0,
                ),
                TooltipPlacement::Right => (
                    tx - arrow_sz + 1.0,
                    ty + text_h * 0.5 - arrow_sz,
                    arrow_sz,
                    arrow_sz * 2.0,
                ),
            };
            draw_arrow(ctx, ax, ay, aw, ah, self.placement, bg);
        }

        ctx.text_center(&self.text, tip_frame, txt_color, 12.0);
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        let text_w = self.text.len() as f32 * 7.5 + 20.0;
        let text_h = 26.0;
        let gap = if self.arrow { 8.0 } else { 4.0 };
        let (tx, ty) = tooltip_origin(frame, self.placement, text_w, text_h, gap);
        frame.union(&Rect::new(tx, ty, text_w, text_h))
    }

    overlay_entry => (&self, id: crate::ui::WidgetId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        if !self.visible {
            return None;
        }

        let text_w = self.text.len() as f32 * 7.5 + 16.0;
        let text_h = 26.0;
        let arrow_sz = 6.0;
        let gap = if self.arrow { arrow_sz + 2.0 } else { 4.0 };
        let (tx, ty) = tooltip_origin(frame, self.placement, text_w, text_h, gap);
        Some(
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Tooltip)
                .bounds(Rect::new(tx, ty, text_w, text_h))
                .z_index(1100)
                .managed(true),
        )
    }
}

fn tooltip_origin(
    frame: Rect,
    placement: TooltipPlacement,
    text_w: f32,
    text_h: f32,
    gap: f32,
) -> (f32, f32) {
    match placement {
        TooltipPlacement::Top => (
            frame.x + frame.w * 0.5 - text_w * 0.5,
            frame.y - text_h - gap,
        ),
        TooltipPlacement::Bottom => (
            frame.x + frame.w * 0.5 - text_w * 0.5,
            frame.y + frame.h + gap,
        ),
        TooltipPlacement::Left => (
            frame.x - text_w - gap,
            frame.y + frame.h * 0.5 - text_h * 0.5,
        ),
        TooltipPlacement::Right => (
            frame.x + frame.w + gap,
            frame.y + frame.h * 0.5 - text_h * 0.5,
        ),
    }
}

fn draw_arrow(
    ctx: &mut PaintContext,
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
            pending: false,
            delay_ms: 0,
            timer_id: 1,
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

    pub fn delay_ms(mut self, ms: u32) -> Self {
        self.delay_ms = ms;
        self
    }

    pub fn timer_id(mut self, id: u32) -> Self {
        self.timer_id = id;
        self
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }
}
