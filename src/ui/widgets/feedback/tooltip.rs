use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, FillRule, PathBuilder, Radius};
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::SnapshotFields;
use crate::ui::{EventResult, MouseButton, SystemEvent, WidgetTree};

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

component! {
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
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    hit_test_children => (&self) -> bool { false }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        children.iter().map(|child| (child.id, frame)).collect()
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match (self.trigger, event) {
            (TriggerMode::Hover, SystemEvent::PointerEnter) => {
                if self.delay_ms == 0 {
                    self.open();
                    self.pending = false;
                } else {
                    self.close();
                    self.pending = true;
                }
                EventResult::Handled
            }
            (TriggerMode::Hover, SystemEvent::PointerLeave) => {
                self.close();
                self.pending = false;
                EventResult::Handled
            }
            (
                TriggerMode::Click,
                SystemEvent::PointerDown {
                    button: MouseButton::Left,
                    ..
                },
            ) => {
                if self.is_present() {
                    self.close();
                } else {
                    self.open();
                }
                self.pending = false;
                EventResult::Handled
            }
            (_, SystemEvent::Timer { id }) if self.pending && *id == self.timer_id => {
                self.open();
                self.pending = false;
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    on_focus_within => (&mut self, focused: bool) -> EventResult {
        if self.trigger != TriggerMode::Focus {
            return EventResult::NotHandled;
        }
        if focused {
            if self.delay_ms == 0 {
                self.open();
                self.pending = false;
            } else {
                self.close();
                self.pending = true;
            }
        } else {
            self.close();
            self.pending = false;
        }
        EventResult::Handled
    }

    active_timer => (&self) -> Option<(u64, std::time::Duration)> {
        (self.pending && self.delay_ms > 0).then_some((
            u64::from(self.timer_id),
            std::time::Duration::from_millis(u64::from(self.delay_ms)),
        ))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if !self.is_present() {
            return;
        }

        let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
        let bg = fade_color(self.bg_color.unwrap_or(Color::from_rgba(50, 50, 50, 230)), opacity);
        let txt_color = fade_color(self.text_color.unwrap_or(Color::white()), opacity);
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
        tooltip_dirty_rect(&self.text, self.arrow, self.placement, frame)
    }

    overlay_entry => (&self, id: crate::ui::ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        if !self.is_present() {
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
                .z_index(1100),
        )
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !self.is_present() || self.transition.finished {
            self.transition_dirty = false;
            return false;
        }

        self.transition.update(dt);
        self.transition_dirty = true;

        if self.closing && self.transition.finished {
            self.visible = false;
            self.closing = false;
        }

        self.is_present() && !self.transition.finished
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty {
            tooltip_dirty_rect(&self.text, self.arrow, self.placement, frame)
        } else {
            Rect::zero()
        }
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

fn tooltip_dirty_rect(text: &str, arrow: bool, placement: TooltipPlacement, frame: Rect) -> Rect {
    let text_w = text.len() as f32 * 7.5 + 20.0;
    let text_h = 26.0;
    let gap = if arrow { 8.0 } else { 4.0 };
    let (tx, ty) = tooltip_origin(frame, placement, text_w, text_h, gap);
    frame.union(&Rect::new(tx, ty, text_w, text_h))
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

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
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
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
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

    pub fn is_present(&self) -> bool {
        self.visible || self.closing
    }

    pub fn open(&mut self) {
        self.visible = true;
        self.closing = false;
        self.transition = TransitionPlayer::new(presets::tooltip_enter());
        self.transition_dirty = true;
    }

    pub fn close(&mut self) {
        if !self.is_present() {
            self.visible = false;
            self.closing = false;
            self.transition_dirty = false;
            return;
        }
        self.visible = false;
        self.closing = true;
        self.transition = TransitionPlayer::new(presets::tooltip_exit());
        self.transition_dirty = true;
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(80.0, 28.0)
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Tooltip {
            text: self.text.clone(),
            placement: self.placement,
            trigger: self.trigger,
            bg_color: self.bg_color,
            text_color: self.text_color,
            delay_ms: self.delay_ms,
            timer_id: self.timer_id,
            arrow: self.arrow,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.text = next.text;
        self.placement = next.placement;
        self.trigger = next.trigger;
        self.bg_color = next.bg_color;
        self.text_color = next.text_color;
        self.delay_ms = next.delay_ms;
        self.timer_id = next.timer_id;
        self.arrow = next.arrow;
    }
}
