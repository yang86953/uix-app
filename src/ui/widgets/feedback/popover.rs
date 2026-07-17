use crate::component;

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, FillRule, PathBuilder, Radius};
use crate::ui::animation::{presets, AnimationConfig, TransitionPlayer};
use crate::ui::SnapshotFields;
use crate::ui::{EventResult, KeyCode, MouseButton, SystemEvent, WidgetTree};

/// Popover placement.
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

/// Popover trigger mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PopoverTrigger {
    Click,
    Hover,
    Focus,
}

component! {
    pub struct Popover {
        title: String,
        content: String,
        visible: bool,
        placement: PopoverPlacement,
        trigger: PopoverTrigger,
        arrow: bool,
        timer: f32,
        enter_animation: AnimationConfig,
        leave_animation: AnimationConfig,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
        focused: bool,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    hit_test_children => (&self) -> bool { false }

    tab_index => (&self) -> i32 { 1 }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::FocusIn => {
                self.focused = true;
                return EventResult::Handled;
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                return EventResult::Handled;
            }
            SystemEvent::KeyDown { key: KeyCode::Enter | KeyCode::Space, .. }
                if self.trigger == PopoverTrigger::Click =>
            {
                if self.visible {
                    self.close();
                } else {
                    self.open();
                }
                return EventResult::Handled;
            }
            SystemEvent::KeyDown { key: KeyCode::Escape, .. } if self.is_present() => {
                self.close();
                return EventResult::Handled;
            }
            _ => {}
        }
        match self.trigger {
            PopoverTrigger::Click => {
                if let SystemEvent::PointerDown {
                    pos,
                    button: MouseButton::Left,
                    ..
                } = event
                {
                    if pos.x >= 0.0 && pos.x <= 80.0 && pos.y >= 0.0 && pos.y <= 28.0 {
                        if self.visible {
                            self.close();
                        } else {
                            self.open();
                        }
                        return EventResult::Handled;
                    }
                    let popup = self.popup_rect(80.0, 28.0);
                    if self.is_present() && !popup.contains(*pos) && !(pos.x >= 0.0 && pos.x <= 80.0 && pos.y >= 0.0 && pos.y <= 28.0) {
                        self.close();
                    }
                }
            }
            PopoverTrigger::Hover => {
                match event {
                    SystemEvent::PointerEnter => { self.open(); self.timer = 0.0; return EventResult::Handled; }
                    SystemEvent::PointerLeave => { self.close(); return EventResult::Handled; }
                    _ => {}
                }
            }
            PopoverTrigger::Focus => {}
        }
        EventResult::NotHandled
    }

    on_focus_within => (&mut self, focused: bool) -> EventResult {
        if self.trigger != PopoverTrigger::Focus {
            return EventResult::NotHandled;
        }
        if focused {
            self.open();
        } else {
            self.close();
        }
        EventResult::Handled
    }


    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        ctx.stroke_rect(
            frame,
            if self.focused {
                ctx.tokens().color_primary()
            } else {
                border
            },
            if self.focused { 2.0 } else { 1.0 },
            r,
        );
        ctx.text_center("Popover", frame, text_secondary, 12.0);

        if self.is_present() {
            let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
            let popup_bg = fade_color(bg, opacity);
            let popup_border = fade_color(border, opacity);
            let popup_text = fade_color(text_color, opacity);
            let popup_secondary = fade_color(text_secondary, opacity);
            let (pw, ph) = (220.0, 100.0);
            let pop_rect = self.transitioned_popup_rect(frame, pw, ph);
            let (px, py) = (pop_rect.x, pop_rect.y);
            ctx.fill_rect(pop_rect, popup_bg, r);
            ctx.stroke_rect(pop_rect, popup_border, 1.0, r);

            if self.arrow {
                draw_popover_arrow(ctx, frame, pop_rect, self.placement, popup_bg);
            }

            if !self.title.is_empty() {
                let title_rect = Rect::new(px, py, pw, 32.0);
                let title_y = ctx.visual_center_y(title_rect, 14.0);
                ctx.draw_text(&self.title, Point::new(px + 12.0, title_y), popup_text, 14.0);
                ctx.fill_rect(Rect::new(px + 12.0, py + 32.0, pw - 24.0, 1.0), popup_border, None);
            }
            let content_y = py + if self.title.is_empty() { 12.0 } else { 40.0 };
            ctx.draw_text(&self.content, Point::new(px + 12.0, content_y), popup_secondary, 12.0);
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        self.transition_dirty_rect(frame)
    }

    overlay_entry => (&self, id: crate::ui::ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        if !self.is_present() {
            return None;
        }

        let (pw, ph) = (220.0, 100.0);
        let popup = self.transition_sweep_rect(self.popup_rect_for_frame(frame, pw, ph));
        Some(
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                .bounds(popup)
                .z_index(900),
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
            self.transition_dirty_rect(frame)
        } else {
            Rect::zero()
        }
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
            enter_animation: presets::tooltip_enter(),
            leave_animation: presets::tooltip_exit(),
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
            focused: false,
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

    /// 设置打开时播放的动画；已打开时从当前声明重新开始进场。
    pub fn enter_animation(mut self, animation: AnimationConfig) -> Self {
        self.enter_animation = animation;
        if self.visible && !self.closing {
            self.transition = TransitionPlayer::new(animation);
            self.transition_dirty = true;
        }
        self
    }

    /// 设置关闭时播放的动画。
    pub fn leave_animation(mut self, animation: AnimationConfig) -> Self {
        self.leave_animation = animation;
        if self.closing {
            self.transition = TransitionPlayer::new(animation);
            self.transition_dirty = true;
        }
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
        self.transition = TransitionPlayer::new(self.enter_animation);
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
        self.transition = TransitionPlayer::new(self.leave_animation);
        self.transition_dirty = true;
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.title = next.title;
        self.content = next.content;
        self.placement = next.placement;
        self.trigger = next.trigger;
        self.arrow = next.arrow;
        self.enter_animation = next.enter_animation;
        self.leave_animation = next.leave_animation;
    }

    fn popup_rect(&self, _fw: f32, _fh: f32) -> Rect {
        let (pw, ph) = (220.0, 100.0);
        let (px, py) = popover_position(
            Rect::new(0.0, 0.0, 80.0, 28.0),
            self.placement,
            self.arrow,
            pw,
            ph,
        );
        Rect::new(px, py, pw, ph)
    }

    fn popup_position(&self, frame: Rect, pw: f32, ph: f32) -> (f32, f32) {
        popover_position(frame, self.placement, self.arrow, pw, ph)
    }

    fn popup_rect_for_frame(&self, frame: Rect, pw: f32, ph: f32) -> Rect {
        let (x, y) = self.popup_position(frame, pw, ph);
        Rect::new(x, y, pw, ph)
    }

    fn transitioned_popup_rect(&self, frame: Rect, pw: f32, ph: f32) -> Rect {
        let rect = self.popup_rect_for_frame(frame, pw, ph);
        let scale = self.transition.scale.max(0.0);
        let width = rect.w * scale;
        let height = rect.h * scale;
        Rect::new(
            rect.x + (rect.w - width) * 0.5 + self.transition.offset.x,
            rect.y + (rect.h - height) * 0.5 + self.transition.offset.y,
            width,
            height,
        )
    }

    fn transition_sweep_rect(&self, rect: Rect) -> Rect {
        let (from, to) = self.transition.offset_endpoints();
        rect.union(&translated_rect(rect, from))
            .union(&translated_rect(rect, to))
    }

    fn transition_dirty_rect(&self, frame: Rect) -> Rect {
        let popup = self.popup_rect_for_frame(frame, 220.0, 100.0);
        frame.union(&self.transition_sweep_rect(popup))
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(80.0, 28.0)
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Popover {
            title: self.title.clone(),
            content: self.content.clone(),
            placement: self.placement,
            trigger: self.trigger,
            arrow: self.arrow,
        }
    }
}

