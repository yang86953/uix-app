use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::native::traits::input::ControlSize;
use crate::ui::animation::{presets, SlideDirection, TransitionPlayer};
use crate::ui::core::widget::WidgetCore;
use crate::ui::SnapshotFields;
use crate::ui::{EventResult, SystemEvent, WidgetTree};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrawerPlacement {
    Right,
    Left,
    Top,
    Bottom,
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
        pub(crate) transition: TransitionPlayer,
        closing: bool,
        pub(crate) transition_dirty: bool,
        last_surface_w: Cell<f32>,
        last_surface_h: Cell<f32>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    visible => (&self) -> bool { self.is_present() }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if !self.is_present() {
            if let SystemEvent::PointerDown { pos, .. } = event {
                if pos.x >= 0.0 && pos.x <= 96.0 && pos.y >= 0.0 && pos.y <= 32.0 {
                    self.open();
                    return EventResult::Handled;
                }
            }
            return EventResult::NotHandled;
        }

        if let SystemEvent::PointerDown { pos, .. } = event {
            if self.mask_closable {
                let outside = match self.placement {
                    DrawerPlacement::Right => pos.x < 0.0,
                    DrawerPlacement::Left => pos.x >= self.width,
                    DrawerPlacement::Top => pos.y >= self.height,
                    DrawerPlacement::Bottom => pos.y < 0.0,
                };
                if outside {
                    self.close();
                    return EventResult::Handled;
                }
            }

            if self.closable {
                let cx = self.width - 36.0;
                if pos.x >= cx - 12.0 && pos.x <= cx + 12.0 && pos.y >= 8.0 && pos.y <= 32.0 {
                    self.close();
                    return EventResult::Handled;
                }
            }
        }

        if let SystemEvent::KeyDown { key, .. } = event {
            if *key == crate::ui::KeyCode::Escape && self.closable {
                self.close();
                return EventResult::Handled;
            }
        }

        EventResult::Handled
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let loc = crate::ui::locale::use_locale();
        if !self.is_present() {
            let primary = ctx.tokens().color_primary();
            let trigger = Rect::new(frame.x, frame.y, 96.0, 32.0);
            ctx.fill_rect(trigger, primary, Some(Radius::uniform(ctx.tokens().border_radius())));
            ctx.text_center("打开 Drawer", trigger, Color::white(), 13.0);
            return;
        }

        let mask_alpha = (96.0 * self.transition_opacity()).round().clamp(0.0, 96.0) as u8;
        if self.mask {
            ctx.fill_rect(
                Rect::new(-2000.0, -2000.0, 4000.0, 4000.0),
                Color::from_rgba(0, 0, 0, mask_alpha),
                None,
            );
        }

        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border_secondary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let r = Radius::uniform(ctx.tokens().border_radius_lg());

        let drawer_rect = if self.mask {
            let surface_w = ctx.canvas_2d().width() as f32;
            let surface_h = ctx.canvas_2d().height() as f32;
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
        let drawer_rect = self.apply_transition_to_rect(drawer_rect);
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
        ctx.fill_rect(drawer_rect, bg, corner);
        ctx.stroke_rect(drawer_rect, border, 1.0, corner);

        let header_rect = Rect::new(drawer_x, drawer_y, drawer_w, 48.0);
        let ty = ctx.visual_center_y(header_rect, 16.0);
        ctx.draw_text(&self.title, Point::new(drawer_x + 24.0, ty), text, 16.0);

        if !self.extra.is_empty() {
            ctx.draw_text(
                &self.extra,
                Point::new(drawer_x + drawer_w - 120.0, ty),
                text_sec,
                14.0,
            );
        }
        if self.closable {
            ctx.draw_text("x", Point::new(drawer_x + drawer_w - 36.0, ty), text_sec, 16.0);
        }
        ctx.fill_rect(Rect::new(drawer_x, drawer_y + 48.0, drawer_w, 1.0), border, None);

        let footer_h = if self.footer_visible { 56.0 } else { 0.0 };
        if self.footer_visible {
            let footer_y = drawer_y + drawer_h - footer_h;
            ctx.fill_rect(Rect::new(drawer_x, footer_y, drawer_w, 1.0), border, None);
            let primary = ctx.tokens().color_primary();
            let btn_r = Some(Radius::uniform(4.0));
            let ok_rect = Rect::new(drawer_x + drawer_w - 100.0, footer_y + 14.0, 80.0, 28.0);
            ctx.fill_rect(ok_rect, primary, btn_r);
            ctx.text_center(loc.drawer_ok, ok_rect, Color::white(), 13.0);
        }
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
        if !self.is_present() || children.is_empty() {
            return Vec::new();
        }
        let drawer_rect = if self.mask {
            let (surface_w, surface_h) = tree
                .root_id()
                .and_then(|root_id| tree.get(root_id))
                .map(|root| (root.frame().w, root.frame().h))
                .filter(|(w, h)| *w > 0.0 && *h > 0.0)
                .unwrap_or((1200.0, 760.0));
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
        let drawer_x = drawer_rect.x;
        let drawer_y = drawer_rect.y;
        let drawer_w = drawer_rect.w;
        let drawer_h = drawer_rect.h;
        let footer_h = if self.footer_visible { 56.0 } else { 0.0 };
        let body_y = drawer_y + 56.0;
        let body_h = drawer_h - 56.0 - footer_h;
        let pad = 24.0;
        children
            .iter()
            .map(|child| {
                (
                    child.id,
                    Rect::new(
                        drawer_x + pad,
                        body_y + pad,
                        drawer_w - pad * 2.0,
                        body_h - pad * 2.0,
                    ),
                )
            })
            .collect()
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
        Self {
            title: title.to_string(),
            visible: false,
            width: 378.0,
            height: 300.0,
            drawer_size: ControlSize::Medium,
            placement: DrawerPlacement::Right,
            closable: true,
            mask_closable: true,
            mask: true,
            footer_visible: false,
            extra: String::new(),
            transition: TransitionPlayer::new(presets::drawer_enter(Self::slide_direction_for(
                DrawerPlacement::Right,
            ))),
            closing: false,
            transition_dirty: false,
            last_surface_w: Cell::new(0.0),
            last_surface_h: Cell::new(0.0),
        }
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
        self.width = w;
        self.height = h;
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
        self.transition =
            TransitionPlayer::new(presets::drawer_enter(Self::slide_direction_for(p)));
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

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn open(&mut self) {
        self.visible = true;
        self.closing = false;
        self.transition = TransitionPlayer::new(presets::drawer_enter(Self::slide_direction_for(
            self.placement,
        )));
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
        self.transition = TransitionPlayer::new(presets::drawer_exit(Self::slide_direction_for(
            self.placement,
        )));
        self.transition_dirty = true;
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
        match self.placement {
            DrawerPlacement::Right => Rect::new(surface_w - self.width, 0.0, self.width, surface_h),
            DrawerPlacement::Left => Rect::new(0.0, 0.0, self.width, surface_h),
            DrawerPlacement::Top => Rect::new(0.0, 0.0, surface_w, self.height),
            DrawerPlacement::Bottom => {
                Rect::new(0.0, surface_h - self.height, surface_w, self.height)
            }
        }
    }

    fn slide_direction_for(placement: DrawerPlacement) -> SlideDirection {
        match placement {
            DrawerPlacement::Right => SlideDirection::Left,
            DrawerPlacement::Left => SlideDirection::Right,
            DrawerPlacement::Top => SlideDirection::Down,
            DrawerPlacement::Bottom => SlideDirection::Up,
        }
    }

    fn intrinsic_size(&self) -> Size {
        if self.is_present() {
            if self.mask {
                // Masked drawer paints in overlay space; keep layout slot empty.
                Size::zero()
            } else {
                match self.placement {
                    DrawerPlacement::Right | DrawerPlacement::Left => Size::new(self.width, 600.0),
                    DrawerPlacement::Top | DrawerPlacement::Bottom => Size::new(400.0, self.height),
                }
            }
        } else {
            Size::new(96.0, 32.0)
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Drawer {
            title: self.title.clone(),
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
