use crate::define_widget;
use uix_core::{Point, Rect, Size};
use uix_graphics::{Color, Radius, PathBuilder, FillRule};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};

/// Popconfirm 弹出位置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PopconfirmPlacement {
    Top, TopLeft, TopRight,
    Bottom, BottomLeft, BottomRight,
}

define_widget! {
    pub struct Popconfirm {
        title: String,
        confirm_text: String,
        cancel_text: String,
        visible: bool,
        placement: PopconfirmPlacement,
        arrow: bool,
        icon: bool,
        on_confirm: Option<Box<dyn FnMut() + 'static>>,
        on_cancel: Option<Box<dyn FnMut() + 'static>>,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        Size::new(80.0, 28.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if let WidgetEvent::MouseDown { pos, .. } = event {
            if pos.x >= 0.0 && pos.x <= 80.0 && pos.y >= 0.0 && pos.y <= 28.0 {
                self.visible = !self.visible;
                return EventResult::Handled;
            }
            if self.visible {
                let (pw, ph) = (200.0, 110.0);
                let (px, py) = self.popup_pos(pw, ph);
                let pop_rect = Rect::new(px, py, pw, ph);
                // 点击弹窗外关闭
                if !pop_rect.contains(*pos) {
                    self.visible = false;
                    return EventResult::Handled;
                }
                // 确认按钮
                let confirm_rect = Rect::new(px + 12.0, py + ph - 36.0, 80.0, 26.0);
                let cancel_rect = Rect::new(px + pw - 92.0, py + ph - 36.0, 80.0, 26.0);
                if confirm_rect.contains(*pos) {
                    self.visible = false;
                    if let Some(ref mut cb) = self.on_confirm { cb(); }
                    return EventResult::Handled;
                }
                if cancel_rect.contains(*pos) {
                    self.visible = false;
                    if let Some(ref mut cb) = self.on_cancel { cb(); }
                    return EventResult::Handled;
                }
            }
        }
        EventResult::NotHandled
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        let (pw, ph) = (200.0, 110.0);
        let (px, py) = self.popup_pos(pw, ph);
        let pop = Rect::new(px, py, pw, ph);
        frame.union(&pop)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let loc = crate::locale::use_locale();
        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let primary = ctx.tokens().color_primary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        let trigger_y = ctx.visual_center_y(frame, 13.0);
        ctx.draw_text(loc.delete_text, Point::new(frame.x + 20.0, trigger_y),
            ctx.tokens().color_error(), 13.0);

        if self.visible {
            let (pw, ph) = (200.0, 110.0);
            let (px, py) = self.popup_pos(pw, ph);
            let pop_rect = Rect::new(px, py, pw, ph);
            ctx.fill_rect(pop_rect, bg, r);
            ctx.stroke_rect(pop_rect, border, 1.0, r);

            if self.arrow {
                draw_popconfirm_arrow(ctx, frame, pop_rect, self.placement, bg);
            }

            // 图标 + 标题
            let title_x = if self.icon { px + 36.0 } else { px + 12.0 };
            if self.icon {
                ctx.draw_text("⚠", Point::new(px + 12.0, py + 14.0), ctx.tokens().color_warning(), 16.0);
            }
            let loc = crate::locale::use_locale();
            let title = if self.title.is_empty() { loc.popconfirm_title } else { &self.title };
            ctx.draw_text(title, Point::new(title_x, py + 16.0), text_color, 13.0);

            // 确认按钮
            let btn_r = Some(Radius::uniform(4.0));
            ctx.fill_rect(Rect::new(px + 12.0, py + ph - 36.0, 80.0, 26.0), primary, btn_r);
            let confirm = if self.confirm_text.is_empty() { loc.popconfirm_ok } else { &self.confirm_text };
            ctx.text_center(confirm, Rect::new(px + 12.0, py + ph - 36.0, 80.0, 26.0), Color::white(), 12.0);

            ctx.stroke_rect(Rect::new(px + pw - 92.0, py + ph - 36.0, 80.0, 26.0), border, 1.0, btn_r);
            let cancel = if self.cancel_text.is_empty() { loc.popconfirm_cancel } else { &self.cancel_text };
            ctx.text_center(cancel, Rect::new(px + pw - 92.0, py + ph - 36.0, 80.0, 26.0), text_color, 12.0);
        }
    }
}

impl Default for Popconfirm { fn default() -> Self { Self::new() } }

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
            on_confirm: None,
            on_cancel: None,
        }
    }
    pub fn title(mut self, t: impl Into<String>) -> Self { self.title = t.into(); self }
    pub fn confirm_text(mut self, t: impl Into<String>) -> Self { self.confirm_text = t.into(); self }
    pub fn cancel_text(mut self, t: impl Into<String>) -> Self { self.cancel_text = t.into(); self }
    pub fn placement(mut self, p: PopconfirmPlacement) -> Self { self.placement = p; self }
    pub fn arrow(mut self, v: bool) -> Self { self.arrow = v; self }
    pub fn icon(mut self, v: bool) -> Self { self.icon = v; self }
    pub fn on_confirm<F: FnMut() + 'static>(mut self, f: F) -> Self { self.on_confirm = Some(Box::new(f)); self }
    pub fn on_cancel<F: FnMut() + 'static>(mut self, f: F) -> Self { self.on_cancel = Some(Box::new(f)); self }

    fn popup_pos(&self, pw: f32, ph: f32) -> (f32, f32) {
        let gap = if self.arrow { 10.0 } else { 4.0 };
        match self.placement {
            PopconfirmPlacement::Top | PopconfirmPlacement::TopLeft | PopconfirmPlacement::TopRight => (0.0, -ph - gap),
            PopconfirmPlacement::Bottom | PopconfirmPlacement::BottomLeft | PopconfirmPlacement::BottomRight => (0.0, 28.0 + gap),
        }
    }
}

fn draw_popconfirm_arrow(ctx: &mut RenderContext, trigger: Rect, popup: Rect, placement: PopconfirmPlacement, color: Color) {
    let arrow_sz = 6.0;
    let (x1, y1, x2, y2, x3, y3) = match placement {
        PopconfirmPlacement::Top | PopconfirmPlacement::TopLeft | PopconfirmPlacement::TopRight => {
            let cx = popup.x + popup.w / 2.0;
            (cx - arrow_sz, popup.y + popup.h, cx + arrow_sz, popup.y + popup.h, cx, popup.y + popup.h + arrow_sz)
        }
        PopconfirmPlacement::Bottom | PopconfirmPlacement::BottomLeft | PopconfirmPlacement::BottomRight => {
            let cx = popup.x + popup.w / 2.0;
            (cx - arrow_sz, popup.y, cx + arrow_sz, popup.y, cx, popup.y - arrow_sz)
        }
    };
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1);
    pb.line_to(x2, y2);
    pb.line_to(x3, y3);
    pb.close();
    ctx.fill_path(&pb.build(), color, FillRule::NonZero);
}
