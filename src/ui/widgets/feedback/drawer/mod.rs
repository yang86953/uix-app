use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::{Color, Radius};
use crate::native::windowing::input::ControlSize;
use crate::ui::animation::{presets, AnimationConfig, TransitionPlayer};
use crate::ui::component::paint_context::PaintContext;
use crate::ui::component::widget::WidgetCore;
use crate::ui::SnapshotFields;
use crate::ui::{EventResult, MouseButton, SystemEvent, WidgetTree};

mod methods;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrawerPlacement {
    Right,
    Left,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DrawerPointerTarget {
    Trigger,
    Close,
    Mask,
}

component! {
    /// Sliding drawer panel.
    pub struct Drawer {
        title: String,
        visible: bool,
        width: f32,
        height: f32,
        drawer_size: ControlSize,
        placement: DrawerPlacement,
        closable: bool,
        mask_closable: bool,
        mask: bool,
        footer_visible: bool,
        extra: String,
        enter_animation: Option<AnimationConfig>,
        leave_animation: Option<AnimationConfig>,
        pub(crate) transition: TransitionPlayer,
        closing: bool,
        pub(crate) transition_dirty: bool,
        layout_requested: Cell<bool>,
        last_frame: Cell<Rect>,
        last_surface_w: Cell<f32>,
        last_surface_h: Cell<f32>,
        last_trigger_rect: Cell<Rect>,
        last_panel_rect: Cell<Rect>,
        close_hovered: Cell<bool>,
        pressed_target: Cell<Option<DrawerPointerTarget>>,
        activation_key: Cell<Option<crate::ui::KeyCode>>,
    }

    // Closed drawers still render and receive input through their trigger.
    visible => (&self) -> bool { true }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    tab_index => (&self) -> i32 { i32::from(!self.is_present()) }

    hit_test_frame => (&self, actual_frame: Rect) -> Rect {
        self.last_frame.set(Self::normalize_frame(actual_frame));
        if self.mask && self.is_present() {
            Rect::new(0.0, 0.0, self.last_surface_w.get(), self.last_surface_h.get())
        } else if !self.is_present() {
            let trigger = Self::trigger_rect_for_size(actual_frame.w, actual_frame.h);
            self.last_trigger_rect.set(trigger);
            Rect::new(
                actual_frame.x + trigger.x,
                actual_frame.y + trigger.y,
                trigger.w,
                trigger.h,
            )
        } else {
            actual_frame
        }
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if !self.is_present() {
            let trigger = self.trigger_rect_local();
            return match event {
                SystemEvent::PointerDown {
                    pos,
                    button: MouseButton::Left,
                    ..
                } if trigger.contains(*pos) => {
                    self.close_hovered.set(true);
                    self.pressed_target.set(Some(DrawerPointerTarget::Trigger));
                    EventResult::Handled
                }
                SystemEvent::PointerUp {
                    pos,
                    button: MouseButton::Left,
                    ..
                } if self.pressed_target.replace(None) == Some(DrawerPointerTarget::Trigger) => {
                    let released_inside = trigger.contains(*pos);
                    self.close_hovered.set(released_inside);
                    if released_inside {
                        self.open();
                    }
                    EventResult::Handled
                }
                SystemEvent::PointerMove { pos, .. } => {
                    let hovered = trigger.contains(*pos);
                    if self.close_hovered.replace(hovered) != hovered {
                        EventResult::Handled
                    } else {
                        EventResult::NotHandled
                    }
                }
                SystemEvent::PointerLeave | SystemEvent::FocusOut => {
                    self.cancel_interaction();
                    EventResult::Handled
                }
                SystemEvent::KeyDown {
                    key: key @ (crate::ui::KeyCode::Enter | crate::ui::KeyCode::Space),
                    ..
                } => {
                    self.activation_key.set(Some(*key));
                    EventResult::Handled
                }
                SystemEvent::KeyUp {
                    key: key @ (crate::ui::KeyCode::Enter | crate::ui::KeyCode::Space),
                    ..
                } if self.activation_key.replace(None) == Some(*key) => {
                    self.open();
                    EventResult::Handled
                }
                SystemEvent::FocusIn => EventResult::Handled,
                _ => EventResult::NotHandled,
            };
        }

        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let target = self.pointer_target_at(*pos);
                self.close_hovered
                    .set(target == Some(DrawerPointerTarget::Close));
                self.pressed_target.set(target);
                if target.is_some() {
                    return EventResult::Handled;
                }
                EventResult::Handled
            }
            SystemEvent::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let armed = self.pressed_target.replace(None);
                let target = self.pointer_target_at(*pos);
                self.close_hovered
                    .set(target == Some(DrawerPointerTarget::Close));
                if armed.is_some() && armed == target {
                    self.close();
                }
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                let hovered = self.pointer_target_at(*pos) == Some(DrawerPointerTarget::Close);
                self.close_hovered.set(hovered);
                EventResult::Handled
            }
            SystemEvent::PointerLeave | SystemEvent::FocusOut => {
                self.cancel_interaction();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => {
                if *key == crate::ui::KeyCode::Escape && self.closable {
                    self.close();
                    return EventResult::Handled;
                }
                EventResult::Handled
            }
            _ => EventResult::Handled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.last_frame.set(Self::normalize_frame(frame));
        let loc = crate::ui::component::locale::use_locale();
        if !self.is_present() {
            let local_trigger = Self::trigger_rect_for_size(frame.w, frame.h);
            self.last_trigger_rect.set(local_trigger);
            let primary = if self.pressed_target.get() == Some(DrawerPointerTarget::Trigger)
                || self.activation_key.get().is_some()
            {
                ctx.tokens().color_primary_active()
            } else if self.close_hovered.get() {
                ctx.tokens().color_primary_hover()
            } else {
                ctx.tokens().color_primary()
            };
            let frame_x = if frame.x.is_finite() { frame.x } else { 0.0 };
            let frame_y = if frame.y.is_finite() { frame.y } else { 0.0 };
            let trigger = Rect::new(
                frame_x + local_trigger.x,
                frame_y + local_trigger.y,
                local_trigger.w,
                local_trigger.h,
            );
            if trigger.w <= 0.0 || trigger.h <= 0.0 {
                return;
            }
            ctx.push_clip(trigger);
            ctx.fill_rect(trigger, primary, Some(Radius::uniform(ctx.tokens().border_radius())));
            ctx.text_center("打开 Drawer", trigger, Color::white(), 13.0);
            ctx.pop_clip();
            return;
        }

        let mask_alpha = (96.0 * self.transition_opacity()).round().clamp(0.0, 96.0) as u8;
        let surface_extent = self.mask.then(|| {
            let surface_size = ctx.surface_size();
            (surface_size.w, surface_size.h)
        });
        if let Some((surface_w, surface_h)) = surface_extent {
            self.last_surface_w.set(surface_w);
            self.last_surface_h.set(surface_h);
            ctx.fill_rect(
                Rect::new(0.0, 0.0, surface_w, surface_h),
                Color::from_rgba(0, 0, 0, mask_alpha),
                None,
            );
        }

        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border_secondary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let r = Radius::uniform(ctx.tokens().border_radius_lg());

        let drawer_rect = if let Some((surface_w, surface_h)) = surface_extent {
            self.overlay_rect_for_surface(surface_w, surface_h)
        } else {
            match self.placement {
                DrawerPlacement::Right | DrawerPlacement::Left => {
                    Rect::new(frame.x, frame.y, self.width, frame.h)
                }
                DrawerPlacement::Top | DrawerPlacement::Bottom => {
                    Rect::new(frame.x, frame.y, frame.w, self.height)
                }
            }
        };
        let drawer_rect = self.apply_transition_to_rect(drawer_rect);
        self.last_panel_rect.set(drawer_rect);
        let drawer_x = drawer_rect.x;
        let drawer_y = drawer_rect.y;
        let drawer_w = drawer_rect.w;
        let drawer_h = drawer_rect.h;
        let corner = match self.placement {
            DrawerPlacement::Right => Some(Radius { tl: r.tl, tr: 0.0, br: 0.0, bl: r.br }),
            DrawerPlacement::Left => Some(Radius { tl: 0.0, tr: r.tr, br: r.bl, bl: 0.0 }),
            DrawerPlacement::Top => Some(Radius { tl: 0.0, tr: 0.0, br: r.bl, bl: r.br }),
            DrawerPlacement::Bottom => Some(Radius { tl: r.tl, tr: r.tr, br: 0.0, bl: 0.0 }),
        };
        ctx.push_clip(drawer_rect);
        ctx.fill_rect(drawer_rect, bg, corner);
        ctx.stroke_rect(drawer_rect, border, 1.0, corner);

        let header_rect = Rect::new(drawer_x, drawer_y, drawer_w, drawer_h.min(48.0));
        let close_w = if self.closable { drawer_w.min(48.0) } else { 0.0 };
        let extra_w = if self.extra.is_empty() {
            0.0
        } else {
            (drawer_w - close_w).clamp(0.0, 120.0)
        };
        let title_x = drawer_x + 24.0_f32.min(drawer_w);
        let title_rect = Rect::new(
            title_x,
            drawer_y,
            (drawer_x + drawer_w - close_w - extra_w - title_x).max(0.0),
            header_rect.h,
        );
        Self::paint_elided_text(ctx, &self.title, title_rect, text, 16.0);

        if !self.extra.is_empty() {
            let extra_rect = Rect::new(
                drawer_x + drawer_w - close_w - extra_w,
                drawer_y,
                extra_w,
                header_rect.h,
            );
            Self::paint_elided_text(ctx, &self.extra, extra_rect, text_sec, 14.0);
        }
        if self.closable {
            let close_rect = Rect::new(
                drawer_x + drawer_w - close_w,
                drawer_y,
                close_w,
                header_rect.h,
            );
            let inset_x = 8.0_f32.min(close_rect.w * 0.5);
            let inset_y = 8.0_f32.min(close_rect.h * 0.5);
            let close_button = Rect::new(
                close_rect.x + inset_x,
                close_rect.y + inset_y,
                (close_rect.w - inset_x * 2.0).max(0.0),
                (close_rect.h - inset_y * 2.0).max(0.0),
            );
            if self.pressed_target.get() == Some(DrawerPointerTarget::Close) {
                ctx.fill_rect(
                    close_button,
                    ctx.tokens().color_fill_secondary(),
                    Some(Radius::uniform(4.0)),
                );
            } else if self.close_hovered.get() {
                ctx.fill_rect(
                    close_button,
                    ctx.tokens().color_fill_tertiary(),
                    Some(Radius::uniform(4.0)),
                );
            }
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                "x",
                close_button,
                text_sec,
                16.0,
            );
        }
        ctx.fill_rect(Rect::new(drawer_x, drawer_y + 48.0, drawer_w, 1.0), border, None);

