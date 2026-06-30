use crate::define_widget;
use crate::animation::transition::{presets, TransitionPlayer, SlideDirection};
use uix_platform::{ControlSize, Point, Rect, Size};
use uix_graphics::{Color, Radius};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};

/// 抽屉滑出方向。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrawerPlacement {
    Right, Left, Top, Bottom,
}

define_widget! {
    /// Drawer — 抽屉面板。
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
        on_close: Option<Box<dyn FnMut() + 'static>>,
        transition_player: Option<TransitionPlayer>,
        prev_visible: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        if self.visible {
            match self.placement {
                DrawerPlacement::Right | DrawerPlacement::Left => Size::new(self.width, 600.0),
                DrawerPlacement::Top | DrawerPlacement::Bottom => Size::new(400.0, self.height),
            }
        } else {
            Size::zero()
        }
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if !self.visible { return EventResult::NotHandled; }
        if let WidgetEvent::MouseDown { pos, .. } = event {
            if self.mask_closable {
                let outside = match self.placement {
                    DrawerPlacement::Right => pos.x < 0.0,
                    DrawerPlacement::Left  => pos.x >= self.width,
                    DrawerPlacement::Top   => pos.y >= self.height,
                    DrawerPlacement::Bottom => pos.y < 0.0,
                };
                if outside { self.do_close(); return EventResult::Handled; }
            }
            if self.closable {
                let cx = self.width - 36.0;
                if pos.x >= cx - 12.0 && pos.x <= cx + 12.0 && pos.y >= 8.0 && pos.y <= 32.0 {
                    self.do_close(); return EventResult::Handled;
                }
            }
        }
        if let WidgetEvent::KeyDown { key, .. } = event {
            if *key == crate::widget::KeyCode::Escape && self.closable {
                self.do_close(); return EventResult::Handled;
            }
        }
        EventResult::Handled
    }

    on_update => (&mut self, dt: f32) {
        if self.visible != self.prev_visible {
            self.prev_visible = self.visible;
            let dir = match self.placement {
                DrawerPlacement::Right => SlideDirection::Right,
                DrawerPlacement::Left => SlideDirection::Left,
                DrawerPlacement::Top => SlideDirection::Up,
                DrawerPlacement::Bottom => SlideDirection::Down,
            };
            if self.visible {
                self.transition_player = Some(TransitionPlayer::new(presets::drawer_enter(dir)));
            } else {
                self.transition_player = Some(TransitionPlayer::new(presets::drawer_exit(dir)));
            }
        }
        if let Some(ref mut tp) = self.transition_player {
            tp.update(dt as f64);
        }
    }

    needs_continuous_update => (&self) -> bool {
        self.transition_player.as_ref().is_some_and(|tp| !tp.finished)
    }

    render => (&self, _frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let loc = crate::locale::use_locale();
        if !self.visible { return; }
        let opacity = self.transition_player.as_ref().map_or(1.0, |tp| tp.opacity_progress);
        let offset = self.transition_player.as_ref().map_or(Point::zero(), |tp| tp.offset);

        ctx.canvas_2d().set_opacity(opacity);

        if self.mask {
            ctx.fill_rect(Rect::new(-2000.0, -2000.0, 4000.0, 4000.0),
                Color::from_rgba(0, 0, 0, 96), None);
        }

        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border_secondary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let r = Radius::uniform(ctx.tokens().border_radius_lg());

        let (drawer_x, drawer_y, drawer_w, drawer_h) = match self.placement {
            DrawerPlacement::Right => (_frame.x + offset.x, _frame.y, self.width, _frame.h),
            DrawerPlacement::Left  => (_frame.x + offset.x, _frame.y, self.width, _frame.h),
            DrawerPlacement::Top   => (_frame.x, _frame.y + offset.y, _frame.w, self.height),
            DrawerPlacement::Bottom => (_frame.x, _frame.y + offset.y, _frame.w, self.height),
        };
        let drawer_rect = Rect::new(drawer_x, drawer_y, drawer_w, drawer_h);
        let corner = match self.placement {
            DrawerPlacement::Right => Some(Radius { tl: r.tl, tr: 0.0, br: 0.0, bl: r.br }),
            DrawerPlacement::Left  => Some(Radius { tl: 0.0, tr: r.tr, br: r.bl, bl: 0.0 }),
            DrawerPlacement::Top   => Some(Radius { tl: 0.0, tr: 0.0, br: r.bl, bl: r.br }),
            DrawerPlacement::Bottom => Some(Radius { tl: r.tl, tr: r.tr, br: 0.0, bl: 0.0 }),
        };
        ctx.fill_rect(drawer_rect, bg, corner);
        ctx.stroke_rect(drawer_rect, border, 1.0, corner);

        let header_rect = Rect::new(drawer_x, drawer_y, drawer_w, 48.0);
        let ty = ctx.visual_center_y(header_rect, 16.0);
        ctx.draw_text(&self.title, Point::new(drawer_x + 24.0, ty), text, 16.0);

        if !self.extra.is_empty() {
            ctx.draw_text(&self.extra, Point::new(drawer_x + drawer_w - 120.0, ty), text_sec, 14.0);
        }
        if self.closable {
            ctx.draw_text("✕", Point::new(drawer_x + drawer_w - 36.0, ty), text_sec, 16.0);
        }
        ctx.fill_rect(Rect::new(drawer_x, drawer_y + 48.0, drawer_w, 1.0), border, None);

        let footer_h = if self.footer_visible { 56.0 } else { 0.0 };
        if self.footer_visible {
            let footer_y = drawer_y + drawer_h - footer_h;
            ctx.fill_rect(Rect::new(drawer_x, footer_y, drawer_w, 1.0), border, None);
            let primary = ctx.tokens().color_primary();
            let btn_r = Some(Radius::uniform(4.0));
            ctx.fill_rect(Rect::new(drawer_x + drawer_w - 100.0, footer_y + 14.0, 80.0, 28.0), primary, btn_r);
            ctx.text_center(loc.drawer_ok, Rect::new(drawer_x + drawer_w - 100.0, footer_y + 14.0, 80.0, 28.0), Color::white(), 13.0);
        }

        ctx.canvas_2d().set_opacity(1.0);
    }

    layout_children => (&self, frame: Rect, children: &[crate::widget::WidgetId], _tree: &WidgetTree)
        -> Vec<(crate::widget::WidgetId, Rect)>
    {
        if !self.visible || children.is_empty() { return Vec::new(); }
        let (drawer_x, drawer_y, drawer_w, drawer_h) = match self.placement {
            DrawerPlacement::Right => (frame.x, frame.y, self.width, frame.h),
            DrawerPlacement::Left  => (frame.x, frame.y, self.width, frame.h),
            DrawerPlacement::Top   => (frame.x, frame.y, frame.w, self.height),
            DrawerPlacement::Bottom => (frame.x, frame.y, frame.w, self.height),
        };
        let footer_h = if self.footer_visible { 56.0 } else { 0.0 };
        let body_y = drawer_y + 56.0;
        let body_h = drawer_h - 56.0 - footer_h;
        let pad = 24.0;
        children.iter().map(|&cid| (cid, Rect::new(drawer_x + pad, body_y + pad, drawer_w - pad * 2.0, body_h - pad * 2.0))).collect()
    }
}

