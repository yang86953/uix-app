use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::{Color, FillRule, PathBuilder, Radius};
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::component::paint_context::PaintContext;
use crate::ui::component_snapshot::SnapshotPopconfirm;
use crate::ui::SnapshotFields;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, WidgetTree,
};

mod geometry;

use self::geometry::*;

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

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
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

        let loc = crate::ui::component::locale::use_locale();
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
        if self.focused && tree.keyboard_focus_visible() {
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

            let loc = crate::ui::component::locale::use_locale();
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
                crate::ui::widgets::icon::Icon::paint_in_frame(
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
            if self.focused && tree.keyboard_focus_visible() && self.visible {
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
            .unwrap_or_default();
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

