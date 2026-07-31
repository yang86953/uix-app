use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::resources::font::text_backend::estimate_text_metrics;
use crate::draw::{Color, FillRule, PathBuilder, Radius};
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::component::paint_context::PaintContext;
use crate::ui::SnapshotFields;
use crate::ui::{EventResult, KeyCode, MouseButton, SystemEvent, WidgetTree};

const TOOLTIP_FONT_SIZE: f32 = 12.0;
const TOOLTIP_HORIZONTAL_PADDING: f32 = 16.0;
const TOOLTIP_HEIGHT: f32 = 26.0;
const TOOLTIP_ARROW_SIZE: f32 = 6.0;

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
    ContextMenu,
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
        pressed_button: Option<MouseButton>,
        pressed_key: Option<KeyCode>,
        last_frame: std::cell::Cell<Rect>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    hit_test_children => (&self) -> bool { false }

    hit_test_frame => (&self, actual_frame: Rect) -> Rect {
        let frame = Self::normalize_frame(actual_frame);
        self.last_frame.set(frame);
        frame
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        let frame = Self::normalize_frame(frame);
        self.last_frame.set(frame);
        children.iter().map(|child| (child.id, frame)).collect()
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::FocusIn => return EventResult::Handled,
            SystemEvent::FocusOut => {
                self.cancel_pending_activation();
                if self.trigger == TriggerMode::Focus {
                    self.close();
                }
                return EventResult::Handled;
            }
            SystemEvent::WindowBlur => {
                let changed = self.pending
                    || self.pressed_button.is_some()
                    || self.pressed_key.is_some()
                    || self.is_present();
                self.cancel_pending_activation();
                self.close();
                return if changed {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                };
            }
            SystemEvent::KeyDown { key: KeyCode::Escape, .. } if self.is_present() => {
                self.cancel_pending_activation();
                self.close();
                return EventResult::Handled;
            }
            SystemEvent::Timer { id } if self.pending && *id == self.timer_id => {
                self.open();
                return EventResult::Handled;
            }
            SystemEvent::PointerLeave => {
                let had_activation = self.pressed_button.take().is_some();
                if self.trigger == TriggerMode::Hover {
                    self.pending = false;
                    self.close();
                    return EventResult::Handled;
                }
                if had_activation {
                    return EventResult::Handled;
                }
            }
            _ => {}
        }

        match self.trigger {
            TriggerMode::Hover => {
                if let SystemEvent::PointerEnter = event {
                    if self.delay_ms == 0 {
                        self.open();
                    } else {
                        self.close();
                        self.pending = true;
                    }
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            TriggerMode::Focus => EventResult::NotHandled,
            TriggerMode::Click | TriggerMode::ContextMenu => {
                let expected_button = if self.trigger == TriggerMode::ContextMenu {
                    MouseButton::Right
                } else {
                    MouseButton::Left
                };
                match event {
                    SystemEvent::PointerDown { pos, button, .. }
                        if *button == expected_button =>
                    {
                        if self.trigger_rect().contains(*pos) {
                            self.pressed_button = Some(*button);
                            EventResult::Handled
                        } else if self.is_present() {
                            self.close();
                            EventResult::Handled
                        } else {
                            EventResult::NotHandled
                        }
                    }
                    SystemEvent::PointerUp { pos, button, .. }
                        if *button == expected_button =>
                    {
                        let armed = self.pressed_button.take() == Some(*button);
                        if armed && self.trigger_rect().contains(*pos) {
                            self.toggle();
                        }
                        if armed {
                            EventResult::Handled
                        } else {
                            EventResult::NotHandled
                        }
                    }
                    SystemEvent::KeyDown {
                        key: key @ (KeyCode::Enter | KeyCode::Space),
                        ..
                    } => {
                        self.pressed_key = Some(*key);
                        EventResult::Handled
                    }
                    SystemEvent::KeyUp {
                        key: key @ (KeyCode::Enter | KeyCode::Space),
                        ..
                    } => {
                        if self.pressed_key.take() == Some(*key) {
                            self.toggle();
                        }
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
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
        self.last_frame.set(Self::normalize_frame(frame));
        if !self.is_present() {
            return;
        }

        let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
        let bg = fade_color(self.bg_color.unwrap_or(Color::from_rgba(50, 50, 50, 230)), opacity);
        let txt_color = fade_color(self.text_color.unwrap_or(Color::white()), opacity);
        paint_tooltip_bubble(
            ctx,
            &self.text,
            frame,
            self.placement,
            bg,
            txt_color,
            self.arrow,
        );
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        tooltip_dirty_rect(&self.text, self.arrow, self.placement, frame)
    }

    overlay_entry => (&self, id: crate::ui::ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        if !self.is_present() {
            return None;
        }

        Some(
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Tooltip)
                .bounds(tooltip_bubble_rect(
                    &self.text,
                    self.arrow,
                    self.placement,
                    frame,
                ))
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

pub(crate) fn tooltip_dirty_rect(
    text: &str,
    arrow: bool,
    placement: TooltipPlacement,
    frame: Rect,
) -> Rect {
    frame.union(&tooltip_bubble_rect(text, arrow, placement, frame))
}

pub(crate) fn tooltip_bubble_rect(
    text: &str,
    arrow: bool,
    placement: TooltipPlacement,
    frame: Rect,
) -> Rect {
    let bubble = tooltip_bubble_size(text);
    let text_w = bubble.w;
    let text_h = bubble.h;
    let gap = tooltip_gap(arrow);
    let (tx, ty) = tooltip_origin(frame, placement, text_w, text_h, gap);
    Rect::new(tx, ty, text_w, text_h)
}

pub(crate) fn paint_tooltip_bubble(
    ctx: &mut PaintContext,
    text: &str,
    frame: Rect,
    placement: TooltipPlacement,
    bg: Color,
    text_color: Color,
    arrow: bool,
) -> Rect {
    let tip_frame = tooltip_bubble_rect(text, arrow, placement, frame);
    ctx.fill_rect(tip_frame, bg, Some(Radius::uniform(4.0)));

    if arrow {
        let arrow_sz = TOOLTIP_ARROW_SIZE;
        let (ax, ay, aw, ah) = match placement {
            TooltipPlacement::Top => (
                tip_frame.x + tip_frame.w * 0.5 - arrow_sz,
                tip_frame.y + tip_frame.h - 1.0,
                arrow_sz * 2.0,
                arrow_sz,
            ),
            TooltipPlacement::Bottom => (
                tip_frame.x + tip_frame.w * 0.5 - arrow_sz,
                tip_frame.y - arrow_sz + 1.0,
                arrow_sz * 2.0,
                arrow_sz,
            ),
            TooltipPlacement::Left => (
                tip_frame.x + tip_frame.w - 1.0,
                tip_frame.y + tip_frame.h * 0.5 - arrow_sz,
                arrow_sz,
                arrow_sz * 2.0,
            ),
            TooltipPlacement::Right => (
                tip_frame.x - arrow_sz + 1.0,
                tip_frame.y + tip_frame.h * 0.5 - arrow_sz,
                arrow_sz,
                arrow_sz * 2.0,
            ),
        };
        draw_arrow(ctx, ax, ay, aw, ah, placement, bg);
    }

    ctx.text_center(text, tip_frame, text_color, TOOLTIP_FONT_SIZE);
    tip_frame
}

fn tooltip_bubble_size(text: &str) -> Size {
    let text_width = estimate_text_metrics(text, f32::INFINITY, TOOLTIP_FONT_SIZE).max_line_width;
    Size::new(text_width + TOOLTIP_HORIZONTAL_PADDING, TOOLTIP_HEIGHT)
}

fn tooltip_gap(arrow: bool) -> f32 {
    if arrow {
        TOOLTIP_ARROW_SIZE + 2.0
    } else {
        4.0
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
            pressed_button: None,
            pressed_key: None,
            last_frame: std::cell::Cell::new(Rect::new(0.0, 0.0, 80.0, 28.0)),
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

    pub fn bg(self, c: Color) -> Self {
        self.bg_color(c)
    }

    pub fn text_color(mut self, c: Color) -> Self {
        self.text_color = Some(c);
        self
    }

    pub fn color(self, c: Color) -> Self {
        self.text_color(c)
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
        self.cancel_pending_activation();
        self.visible = true;
        self.closing = false;
        self.transition = TransitionPlayer::new(presets::tooltip_enter());
        self.transition_dirty = true;
    }

    pub fn close(&mut self) {
        self.cancel_pending_activation();
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

    fn toggle(&mut self) {
        if self.visible && !self.closing {
            self.close();
        } else {
            self.open();
        }
    }

    fn cancel_pending_activation(&mut self) {
        self.pending = false;
        self.pressed_button = None;
        self.pressed_key = None;
    }

    fn trigger_rect(&self) -> Rect {
        let frame = self.last_frame.get();
        let width = if frame.w > 0.0 { frame.w } else { 80.0 };
        let height = if frame.h > 0.0 { frame.h } else { 28.0 };
        Rect::new(0.0, 0.0, width, height)
    }

    fn normalize_frame(frame: Rect) -> Rect {
        Rect::new(
            if frame.x.is_finite() { frame.x } else { 0.0 },
            if frame.y.is_finite() { frame.y } else { 0.0 },
            if frame.w.is_finite() {
                frame.w.max(0.0)
            } else {
                0.0
            },
            if frame.h.is_finite() {
                frame.h.max(0.0)
            } else {
                0.0
            },
        )
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
        let trigger_changed = self.trigger != next.trigger;
        self.text = next.text;
        self.placement = next.placement;
        self.trigger = next.trigger;
        self.bg_color = next.bg_color;
        self.text_color = next.text_color;
        self.delay_ms = next.delay_ms;
        self.timer_id = next.timer_id;
        self.arrow = next.arrow;
        if trigger_changed {
            self.pending = false;
            self.pressed_button = None;
            self.pressed_key = None;
        }
    }
}