impl Drawer {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            visible: false,
            width: 378.0, height: 300.0,
            drawer_size: ControlSize::Medium,
            placement: DrawerPlacement::Right,
            closable: true, mask_closable: true, mask: true,
            footer_visible: false,
            extra: String::new(),
            on_close: None,
            transition_player: None,
            prev_visible: false,
        }
    }

    fn do_close(&mut self) {
        self.visible = false;
        if let Some(ref mut cb) = self.on_close { cb(); }
    }

    pub fn visible(mut self, v: bool) -> Self { self.visible = v; self.prev_visible = !v; self }
    pub fn show(mut self) -> Self { self.visible = true; self.prev_visible = false; self }
    pub fn size(mut self, w: f32, h: f32) -> Self { self.width = w; self.height = h; self }
    pub fn drawer_size(mut self, s: ControlSize) -> Self {
        self.drawer_size = s;
        match s {
            ControlSize::Small => { self.width = 300.0; self.height = 200.0; }
            ControlSize::Medium => { self.width = 378.0; self.height = 300.0; }
            ControlSize::Large => { self.width = 600.0; self.height = 450.0; }
        }
        self
    }
    pub fn placement(mut self, p: DrawerPlacement) -> Self { self.placement = p; self }
    pub fn closable(mut self, v: bool) -> Self { self.closable = v; self }
    pub fn mask_closable(mut self, v: bool) -> Self { self.mask_closable = v; self }
    pub fn mask(mut self, v: bool) -> Self { self.mask = v; self }
    pub fn footer_visible(mut self, v: bool) -> Self { self.footer_visible = v; self }
    pub fn extra(mut self, t: impl Into<String>) -> Self { self.extra = t.into(); self }
    pub fn on_close<F: FnMut() + 'static>(mut self, f: F) -> Self { self.on_close = Some(Box::new(f)); self }
    pub fn is_visible(&self) -> bool { self.visible }
    pub fn open(&mut self) { self.prev_visible = self.visible; self.visible = true; }
    pub fn close(&mut self) { self.do_close(); }
    pub fn set_visible(&mut self, v: bool) { self.prev_visible = self.visible; self.visible = v; if !v && self.on_close.is_some() { /* on_close already called in do_close */ } }
}
