use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::api::PaintContext;
use crate::draw::{Color, Radius};
use crate::native::traits::input::ControlSize;
use crate::ui::animation::{presets, AnimationConfig, TransitionPlayer};
use crate::ui::core::widget::WidgetCore;
use crate::ui::SnapshotFields;
use crate::ui::{EventResult, MouseButton, SystemEvent, WidgetTree};

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
        let loc = crate::ui::locale::use_locale();
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

impl Drawer {
    pub fn new(title: &str) -> Self {
        let size = crate::ui::config::use_config().size;
        Self {
            title: title.to_string(),
            visible: false,
            width: 378.0,
            height: 300.0,
            drawer_size: size,
            placement: DrawerPlacement::Right,
            closable: true,
            mask_closable: true,
            mask: true,
            footer_visible: false,
            extra: String::new(),
            enter_animation: None,
            leave_animation: None,
            transition: TransitionPlayer::new(presets::drawer_enter(
                Self::animation_placement_for(DrawerPlacement::Right),
            )),
            closing: false,
            transition_dirty: false,
            layout_requested: Cell::new(false),
            last_frame: Cell::new(Rect::zero()),
            last_surface_w: Cell::new(0.0),
            last_surface_h: Cell::new(0.0),
            last_trigger_rect: Cell::new(Rect::new(0.0, 0.0, 96.0, 32.0)),
            last_panel_rect: Cell::new(Rect::zero()),
            close_hovered: Cell::new(false),
            pressed_target: Cell::new(None),
            activation_key: Cell::new(None),
        }
        .drawer_size(size)
    }

    pub fn visible(mut self, v: bool) -> Self {
        self.set_visible(v);
        self
    }