fn popover_position(
    frame: Rect,
    placement: PopoverPlacement,
    arrow: bool,
    pw: f32,
    ph: f32,
) -> (f32, f32) {
    let gap = if arrow { 10.0 } else { 4.0 };
    match placement {
        PopoverPlacement::Top | PopoverPlacement::TopLeft => (frame.x, frame.y - ph - gap),
        PopoverPlacement::TopRight => (frame.x + frame.w - pw, frame.y - ph - gap),
        PopoverPlacement::Bottom | PopoverPlacement::BottomLeft => {
            (frame.x, frame.y + frame.h + gap)
        }
        PopoverPlacement::BottomRight => (frame.x + frame.w - pw, frame.y + frame.h + gap),
        PopoverPlacement::Left => (frame.x - pw - gap, frame.y + frame.h * 0.5 - ph * 0.5),
        PopoverPlacement::LeftTop => (frame.x - pw - gap, frame.y),
        PopoverPlacement::LeftBottom => (frame.x - pw - gap, frame.y + frame.h - ph),
        PopoverPlacement::Right => (frame.x + frame.w + gap, frame.y + frame.h * 0.5 - ph * 0.5),
        PopoverPlacement::RightTop => (frame.x + frame.w + gap, frame.y),
        PopoverPlacement::RightBottom => (frame.x + frame.w + gap, frame.y + frame.h - ph),
    }
}

fn translated_rect(rect: Rect, offset: Point) -> Rect {
    Rect::new(rect.x + offset.x, rect.y + offset.y, rect.w, rect.h)
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

fn draw_popover_arrow(
    ctx: &mut PaintContext,
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