        let footer_h = if self.footer_visible {
            (drawer_h - header_rect.h).clamp(0.0, 56.0)
        } else {
            0.0
        };
        if self.footer_visible {
            let footer_y = drawer_y + drawer_h - footer_h;
            ctx.fill_rect(Rect::new(drawer_x, footer_y, drawer_w, 1.0), border, None);
            let primary = ctx.tokens().color_primary();
            let btn_r = Some(Radius::uniform(4.0));
            let ok_w = (drawer_w - 40.0).clamp(0.0, 80.0);
            let ok_h = (footer_h - 20.0).clamp(0.0, 28.0);
            if ok_w > 0.0 && ok_h > 0.0 {
                let ok_rect = Rect::new(
                    drawer_x + drawer_w - 20.0_f32.min(drawer_w) - ok_w,
                    footer_y + (footer_h - ok_h) * 0.5,
                    ok_w,
                    ok_h,
                );
                ctx.fill_rect(ok_rect, primary, btn_r);
                ctx.text_center(loc.drawer_ok, ok_rect, Color::white(), 13.0);
            }
        }
        ctx.pop_clip();
    }

    overlay_entry => (&self, id: crate::ui::ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        if !self.is_present() {
            return None;
        }

        let bounds = if self.mask {
            Rect::new(-2000.0, -2000.0, 4000.0, 4000.0)
        } else {
            match self.placement {
                DrawerPlacement::Right | DrawerPlacement::Left => {
                    self.apply_transition_to_rect(Rect::new(frame.x, frame.y, self.width, frame.h))
                }
                DrawerPlacement::Top | DrawerPlacement::Bottom => {
                    self.apply_transition_to_rect(Rect::new(frame.x, frame.y, frame.w, self.height))
                }
            }
        };

        Some(
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Drawer)
                .bounds(bounds)
                .z_index(1000),
        )
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        if !self.is_present() {
            return Vec::new();
        }
        let frame = Self::normalize_frame(frame);
        self.last_frame.set(frame);
        let drawer_rect = if self.mask {
            let (surface_w, surface_h) = tree
                .root_id()
                .and_then(|root_id| tree.get(root_id))
                .map(|root| (root.frame().w, root.frame().h))
                .filter(|(w, h)| *w > 0.0 && *h > 0.0)
                .map(|(w, h)| (Self::normalize_dimension(w), Self::normalize_dimension(h)))
                .unwrap_or((1200.0, 760.0));
            self.last_surface_w.set(surface_w);
            self.last_surface_h.set(surface_h);
            self.overlay_rect_for_surface(surface_w, surface_h)
        } else {
            match self.placement {
                DrawerPlacement::Right | DrawerPlacement::Left => {
                    Rect::new(frame.x, frame.y, self.width, frame.h)
                }
                DrawerPlacement::Top | DrawerPlacement::Bottom => {
                    Rect::new(frame.x, frame.y, frame.w, self.height)
                }
            }
        };
        self.last_panel_rect.set(drawer_rect);
        if children.is_empty() {
            return Vec::new();
        }
        let drawer_rect = self.apply_transition_to_rect(drawer_rect);
        self.last_panel_rect.set(drawer_rect);
        let drawer_x = drawer_rect.x;
        let drawer_y = drawer_rect.y;
        let drawer_w = drawer_rect.w;
        let drawer_h = drawer_rect.h;
        let footer_h = if self.footer_visible { 56.0 } else { 0.0 };
        let header_h = drawer_h.min(48.0);
        let body_y = drawer_y + header_h;
        let body_h = (drawer_h - header_h - footer_h).max(0.0);
        let pad = 24.0;
        children
            .iter()
            .map(|child| {
                (
                    child.id,
                    Rect::new(
                        drawer_x + pad,
                        body_y + pad,
                        (drawer_w - pad * 2.0).max(0.0),
                        (body_h - pad * 2.0).max(0.0),
                    ),
                )
            })
            .collect()
    }

    children_clip => (&self, _frame: Rect) -> Option<Rect> {
        if self.is_present() {
            Some(self.body_rect(self.last_panel_rect.get()))
        } else {
            Some(Rect::zero())
        }
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
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
            self.layout_requested.set(true);
        }

        self.is_present() && !self.transition.finished
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty {
            if self.mask {
                let surface_w = self.last_surface_w.get();
                let surface_h = self.last_surface_h.get();
                if surface_w > 0.0 && surface_h > 0.0 {
                    return Rect::new(0.0, 0.0, surface_w, surface_h);
                }
            }
            frame
        } else {
            Rect::zero()
        }
    }
}