    pub fn show(mut self) -> Self {
        self.open();
        self
    }

    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.width = Self::normalize_dimension(w);
        self.height = Self::normalize_dimension(h);
        self
    }

    pub fn drawer_size(mut self, s: ControlSize) -> Self {
        self.drawer_size = s;
        match s {
            ControlSize::Small => {
                self.width = 300.0;
                self.height = 200.0;
            }
            ControlSize::Medium => {
                self.width = 378.0;
                self.height = 300.0;
            }
            ControlSize::Large => {
                self.width = 600.0;
                self.height = 450.0;
            }
        }
        self
    }

    pub fn placement(mut self, p: DrawerPlacement) -> Self {
        self.placement = p;
        if self.visible && !self.closing {
            self.restart_enter_transition();
        }
        self
    }

    pub fn closable(mut self, v: bool) -> Self {
        self.closable = v;
        self
    }

    pub fn mask_closable(mut self, v: bool) -> Self {
        self.mask_closable = v;
        self
    }

    pub fn mask(mut self, v: bool) -> Self {
        self.mask = v;
        self
    }

    pub fn footer_visible(mut self, v: bool) -> Self {
        self.footer_visible = v;
        self
    }

    pub fn extra(mut self, t: impl Into<String>) -> Self {
        self.extra = t.into();
        self
    }

    /// 设置打开时播放的动画；已打开时从当前声明重新开始进场。
    pub fn enter_animation(mut self, animation: AnimationConfig) -> Self {
        self.enter_animation = Some(animation);
        if self.visible && !self.closing {
            self.restart_enter_transition();
        }
        self
    }

    /// 设置关闭时播放的动画。
    pub fn leave_animation(mut self, animation: AnimationConfig) -> Self {
        self.leave_animation = Some(animation);
        if self.closing {
            self.transition = TransitionPlayer::new(animation);
            self.transition_dirty = true;
        }
        self
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn open(&mut self) {
        self.cancel_interaction();
        self.visible = true;
        self.closing = false;
        self.restart_enter_transition();
        self.layout_requested.set(true);
    }

    pub fn close(&mut self) {
        self.cancel_interaction();
        if !self.is_present() {
            self.visible = false;
            self.closing = false;
            self.transition_dirty = false;
            return;
        }
        self.visible = false;
        self.closing = true;
        self.transition = TransitionPlayer::new(self.resolved_leave_animation());
        self.transition_dirty = true;
        self.layout_requested.set(true);
    }

    pub fn set_visible(&mut self, v: bool) {
        if v {
            self.open();
        } else {
            self.close();
        }
    }

    pub(crate) fn is_present(&self) -> bool {
        self.visible || self.closing
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let interaction_geometry_changed = self.width != next.width
            || self.height != next.height
            || self.placement != next.placement
            || self.closable != next.closable
            || self.mask != next.mask;
        self.title = next.title;
        self.width = next.width;
        self.height = next.height;
        self.drawer_size = next.drawer_size;
        self.placement = next.placement;
        self.closable = next.closable;
        self.mask_closable = next.mask_closable;
        self.mask = next.mask;
        self.footer_visible = next.footer_visible;
        self.extra = next.extra;
        self.enter_animation = next.enter_animation;
        self.leave_animation = next.leave_animation;
        if interaction_geometry_changed {
            self.cancel_interaction();
        }
    }

    fn restart_enter_transition(&mut self) {
        let animation = self.resolved_enter_animation();
        let mut transition = TransitionPlayer::new(animation);
        // 与 View 进出场一致：零时长动画在创建后立即完成，避免登记多余帧。
        if animation.duration() <= 0.0 {
            transition.update(0.0);
        }
        self.transition = transition;
        self.transition_dirty = true;
    }

    fn panel_rect_local(&self) -> Rect {
        let frame = self.last_frame.get();
        let panel = self.last_panel_rect.get();
        let panel = if panel.w > 0.0 && panel.h > 0.0 {
            panel
        } else if self.mask {
            self.overlay_rect_for_surface(
                self.last_surface_w.get().max(1200.0),
                self.last_surface_h.get().max(760.0),
            )
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
        Rect::new(panel.x - frame.x, panel.y - frame.y, panel.w, panel.h)
    }

    fn close_rect_local(&self) -> Rect {
        let panel = self.panel_rect_local();
        let close_width = panel.w.min(48.0);
        Rect::new(
            panel.x + panel.w - close_width,
            panel.y,
            close_width,
            panel.h.min(48.0),
        )
    }

    fn pointer_target_at(&self, pos: Point) -> Option<DrawerPointerTarget> {
        if self.closable && self.close_rect_local().contains(pos) {
            Some(DrawerPointerTarget::Close)
        } else if self.mask_closable && !self.panel_rect_local().contains(pos) {
            Some(DrawerPointerTarget::Mask)
        } else {
            None
        }
    }

    fn cancel_interaction(&self) {
        self.close_hovered.set(false);
        self.pressed_target.set(None);
        self.activation_key.set(None);
    }

    fn normalize_frame(frame: Rect) -> Rect {
        Rect::new(
            if frame.x.is_finite() { frame.x } else { 0.0 },
            if frame.y.is_finite() { frame.y } else { 0.0 },
            Self::normalize_dimension(frame.w),
            Self::normalize_dimension(frame.h),
        )
    }

    fn transition_opacity(&self) -> f32 {
        self.transition.opacity_progress.clamp(0.0, 1.0)
    }

    fn apply_transition_to_rect(&self, rect: Rect) -> Rect {
        Rect::new(
            rect.x + self.transition.offset.x,
            rect.y + self.transition.offset.y,
            rect.w,
            rect.h,
        )
    }

    fn overlay_rect_for_surface(&self, surface_w: f32, surface_h: f32) -> Rect {
        let surface_w = Self::normalize_dimension(surface_w);
        let surface_h = Self::normalize_dimension(surface_h);
        let width = Self::normalize_dimension(self.width).min(surface_w);
        let height = Self::normalize_dimension(self.height).min(surface_h);
        match self.placement {
            DrawerPlacement::Right => Rect::new(surface_w - width, 0.0, width, surface_h),
            DrawerPlacement::Left => Rect::new(0.0, 0.0, width, surface_h),
            DrawerPlacement::Top => Rect::new(0.0, 0.0, surface_w, height),
            DrawerPlacement::Bottom => Rect::new(0.0, surface_h - height, surface_w, height),
        }
    }

    fn trigger_rect_for_size(frame_w: f32, frame_h: f32) -> Rect {
        let frame_w = Self::normalize_dimension(frame_w);
        let frame_h = Self::normalize_dimension(frame_h);
        let width = frame_w.min(96.0);
        Rect::new((frame_w - width) * 0.5, 0.0, width, frame_h.min(32.0))
    }

    fn trigger_rect_local(&self) -> Rect {
        let trigger = self.last_trigger_rect.get();
        if trigger.w > 0.0 && trigger.h > 0.0 {
            trigger
        } else {
            Rect::new(0.0, 0.0, 96.0, 32.0)
        }
    }

    fn body_rect(&self, panel: Rect) -> Rect {
        let header_h = panel.h.min(48.0);
        let footer_h = if self.footer_visible {
            (panel.h - header_h).clamp(0.0, 56.0)
        } else {
            0.0
        };
        Rect::new(
            panel.x,
            panel.y + header_h,
            panel.w.max(0.0),
            (panel.h - header_h - footer_h).max(0.0),
        )
    }

    fn normalize_dimension(value: f32) -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    }

    fn paint_elided_text(
        ctx: &mut PaintContext<'_>,
        value: &str,
        frame: Rect,
        color: Color,
        font_size: f32,
    ) {
        if value.is_empty() || frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let value = value.replace(['\r', '\n'], " ");
        let visible = if Self::text_width(ctx, &value, font_size) <= frame.w {
            value
        } else {
            const ELLIPSIS: char = '…';
            let mut visible = String::new();
            for ch in value.chars() {
                visible.push(ch);
                visible.push(ELLIPSIS);
                let fits = Self::text_width(ctx, &visible, font_size) <= frame.w;
                visible.pop();
                if !fits {
                    visible.pop();
                    break;
                }
            }
            visible.push(ELLIPSIS);
            visible
        };
        ctx.push_clip(frame);
        let y = ctx.visual_center_y(frame, font_size);
        ctx.draw_text(&visible, Point::new(frame.x, y), color, font_size);
        ctx.pop_clip();
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

    fn animation_placement_for(placement: DrawerPlacement) -> crate::ui::Placement {
        match placement {
            DrawerPlacement::Right => crate::ui::Placement::Right,
            DrawerPlacement::Left => crate::ui::Placement::Left,
            DrawerPlacement::Top => crate::ui::Placement::Top,
            DrawerPlacement::Bottom => crate::ui::Placement::Bottom,
        }
    }

    fn resolved_enter_animation(&self) -> AnimationConfig {
        self.enter_animation
            .unwrap_or_else(|| presets::drawer_enter(Self::animation_placement_for(self.placement)))
    }

    fn resolved_leave_animation(&self) -> AnimationConfig {
        self.leave_animation
            .unwrap_or_else(|| presets::drawer_exit(Self::animation_placement_for(self.placement)))
    }

    fn intrinsic_size(&self) -> Size {
        if self.is_present() {
            if self.mask {
                // Masked drawer paints in overlay space; keep layout slot empty.
                Size::zero()
            } else {
                match self.placement {
                    DrawerPlacement::Right | DrawerPlacement::Left => {
                        Size::new(Self::normalize_dimension(self.width), 600.0)
                    }
                    DrawerPlacement::Top | DrawerPlacement::Bottom => {
                        Size::new(400.0, Self::normalize_dimension(self.height))
                    }
                }
            }
        } else {
            Size::new(96.0, 32.0)
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Drawer {
            title: self.title.clone(),
            open: self.is_present(),
            width: self.width,
            height: self.height,
            drawer_size: self.drawer_size,
            placement: self.placement,
            closable: self.closable,
            mask_closable: self.mask_closable,
            mask: self.mask,
            footer_visible: self.footer_visible,
            extra: self.extra.clone(),
        }
    }
}
