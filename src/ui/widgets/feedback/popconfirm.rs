use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, FillRule, PathBuilder, Radius};
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::component_snapshot::SnapshotPopconfirm;
use crate::ui::SnapshotFields;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, WidgetTree,
};

const POPCONFIRM_WIDTH: f32 = 200.0;
const POPCONFIRM_HEIGHT: f32 = 110.0;
const TRIGGER_WIDTH: f32 = 80.0;
const TRIGGER_HEIGHT: f32 = 28.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PopconfirmTarget {
    Trigger,
    Confirm,
    Cancel,
}

/// Popconfirm 弹出位置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PopconfirmPlacement {
    Top,
    TopLeft,
    TopRight,
    Bottom,
    BottomLeft,
    BottomRight,
}

component! {
    pub struct Popconfirm {
        title: String,
        confirm_text: String,
        cancel_text: String,
        visible: bool,
        placement: PopconfirmPlacement,
        arrow: bool,
        icon: bool,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
        focused: bool,
        focused_action: usize,
        pending_submit: Cell<bool>,
        hovered_target: Option<PopconfirmTarget>,
        pressed_target: Option<PopconfirmTarget>,
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

    tab_index => (&self) -> i32 { 1 }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(target) = self.target_at(*pos) {
                    self.focused = true;
                    self.pressed_target = Some(target);
                    self.focused_action = match target {
                        PopconfirmTarget::Cancel => 1,
                        _ => 0,
                    };
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
                EventResult::NotHandled
            }
            SystemEvent::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let pressed = self.pressed_target.take();
                if let Some(pressed) = pressed.filter(|pressed| Some(*pressed) == self.target_at(*pos)) {
                    match pressed {
                        PopconfirmTarget::Trigger => {
                            if self.is_present() && !self.closing {
                                self.close();
                            } else {
                                self.open();
                            }
                        }
                        PopconfirmTarget::Confirm => self.confirm(),
                        PopconfirmTarget::Cancel => self.close(),
                    }
                }
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                let hovered = self.target_at(*pos);
                if self.hovered_target != hovered {
                    self.hovered_target = hovered;
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave => {
                let changed = self.hovered_target.is_some() || self.pressed_target.is_some();
                self.hovered_target = None;
                self.pressed_target = None;
                if changed {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.cancel_pending_activation();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } if self.visible => match key {
                KeyCode::Left | KeyCode::Home => {
                    self.focused_action = 0;
                    EventResult::Handled
                }
                KeyCode::Right | KeyCode::End => {
                    self.focused_action = 1;
                    EventResult::Handled
                }
                KeyCode::Enter | KeyCode::Space => {
                    self.pressed_key = Some(*key);
                    EventResult::Handled
                }
                KeyCode::Escape => {
                    self.close();
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
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
                let matches = self.pressed_key.take() == Some(*key);
                if matches {
                    if self.visible {
                        if self.focused_action == 0 {
                            self.confirm();
                        } else {
                            self.close();
                        }
                    } else {
                        self.open();
                    }
                }
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    on_focus_within => (&mut self, focused: bool) -> EventResult {
        self.focused = focused;
        if !focused {
            self.cancel_pending_activation();
        }
        if !focused && self.visible {
            self.close();
        }
        EventResult::Handled
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_submit
            .replace(false)
            .then(|| SemanticEvent::submit(id, "confirm"))
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        self.dirty_rect_for_frame(frame)
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let frame = Self::normalize_frame(frame);
        let surface_size = ctx.logical_surface_size();
        let surface = Self::normalize_frame(Rect::new(0.0, 0.0, surface_size.w, surface_size.h));
        self.last_frame.set(frame);
        self.surface_rect.set(surface);
        let popup_geometry = resolve_popconfirm_geometry(
            frame,
            surface,
            self.placement,
            self.arrow,
            POPCONFIRM_WIDTH,
            POPCONFIRM_HEIGHT,
        );
        self.popup_rect.set(Rect::new(
            popup_geometry.popup.x - frame.x,
            popup_geometry.popup.y - frame.y,
            popup_geometry.popup.w,
            popup_geometry.popup.h,
        ));

        let loc = crate::ui::locale::use_locale();
        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let primary = ctx.tokens().color_primary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        ctx.push_clip(surface);
        if self.hovered_target == Some(PopconfirmTarget::Trigger)
            || self.pressed_target == Some(PopconfirmTarget::Trigger)
        {
            ctx.fill_rect(
                frame,
                if self.pressed_target == Some(PopconfirmTarget::Trigger) {
                    ctx.tokens().color_fill_secondary()
                } else {
                    ctx.tokens().color_fill_tertiary()
                },
                r,
            );
        }
        Self::paint_elided_text(
            ctx,
            loc.delete_text,
            frame,
            ctx.tokens().color_error(),
            13.0,
            true,
        );
        if self.focused {
            ctx.stroke_rect(frame, primary, 2.0, r);
        }

        if self.is_present() && popup_geometry.popup.w > 0.0 && popup_geometry.popup.h > 0.0 {
            let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
            let popup_bg = fade_color(bg, opacity);
            let popup_border = fade_color(border, opacity);
            let popup_text = fade_color(text_color, opacity);
            let popup_primary = fade_color(primary, opacity);
            let popup_warning = fade_color(ctx.tokens().color_warning(), opacity);
            let pop_rect = popup_geometry.popup;
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
                draw_popconfirm_arrow(
                    ctx,
                    frame,
                    pop_rect,
                    popup_geometry.placement,
                    popup_bg,
                );
            }

            let loc = crate::ui::locale::use_locale();
            let title = if self.title.is_empty() {
                loc.popconfirm_title
            } else {
                &self.title
            };
            let inset = 12.0_f32.min(pop_rect.w * 0.5);
            let (confirm_rect, cancel_rect) = button_rects_for_popup(pop_rect);
            let title_bottom = if confirm_rect.h > 0.0 {
                (confirm_rect.y - 6.0).max(pop_rect.y)
            } else {
                pop_rect.y + pop_rect.h
            };
            let icon_width = if self.icon && pop_rect.w >= 48.0 {
                20.0
            } else {
                0.0
            };
            if icon_width > 0.0 {
                crate::ui::widgets::icon::paint_icon_in_frame(
                    ctx,
                    "alert-triangle",
                    Rect::new(
                        pop_rect.x + inset,
                        pop_rect.y + 8.0,
                        icon_width,
                        (title_bottom - pop_rect.y - 8.0).max(0.0),
                    ),
                    popup_warning,
                    16.0,
                );
            }
            let title_rect = Rect::new(
                pop_rect.x + inset + icon_width,
                pop_rect.y + 6.0,
                (pop_rect.w - inset * 2.0 - icon_width).max(0.0),
                (title_bottom - pop_rect.y - 6.0).max(0.0),
            );
            Self::paint_elided_text(ctx, title, title_rect, popup_text, 13.0, false);

            let btn_r = Some(Radius::uniform(4.0));
            if self.hovered_target == Some(PopconfirmTarget::Confirm)
                || self.pressed_target == Some(PopconfirmTarget::Confirm)
            {
                ctx.fill_rect(
                    confirm_rect,
                    if self.pressed_target == Some(PopconfirmTarget::Confirm) {
                        fade_color(ctx.tokens().color_primary_active(), opacity)
                    } else {
                        fade_color(ctx.tokens().color_primary_hover(), opacity)
                    },
                    btn_r,
                );
            } else {
                ctx.fill_rect(confirm_rect, popup_primary, btn_r);
            }
            let confirm = if self.confirm_text.is_empty() {
                loc.popconfirm_ok
            } else {
                &self.confirm_text
            };
            Self::paint_elided_text(
                ctx,
                confirm,
                confirm_rect,
                fade_color(Color::white(), opacity),
                12.0,
                true,
            );

            if self.hovered_target == Some(PopconfirmTarget::Cancel)
                || self.pressed_target == Some(PopconfirmTarget::Cancel)
            {
                ctx.fill_rect(
                    cancel_rect,
                    if self.pressed_target == Some(PopconfirmTarget::Cancel) {
                        fade_color(ctx.tokens().color_fill_secondary(), opacity)
                    } else {
                        fade_color(ctx.tokens().color_fill_tertiary(), opacity)
                    },
                    btn_r,
                );
            }
            ctx.stroke_rect(cancel_rect, popup_border, 1.0, btn_r);
            let cancel = if self.cancel_text.is_empty() {
                loc.popconfirm_cancel
            } else {
                &self.cancel_text
            };
            Self::paint_elided_text(ctx, cancel, cancel_rect, popup_text, 12.0, true);
            if self.focused && self.visible {
                let (focus_rect, focus_color) = if self.focused_action == 0 {
                    (confirm_rect, fade_color(Color::white(), opacity))
                } else {
                    (cancel_rect, popup_primary)
                };
                ctx.stroke_rect(focus_rect, focus_color, 2.0, btn_r);
            }
        }
        ctx.pop_clip();
    }

    overlay_entry => (&self, id: crate::ui::ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        if !self.is_present() {
            return None;
        }

        let frame = Self::normalize_frame(frame);
        let popup = expand_popconfirm_rect(self.absolute_popup_rect(frame), 12.0)
            .intersect(&self.surface_or_fallback(frame))
            .unwrap_or(Rect::zero());
        Some(
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                .bounds(popup)
                .z_index(950),
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
            self.dirty_rect_for_frame(frame)
        } else {
            Rect::zero()
        }
    }
}

impl Default for Popconfirm {
    fn default() -> Self {
        Self::new()
    }
}

impl Popconfirm {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            confirm_text: "OK".to_string(),
            cancel_text: "Cancel".to_string(),
            visible: false,
            placement: PopconfirmPlacement::Top,
            arrow: true,
            icon: true,
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
            focused: false,
            focused_action: 0,
            pending_submit: Cell::new(false),
            hovered_target: None,
            pressed_target: None,
            pressed_key: None,
            last_frame: Cell::new(Rect::zero()),
            popup_rect: Cell::new(Rect::new(
                0.0,
                -POPCONFIRM_HEIGHT - 10.0,
                POPCONFIRM_WIDTH,
                POPCONFIRM_HEIGHT,
            )),
            surface_rect: Cell::new(Rect::zero()),
        }
    }
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = t.into();
        self
    }
    pub fn confirm_text(mut self, t: impl Into<String>) -> Self {
        self.confirm_text = t.into();
        self
    }
    pub fn cancel_text(mut self, t: impl Into<String>) -> Self {
        self.cancel_text = t.into();
        self
    }
    pub fn placement(mut self, p: PopconfirmPlacement) -> Self {
        self.placement = p;
        self
    }
    pub fn arrow(mut self, v: bool) -> Self {
        self.arrow = v;
        self
    }
    pub fn icon(mut self, v: bool) -> Self {
        self.icon = v;
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
        self.pending_submit.set(false);
        self.focused_action = 0;
        self.visible = true;
        self.closing = false;
        self.transition = TransitionPlayer::new(presets::tooltip_enter());
        self.transition_dirty = true;
    }

    pub(crate) fn focused_action(&self) -> Option<usize> {
        self.visible.then_some(self.focused_action)
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

    pub(crate) fn sync_from(&mut self, next: Self) {
        let geometry_changed = self.placement != next.placement || self.arrow != next.arrow;
        self.title = next.title;
        self.confirm_text = next.confirm_text;
        self.cancel_text = next.cancel_text;
        self.placement = next.placement;
        self.arrow = next.arrow;
        self.icon = next.icon;
        if geometry_changed {
            self.cancel_pending_activation();
        }
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

    fn button_rects(&self) -> (Rect, Rect) {
        button_rects_for_popup(self.popup_rect.get())
    }

    fn target_at(&self, pos: Point) -> Option<PopconfirmTarget> {
        if self.trigger_rect().contains(pos) {
            return Some(PopconfirmTarget::Trigger);
        }
        if !self.visible {
            return None;
        }
        let (confirm, cancel) = self.button_rects();
        if confirm.contains(pos) {
            Some(PopconfirmTarget::Confirm)
        } else if cancel.contains(pos) {
            Some(PopconfirmTarget::Cancel)
        } else {
            None
        }
    }

    fn cancel_pending_activation(&mut self) {
        self.pressed_target = None;
        self.pressed_key = None;
    }

    fn confirm(&mut self) {
        self.pending_submit.set(true);
        self.close();
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(TRIGGER_WIDTH, TRIGGER_HEIGHT)
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Popconfirm(SnapshotPopconfirm {
            title: self.title.clone(),
            confirm_text: self.confirm_text.clone(),
            cancel_text: self.cancel_text.clone(),
            placement: self.placement,
            arrow: self.arrow,
            icon: self.icon,
            visible: self.visible,
            focused_action: self.focused_action(),
        })
    }

    fn absolute_popup_rect(&self, frame: Rect) -> Rect {
        if self.last_frame.get() == frame && self.popup_rect.get().w >= 0.0 {
            let popup = self.popup_rect.get();
            Rect::new(frame.x + popup.x, frame.y + popup.y, popup.w, popup.h)
        } else {
            resolve_popconfirm_geometry(
                frame,
                self.surface_or_fallback(frame),
                self.placement,
                self.arrow,
                POPCONFIRM_WIDTH,
                POPCONFIRM_HEIGHT,
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
                frame.x - POPCONFIRM_WIDTH * 2.0,
                frame.y - POPCONFIRM_HEIGHT * 2.0,
                POPCONFIRM_WIDTH * 5.0 + frame.w,
                POPCONFIRM_HEIGHT * 5.0 + frame.h,
            ))
        }
    }

    fn dirty_rect_for_frame(&self, frame: Rect) -> Rect {
        let frame = Self::normalize_frame(frame);
        frame
            .union(&expand_popconfirm_rect(
                self.absolute_popup_rect(frame),
                12.0,
            ))
            .intersect(&self.surface_or_fallback(frame))
            .unwrap_or(Rect::zero())
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
            crate::draw::font::text_backend::estimate_text_metrics(value, f32::INFINITY, font_size)
                .max_line_width,
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct PopconfirmGeometry {
    popup: Rect,
    placement: PopconfirmPlacement,
}

fn resolve_popconfirm_geometry(
    trigger: Rect,
    surface: Rect,
    placement: PopconfirmPlacement,
    arrow: bool,
    preferred_width: f32,
    preferred_height: f32,
) -> PopconfirmGeometry {
    let width = preferred_width.min(surface.w).max(0.0);
    let height = preferred_height.min(surface.h).max(0.0);
    if width <= 0.0 || height <= 0.0 {
        return PopconfirmGeometry {
            popup: Rect::zero(),
            placement,
        };
    }
    let flipped = match placement {
        PopconfirmPlacement::Top => PopconfirmPlacement::Bottom,
        PopconfirmPlacement::TopLeft => PopconfirmPlacement::BottomLeft,
        PopconfirmPlacement::TopRight => PopconfirmPlacement::BottomRight,
        PopconfirmPlacement::Bottom => PopconfirmPlacement::Top,
        PopconfirmPlacement::BottomLeft => PopconfirmPlacement::TopLeft,
        PopconfirmPlacement::BottomRight => PopconfirmPlacement::TopRight,
    };
    let authored = rect_for_popconfirm_placement(trigger, placement, arrow, width, height);
    let alternate = rect_for_popconfirm_placement(trigger, flipped, arrow, width, height);
    let (candidate, resolved) = if popconfirm_overflow_score(alternate, surface)
        < popconfirm_overflow_score(authored, surface)
    {
        (alternate, flipped)
    } else {
        (authored, placement)
    };
    let max_x = surface.x + surface.w - width;
    let max_y = surface.y + surface.h - height;
    PopconfirmGeometry {
        popup: Rect::new(
            candidate.x.clamp(surface.x, max_x),
            candidate.y.clamp(surface.y, max_y),
            width,
            height,
        ),
        placement: resolved,
    }
}

fn rect_for_popconfirm_placement(
    frame: Rect,
    placement: PopconfirmPlacement,
    arrow: bool,
    width: f32,
    height: f32,
) -> Rect {
    let (x, y) = popconfirm_position(frame, placement, arrow, width, height);
    Rect::new(x, y, width, height)
}

fn popconfirm_overflow_score(rect: Rect, surface: Rect) -> f32 {
    (surface.x - rect.x).max(0.0)
        + (surface.y - rect.y).max(0.0)
        + (rect.x + rect.w - surface.x - surface.w).max(0.0)
        + (rect.y + rect.h - surface.y - surface.h).max(0.0)
}

fn button_rects_for_popup(popup: Rect) -> (Rect, Rect) {
    let inset = 12.0_f32.min(popup.w * 0.5);
    let gap = 8.0_f32.min(popup.w);
    let available = (popup.w - inset * 2.0 - gap).max(0.0);
    let button_width = available * 0.5;
    let button_height = 26.0_f32.min((popup.h - 10.0).max(0.0));
    let y = (popup.y + popup.h - 10.0 - button_height).max(popup.y);
    (
        Rect::new(popup.x + inset, y, button_width, button_height),
        Rect::new(
            popup.x + inset + button_width + gap,
            y,
            button_width,
            button_height,
        ),
    )
}

fn popconfirm_position(
    frame: Rect,
    placement: PopconfirmPlacement,
    arrow: bool,
    pw: f32,
    ph: f32,
) -> (f32, f32) {
    let gap = if arrow { 10.0 } else { 4.0 };
    match placement {
        PopconfirmPlacement::Top | PopconfirmPlacement::TopLeft => (frame.x, frame.y - ph - gap),
        PopconfirmPlacement::TopRight => (frame.x + frame.w - pw, frame.y - ph - gap),
        PopconfirmPlacement::Bottom | PopconfirmPlacement::BottomLeft => {
            (frame.x, frame.y + frame.h + gap)
        }
        PopconfirmPlacement::BottomRight => (frame.x + frame.w - pw, frame.y + frame.h + gap),
    }
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

fn expand_popconfirm_rect(rect: Rect, amount: f32) -> Rect {
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

fn draw_popconfirm_arrow(
    ctx: &mut PaintContext,
    trigger: Rect,
    popup: Rect,
    placement: PopconfirmPlacement,
    color: Color,
) {
    let arrow_sz = 6.0;
    let (x1, y1, x2, y2, x3, y3) = match placement {
        PopconfirmPlacement::Top | PopconfirmPlacement::TopLeft | PopconfirmPlacement::TopRight => {
            let cx =
                popconfirm_arrow_anchor(trigger.x + trigger.w * 0.5, popup.x, popup.w, arrow_sz);
            (
                cx - arrow_sz,
                popup.y + popup.h,
                cx + arrow_sz,
                popup.y + popup.h,
                cx,
                popup.y + popup.h + arrow_sz,
            )
        }
        PopconfirmPlacement::Bottom
        | PopconfirmPlacement::BottomLeft
        | PopconfirmPlacement::BottomRight => {
            let cx =
                popconfirm_arrow_anchor(trigger.x + trigger.w * 0.5, popup.x, popup.w, arrow_sz);
            (
                cx - arrow_sz,
                popup.y,
                cx + arrow_sz,
                popup.y,
                cx,
                popup.y - arrow_sz,
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

fn popconfirm_arrow_anchor(desired: f32, start: f32, length: f32, inset: f32) -> f32 {
    if length <= inset * 2.0 {
        start + length * 0.5
    } else {
        desired.clamp(start + inset, start + length - inset)
    }
}
