use crate::define_widget;
use crate::animation::transition::{presets, Transition, TransitionPlayer};
use uix_core::{ControlSize, Point, Rect, Size};
use uix_graphics::{Color, Radius};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};

/// Modal — 模态对话框。
define_widget! {
    pub struct Modal {
        title: String,
        visible: bool,
        width: f32,
        height: f32,
        modal_size: ControlSize,
        closable: bool,
        mask_closable: bool,
        footer_visible: bool,
        centered: bool,
        on_ok: Option<Box<dyn FnMut() + 'static>>,
        on_cancel: Option<Box<dyn FnMut() + 'static>>,
        transition_player: Option<TransitionPlayer>,
        /// 上次 visible 值，用于检测变化触发过渡动画。
        prev_visible: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        if self.visible { Size::new(self.width, self.height) } else { Size::zero() }
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if !self.visible { return EventResult::NotHandled; }
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                let close_rect = Rect::new(self.width - 48.0, 0.0, 48.0, 48.0);
                if self.closable && close_rect.contains(*pos) {
                    self.close();
                    if let Some(ref mut cb) = self.on_cancel { cb(); }
                    return EventResult::Handled;
                }
                if self.mask_closable && (pos.x < 0.0 || pos.y < 0.0) {
                    self.close();
                    if let Some(ref mut cb) = self.on_cancel { cb(); }
                    return EventResult::Handled;
                }
                EventResult::Handled
            }
            WidgetEvent::KeyDown { key, .. } => {
                if *key == crate::widget::KeyCode::Escape && self.closable {
                    self.close();
                    if let Some(ref mut cb) = self.on_cancel { cb(); }
                    return EventResult::Handled;
                }
                EventResult::Handled
            }
            _ => { if self.visible { EventResult::Handled } else { EventResult::NotHandled } }
        }
    }

    on_update => (&mut self, dt: f32) {
        if self.visible != self.prev_visible {
            self.prev_visible = self.visible;
            if self.visible {
                self.transition_player = Some(TransitionPlayer::new(presets::modal_enter()));
            } else {
                self.transition_player = Some(TransitionPlayer::new(presets::modal_exit()));
            }
        }
        if let Some(ref mut tp) = self.transition_player {
            tp.update(dt as f64);
        }
    }

    needs_continuous_update => (&self) -> bool {
        self.transition_player.as_ref().map_or(false, |tp| !tp.finished)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if !self.visible { return; }
        let opacity = self.transition_player.as_ref().map_or(1.0, |tp| tp.opacity_progress);
        let scale = self.transition_player.as_ref().map_or(1.0, |tp| tp.scale);

        let border_radius_lg = ctx.tokens().border_radius_lg();
        let bg_container = ctx.tokens().color_bg_container();
        let border_secondary = ctx.tokens().color_border_secondary();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();

        ctx.engine().set_opacity(opacity);

        ctx.fill_rect(Rect::new(frame.x - 1000.0, frame.y - 1000.0, frame.w + 2000.0, frame.h + 2000.0),
            Color::from_rgba(0, 0, 0, 128), None);

        let cx = frame.x + frame.w / 2.0;
        let cy = frame.y + frame.h / 2.0;
        let sw = frame.w * scale;
        let sh = frame.h * scale;
        let scaled_frame = Rect::new(cx - sw / 2.0, cy - sh / 2.0, sw, sh);

        let radius = Some(Radius::uniform(border_radius_lg));
        ctx.fill_rect(scaled_frame, bg_container, radius);
        ctx.stroke_rect(scaled_frame, border_secondary, 1.0, radius);

        let title_h = 56.0;
        let footer_h = if self.footer_visible { 56.0 } else { 0.0 };

        let title_rect = Rect::new(scaled_frame.x, scaled_frame.y, scaled_frame.w, title_h);
        let ty = ctx.visual_center_y(title_rect, 16.0);
        ctx.draw_text(&self.title, Point::new(scaled_frame.x + 24.0, ty), text_color, 16.0);
        ctx.fill_rect(Rect::new(scaled_frame.x, scaled_frame.y + title_h, scaled_frame.w, 1.0), border_secondary, None);

        if self.closable {
            ctx.draw_text("✕", Point::new(scaled_frame.x + scaled_frame.w - 36.0, ty), text_secondary, 16.0);
        }
        if self.footer_visible {
            ctx.fill_rect(Rect::new(scaled_frame.x, scaled_frame.y + scaled_frame.h - footer_h, scaled_frame.w, 1.0), border_secondary, None);
        }

        ctx.engine().set_opacity(1.0);
    }

    layout_children => (&self, frame: Rect, children: &[crate::widget::WidgetId], _tree: &WidgetTree)
        -> Vec<(crate::widget::WidgetId, Rect)>
    {
        if !self.visible || children.is_empty() { return Vec::new(); }
        let title_h = 56.0;
        let footer_h = if self.footer_visible { 56.0 } else { 0.0 };
        let body_y = frame.y + title_h;
        let body_h = frame.h - title_h - footer_h;
        let padding = 24.0;
        children.iter().map(|&cid| (cid, Rect::new(frame.x + padding, body_y + padding, frame.w - padding * 2.0, body_h - padding * 2.0))).collect()
    }
}

impl Modal {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            visible: false, width: 520.0, height: 300.0,
            modal_size: ControlSize::Medium,
            closable: true, mask_closable: true, footer_visible: true,
            centered: true,
            on_ok: None, on_cancel: None,
            transition_player: None,
            prev_visible: false,
        }
    }

    pub fn visible(mut self, v: bool) -> Self { self.visible = v; self.prev_visible = !v; self }
    pub fn show(mut self) -> Self { self.visible = true; self.prev_visible = false; self }
    pub fn size(mut self, w: f32, h: f32) -> Self { self.width = w; self.height = h; self }
    pub fn modal_size(mut self, s: ControlSize) -> Self {
        self.modal_size = s;
        match s {
            ControlSize::Small => { self.width = 400.0; self.height = 200.0; }
            ControlSize::Medium => { self.width = 520.0; self.height = 300.0; }
            ControlSize::Large => { self.width = 720.0; self.height = 400.0; }
        }
        self
    }
    pub fn closable(mut self, v: bool) -> Self { self.closable = v; self }
    pub fn mask_closable(mut self, v: bool) -> Self { self.mask_closable = v; self }
    pub fn footer_visible(mut self, v: bool) -> Self { self.footer_visible = v; self }
    pub fn centered(mut self, v: bool) -> Self { self.centered = v; self }
    pub fn on_ok<F: FnMut() + 'static>(mut self, f: F) -> Self { self.on_ok = Some(Box::new(f)); self }
    pub fn on_cancel<F: FnMut() + 'static>(mut self, f: F) -> Self { self.on_cancel = Some(Box::new(f)); self }
    pub fn is_visible(&self) -> bool { self.visible }
    pub fn set_visible(&mut self, v: bool) { self.prev_visible = self.visible; self.visible = v; }
    pub fn open(&mut self) { self.set_visible(true); }
    pub fn close(&mut self) { self.set_visible(false); }
    pub fn confirm(&mut self) { if let Some(ref mut cb) = self.on_ok { cb(); } self.close(); }
}
