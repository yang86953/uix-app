use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::component;

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::api::PaintContext;
use crate::draw::{Color, FillRule, PathBuilder, Radius};
use crate::ui::animation::{presets, AnimationConfig, TransitionPlayer};
use crate::ui::SnapshotFields;
use crate::ui::{EventResult, KeyCode, MouseButton, State, SystemEvent, WidgetTree};

const POPOVER_WIDTH: f32 = 220.0;
const POPOVER_HEIGHT: f32 = 100.0;
const TRIGGER_WIDTH: f32 = 80.0;
const TRIGGER_HEIGHT: f32 = 28.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PopoverPressTarget {
    Trigger,
}

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
        background: Option<Color>,
        custom_trigger: bool,
        #[snapshot(skip)]
        custom_trigger_view:
            Option<Rc<RefCell<Option<crate::ui::view::ViewNode>>>>,
        #[snapshot(skip)]
        open_binding: Option<State<bool>>,
        timer: f32,
        enter_animation: AnimationConfig,
        leave_animation: AnimationConfig,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
        focused: bool,
        hovered: bool,
        pressed_target: Option<PopoverPressTarget>,
        pressed_key: Option<KeyCode>,
        last_frame: Cell<Rect>,
        popup_rect: Cell<Rect>,
        surface_rect: Cell<Rect>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    hit_test_children => (&self) -> bool { false }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        children.iter().map(|child| (child.id, Self::normalize_frame(frame))).collect()
    }

    build_view_children => (&self) -> Vec<crate::ui::view::ViewNode> {
        self.custom_trigger_view
            .as_ref()
            .and_then(|view| view.borrow_mut().take())
            .into_iter()
            .collect()
    }

    tab_index => (&self) -> i32 { 1 }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_open();
        match event {
            SystemEvent::FocusIn => {
                self.focused = true;
                return EventResult::Handled;
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.cancel_pending_activation();
                return EventResult::Handled;
            }
            SystemEvent::KeyDown { key: key @ (KeyCode::Enter | KeyCode::Space), .. }
                if self.trigger == PopoverTrigger::Click =>
            {
                self.pressed_key = Some(*key);
                return EventResult::Handled;
            }
            SystemEvent::KeyUp { key: key @ (KeyCode::Enter | KeyCode::Space), .. }
                if self.trigger == PopoverTrigger::Click =>
            {
                let matches = self.pressed_key.take() == Some(*key);
                if matches {
                    self.toggle();
                }
                return EventResult::Handled;
            }
            SystemEvent::KeyDown { key: KeyCode::Escape, .. } if self.is_present() => {
                self.cancel_pending_activation();
                self.close();
                return EventResult::Handled;
            }
            SystemEvent::PointerMove { pos, .. } => {
                let hovered = self.trigger_rect().contains(*pos);
                if self.hovered != hovered {
                    self.hovered = hovered;
                    return EventResult::Handled;
                }
            }
            SystemEvent::PointerLeave => {
                let changed = self.hovered || self.pressed_target.is_some();
                self.hovered = false;
                self.pressed_target = None;
                if self.trigger == PopoverTrigger::Hover && self.is_present() {
                    self.close();
                    return EventResult::Handled;
                }
                if changed {
                    return EventResult::Handled;
                }
            }
            _ => {}
        }
        match self.trigger {
            PopoverTrigger::Click => {
                match event {
                    SystemEvent::PointerDown {
                        pos,
                        button: MouseButton::Left,
                        ..
                    } => {
                        if self.trigger_rect().contains(*pos) {
                            self.focused = true;
                            self.pressed_target = Some(PopoverPressTarget::Trigger);
                            return EventResult::Handled;
                        }
                        if self.is_present() && self.popup_rect.get().contains(*pos) {
                            return EventResult::Handled;
                        }
                        if self.is_present() {
                            self.cancel_pending_activation();
                            self.close();
                            return EventResult::Handled;
                        }
                    }
                    SystemEvent::PointerUp {
                        pos,
                        button: MouseButton::Left,
                        ..
                    } => {
                        let pressed = self.pressed_target.take();
                        if pressed == Some(PopoverPressTarget::Trigger)
                            && self.trigger_rect().contains(*pos)
                        {
                            self.toggle();
                        }
                        return EventResult::Handled;
                    }
                    _ => {}
                }
            }
            PopoverTrigger::Hover => {
                if let SystemEvent::PointerEnter = event {
                    self.hovered = true;
                    self.open();
                    self.timer = 0.0;
                    return EventResult::Handled;
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


    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let frame = Self::normalize_frame(frame);
        let surface_size = ctx.logical_surface_size();
        let surface = Self::normalize_frame(Rect::new(0.0, 0.0, surface_size.w, surface_size.h));
        self.last_frame.set(frame);
        self.surface_rect.set(surface);
        let popup_geometry = resolve_popover_geometry(
            frame,
            surface,
            self.placement,
            self.arrow,
            POPOVER_WIDTH,
            POPOVER_HEIGHT,
        );
        self.popup_rect.set(Rect::new(
            popup_geometry.popup.x - frame.x,
            popup_geometry.popup.y - frame.y,
            popup_geometry.popup.w,
            popup_geometry.popup.h,
        ));

        let bg = self
            .background
            .unwrap_or_else(|| ctx.tokens().color_bg_elevated());
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        ctx.push_clip(surface);
        if self.hovered || self.pressed_target.is_some() {
            ctx.fill_rect(
                frame,
                if self.pressed_target.is_some() {
                    ctx.tokens().color_fill_secondary()
                } else {
                    ctx.tokens().color_fill_tertiary()
                },
                r,
            );
        }
        ctx.stroke_rect(
            frame,
            if self.focused && tree.keyboard_focus_visible() {
                ctx.tokens().color_primary()
            } else {
                border
            },
            if self.focused && tree.keyboard_focus_visible() { 2.0 } else { 1.0 },
            r,
        );
        if !self.custom_trigger {
            Self::paint_elided_text(ctx, "Popover", frame, text_secondary, 12.0, true);
        }

        if self.is_present() && popup_geometry.popup.w > 0.0 && popup_geometry.popup.h > 0.0 {
            let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
            let popup_bg = fade_color(bg, opacity);
            let popup_border = fade_color(border, opacity);
            let popup_text = fade_color(text_color, opacity);
            let popup_secondary = fade_color(text_secondary, opacity);
            let pop_rect = self.transitioned_rect(popup_geometry.popup);
            let shadow = ctx.tokens().box_shadow_secondary();
            ctx.draw_box_shadow(
                pop_rect,
                shadow.layer_1.2,
                shadow.layer_1.0,
                shadow.layer_1.1,
                fade_color(shadow.layer_1.3, opacity),
                r,
            );
            ctx.fill_rect(pop_rect, popup_bg, r);
            ctx.stroke_rect(pop_rect, popup_border, 1.0, r);

            if self.arrow {
                draw_popover_arrow(
                    ctx,
                    frame,
                    pop_rect,
                    popup_geometry.placement,
                    popup_bg,
                );
            }

            let inset = 12.0_f32.min(pop_rect.w * 0.5);
            let content_width = (pop_rect.w - inset * 2.0).max(0.0);
            let title_height = 32.0_f32.min(pop_rect.h);
            if !self.title.is_empty() && content_width > 0.0 {
                let title_rect = Rect::new(
                    pop_rect.x + inset,
                    pop_rect.y,
                    content_width,
                    title_height,
                );
                Self::paint_elided_text(ctx, &self.title, title_rect, popup_text, 14.0, false);
                if pop_rect.h > title_height {
                    ctx.fill_rect(
                        Rect::new(
                            pop_rect.x + inset,
                            pop_rect.y + title_height,
                            content_width,
                            1.0,
                        ),
                        popup_border,
                        None,
                    );
                }
            }
            let content_top = if self.title.is_empty() {
                pop_rect.y
            } else {
                (pop_rect.y + title_height + 1.0).min(pop_rect.y + pop_rect.h)
            };
            let content_rect = Rect::new(
                pop_rect.x + inset,
                content_top,
                content_width,
                (pop_rect.y + pop_rect.h - content_top).max(0.0),
            );
            Self::paint_elided_text(
                ctx,
                &self.content,
                content_rect,
                popup_secondary,
                12.0,
                false,
            );
        }
        ctx.pop_clip();
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        self.transition_dirty_rect(frame)
    }

    overlay_entry => (&self, id: crate::ui::ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        if !self.is_present() {
            return None;
        }

        let frame = Self::normalize_frame(frame);
        let popup = expand_popover_rect(self.absolute_popup_rect(frame), 12.0);
        let popup = self
            .transition_sweep_rect(popup)
            .intersect(&self.surface_or_fallback(frame))
            .unwrap_or_default();
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
            background: None,
            custom_trigger: false,
            custom_trigger_view: None,
            open_binding: None,
            timer: 0.0,
            enter_animation: presets::tooltip_enter(),
            leave_animation: presets::tooltip_exit(),
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
            focused: false,
            hovered: false,
            pressed_target: None,
            pressed_key: None,
            last_frame: Cell::new(Rect::zero()),
            popup_rect: Cell::new(Rect::new(
                0.0,
                -POPOVER_HEIGHT - 10.0,
                POPOVER_WIDTH,
                POPOVER_HEIGHT,
            )),
            surface_rect: Cell::new(Rect::zero()),
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

    pub fn bg(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn trigger_view<V: crate::ui::view::View>(mut self, trigger: V) -> Self {
        self.custom_trigger = true;
        self.custom_trigger_view = Some(Rc::new(RefCell::new(Some(crate::ui::view::View::build(
            trigger,
        )))));
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
        self.open_now();
        self.write_bound_open(true);
    }

    /// 将打开状态双向绑定到外部 [`State<bool>`]。
    pub fn controlled_open(mut self, state: &State<bool>) -> Self {
        self.open_binding = Some(state.clone());
        self.apply_bound_open(state.get());
        self
    }

    pub fn set_open(&mut self, open: bool) {
        if open {
            self.open();
        } else {
            self.close();
        }
    }

    fn open_now(&mut self) {
        self.pressed_target = None;
        self.pressed_key = None;
        self.visible = true;
        self.closing = false;
        self.transition = TransitionPlayer::new(self.enter_animation);
        self.transition_dirty = true;
    }

    pub fn close(&mut self) {
        self.close_now();
        self.write_bound_open(false);
    }

    fn close_now(&mut self) {
        self.pressed_target = None;
        self.pressed_key = None;
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
        let geometry_changed = self.placement != next.placement || self.arrow != next.arrow;
        let trigger_changed = self.trigger != next.trigger;
        self.title = next.title;
        self.content = next.content;
        self.placement = next.placement;
        self.trigger = next.trigger;
        self.arrow = next.arrow;
        self.background = next.background;
        self.custom_trigger = next.custom_trigger;
        self.custom_trigger_view = next.custom_trigger_view;
        let controlled_open = next.open_binding.as_ref().map(|_| next.visible);
        self.open_binding = next.open_binding;
        self.enter_animation = next.enter_animation;
        self.leave_animation = next.leave_animation;
        if let Some(open) = controlled_open {
            self.apply_bound_open(open);
        }
        if geometry_changed || trigger_changed {
            self.cancel_pending_activation();
        }
    }

    fn toggle(&mut self) {
        if self.is_present() && !self.closing {
            self.close();
        } else {
            self.open();
        }
    }

    fn sync_bound_open(&mut self) {
        let open = self.open_binding.as_ref().map(State::get);
        if let Some(open) = open {
            self.apply_bound_open(open);
        }
    }

    fn apply_bound_open(&mut self, open: bool) {
        if open {
            if !self.visible || self.closing {
                self.open_now();
            }
        } else if self.visible && !self.closing {
            self.close_now();
        } else if !self.is_present() {
            self.visible = false;
            self.closing = false;
        }
    }

    fn write_bound_open(&self, open: bool) {
        if let Some(state) = self.open_binding.as_ref() {
            if state.get() != open {
                state.set(open);
            }
        }
    }

    fn cancel_pending_activation(&mut self) {
        self.pressed_target = None;
        self.pressed_key = None;
    }

    fn trigger_rect(&self) -> Rect {
        let frame = self.last_frame.get();
        let width = if frame.w > 0.0 {
            frame.w
        } else {
            TRIGGER_WIDTH
        };
        let height = if frame.h > 0.0 {
            frame.h
        } else {
            TRIGGER_HEIGHT
        };
        Rect::new(0.0, 0.0, width, height)
    }

    fn transitioned_rect(&self, rect: Rect) -> Rect {
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
        let frame = Self::normalize_frame(frame);
        let popup = expand_popover_rect(self.absolute_popup_rect(frame), 12.0);
        let bounds = frame.union(&self.transition_sweep_rect(popup));
        bounds
            .intersect(&self.surface_or_fallback(frame))
            .unwrap_or_default()
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(TRIGGER_WIDTH, TRIGGER_HEIGHT)
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Popover {
            title: self.title.clone(),
            content: self.content.clone(),
            placement: self.placement,
            trigger: self.trigger,
            arrow: self.arrow,
            visible: self.visible,
        }
    }

    fn absolute_popup_rect(&self, frame: Rect) -> Rect {
        if self.last_frame.get() == frame && self.popup_rect.get().w >= 0.0 {
            let popup = self.popup_rect.get();
            Rect::new(frame.x + popup.x, frame.y + popup.y, popup.w, popup.h)
        } else {
            resolve_popover_geometry(
                frame,
                self.surface_or_fallback(frame),
                self.placement,
                self.arrow,
                POPOVER_WIDTH,
                POPOVER_HEIGHT,
            )
            .popup
        }
    }

    fn surface_or_fallback(&self, frame: Rect) -> Rect {
        let surface = self.surface_rect.get();
        if surface.w > 0.0 && surface.h > 0.0 {
            surface
        } else {
            frame.union(&Rect::new(
                frame.x - POPOVER_WIDTH * 2.0,
                frame.y - POPOVER_HEIGHT * 2.0,
                POPOVER_WIDTH * 5.0 + frame.w,
                POPOVER_HEIGHT * 5.0 + frame.h,
            ))
        }
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

    fn paint_elided_text(
        ctx: &mut PaintContext<'_>,
        value: &str,
        frame: Rect,
        color: Color,
        font_size: f32,
        centered: bool,
    ) {
        let Some(value) = Self::elide_single_line(ctx, value, font_size, frame.w) else {
            return;
        };
        if frame.h <= 0.0 {
            return;
        }
        ctx.push_clip(frame);
        if centered {
            ctx.text_center(&value, frame, color, font_size);
        } else {
            let y = ctx.visual_center_y(frame, font_size);
            ctx.draw_text(&value, Point::new(frame.x, y), color, font_size);
        }
        ctx.pop_clip();
    }

    fn elide_single_line(
        ctx: &mut PaintContext<'_>,
        value: &str,
        font_size: f32,
        max_width: f32,
    ) -> Option<String> {
        if !max_width.is_finite() || max_width <= 0.0 {
            return None;
        }
        let value = value.replace(['\r', '\n'], " ");
        if Self::text_width(ctx, &value, font_size) <= max_width {
            return Some(value);
        }
        const ELLIPSIS: &str = "…";
        if Self::text_width(ctx, ELLIPSIS, font_size) > max_width {
            return None;
        }
        let mut visible = String::new();
        for ch in value.chars() {
            visible.push(ch);
            visible.push_str(ELLIPSIS);
            let fits = Self::text_width(ctx, &visible, font_size) <= max_width;
            visible.pop();
            if !fits {
                visible.pop();
                break;
            }
        }
        visible.push_str(ELLIPSIS);
        Some(visible)
    }

    fn text_width(ctx: &mut PaintContext<'_>, value: &str, font_size: f32) -> f32 {
        ctx.measure_text(value, font_size).w.max(
            crate::draw::resources::font::text_backend::estimate_text_metrics(
                value,
                f32::INFINITY,
                font_size,
            )
            .max_line_width,
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct PopoverGeometry {
    popup: Rect,
    placement: PopoverPlacement,
}

fn resolve_popover_geometry(
    trigger: Rect,
    surface: Rect,
    placement: PopoverPlacement,
    arrow: bool,
    preferred_width: f32,
    preferred_height: f32,
) -> PopoverGeometry {
    let width = preferred_width.min(surface.w).max(0.0);
    let height = preferred_height.min(surface.h).max(0.0);
    if width <= 0.0 || height <= 0.0 {
        return PopoverGeometry {
            popup: Rect::zero(),
            placement,
        };
    }

    let flipped = flip_popover_placement(placement);
    let authored = rect_for_popover_placement(trigger, placement, arrow, width, height);
    let alternate = rect_for_popover_placement(trigger, flipped, arrow, width, height);
    let (candidate, resolved) =
        if overflow_score(alternate, surface) < overflow_score(authored, surface) {
            (alternate, flipped)
        } else {
            (authored, placement)
        };
    let max_x = surface.x + surface.w - width;
    let max_y = surface.y + surface.h - height;
    PopoverGeometry {
        popup: Rect::new(
            candidate.x.clamp(surface.x, max_x),
            candidate.y.clamp(surface.y, max_y),
            width,
            height,
        ),
        placement: resolved,
    }
}

fn rect_for_popover_placement(
    frame: Rect,
    placement: PopoverPlacement,
    arrow: bool,
    width: f32,
    height: f32,
) -> Rect {
    let (x, y) = popover_position(frame, placement, arrow, width, height);
    Rect::new(x, y, width, height)
}

fn overflow_score(rect: Rect, surface: Rect) -> f32 {
    (surface.x - rect.x).max(0.0)
        + (surface.y - rect.y).max(0.0)
        + (rect.x + rect.w - surface.x - surface.w).max(0.0)
        + (rect.y + rect.h - surface.y - surface.h).max(0.0)
}

fn flip_popover_placement(placement: PopoverPlacement) -> PopoverPlacement {
    match placement {
        PopoverPlacement::Top => PopoverPlacement::Bottom,
        PopoverPlacement::TopLeft => PopoverPlacement::BottomLeft,
        PopoverPlacement::TopRight => PopoverPlacement::BottomRight,
        PopoverPlacement::Bottom => PopoverPlacement::Top,
        PopoverPlacement::BottomLeft => PopoverPlacement::TopLeft,
        PopoverPlacement::BottomRight => PopoverPlacement::TopRight,
        PopoverPlacement::Left => PopoverPlacement::Right,
        PopoverPlacement::LeftTop => PopoverPlacement::RightTop,
        PopoverPlacement::LeftBottom => PopoverPlacement::RightBottom,
        PopoverPlacement::Right => PopoverPlacement::Left,
        PopoverPlacement::RightTop => PopoverPlacement::LeftTop,
        PopoverPlacement::RightBottom => PopoverPlacement::LeftBottom,
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

fn expand_popover_rect(rect: Rect, amount: f32) -> Rect {
    if rect.w <= 0.0 || rect.h <= 0.0 {
        Rect::zero()
    } else {
        Rect::new(
            rect.x - amount,
            rect.y - amount,
            rect.w + amount * 2.0,
            rect.h + amount * 2.0,
        )
    }
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

fn draw_popover_arrow(
    ctx: &mut PaintContext,
    trigger: Rect,
    popup: Rect,
    placement: PopoverPlacement,
    color: Color,
) {
    let arrow_sz = 8.0;
    let (x1, y1, x2, y2, x3, y3) = match placement {
        PopoverPlacement::Top | PopoverPlacement::TopLeft | PopoverPlacement::TopRight => {
            let cx = arrow_anchor(trigger.x + trigger.w * 0.5, popup.x, popup.w, arrow_sz);
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
            let cx = arrow_anchor(trigger.x + trigger.w * 0.5, popup.x, popup.w, arrow_sz);
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
            let cy = arrow_anchor(trigger.y + trigger.h * 0.5, popup.y, popup.h, arrow_sz);
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
            let cy = arrow_anchor(trigger.y + trigger.h * 0.5, popup.y, popup.h, arrow_sz);
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

fn arrow_anchor(desired: f32, start: f32, length: f32, inset: f32) -> f32 {
    if length <= inset * 2.0 {
        start + length * 0.5
    } else {
        desired.clamp(start + inset, start + length - inset)
    }
}
