//! Tooltip widget — 悬浮提示。

use crate::define_widget;
use crate::base::{Rect, Size};
use crate::graphics::{Color, Radius};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};

/// 提示位置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TooltipPlacement { Top, Bottom, Left, Right }

define_widget! {
    /// Tooltip — 鼠标悬浮时显示提示信息。
    pub struct Tooltip {
        text: String,
        placement: TooltipPlacement,
        hovered: bool,
        timer: f32,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        // Tooltip 仅渲染提示文本，不占用布局空间
        Size::new(0.0, 0.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::HoverEnter => {
                self.hovered = true;
                self.timer = 0.0;
                EventResult::Handled
            }
            WidgetEvent::HoverLeave => {
                self.hovered = false;
                self.timer = 0.0;
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    on_update => (&mut self, dt: f32) {
        if self.hovered {
            self.timer += dt;
        }
    }

    needs_continuous_update => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if !self.hovered || self.timer < 0.5 { return; }

        let bg = Color::from_rgba(50, 50, 50, 230);
        let text_color = Color::white();
        let text_w = self.text.len() as f32 * 7.5 + 16.0;
        let text_h = 26.0;
        let r = Some(Radius::uniform(4.0));

        let (tx, ty) = match self.placement {
            TooltipPlacement::Top => (frame.x + frame.w * 0.5 - text_w * 0.5, frame.y - text_h - 6.0),
            TooltipPlacement::Bottom => (frame.x + frame.w * 0.5 - text_w * 0.5, frame.y + frame.h + 6.0),
            TooltipPlacement::Left => (frame.x - text_w - 8.0, frame.y + frame.h * 0.5 - text_h * 0.5),
            TooltipPlacement::Right => (frame.x + frame.w + 8.0, frame.y + frame.h * 0.5 - text_h * 0.5),
        };

        let tip_frame = Rect::new(tx, ty, text_w, text_h);
        ctx.fill_rect(tip_frame, bg, r);
        ctx.text_center(&self.text, tip_frame, text_color, 12.0);
    }

    // 弹窗区域超出 widget frame，需包含在 dirty_rect 中避免被 clip 裁剪
    dirty_rect => (&self, frame: Rect) -> Rect {
        if !self.hovered || self.timer < 0.5 { return frame; }
        let text_w = self.text.len() as f32 * 7.5 + 16.0;
        let text_h = 26.0;
        let (tx, ty) = match self.placement {
            TooltipPlacement::Top => (frame.x + frame.w * 0.5 - text_w * 0.5, frame.y - text_h - 6.0),
            TooltipPlacement::Bottom => (frame.x + frame.w * 0.5 - text_w * 0.5, frame.y + frame.h + 6.0),
            TooltipPlacement::Left => (frame.x - text_w - 8.0, frame.y + frame.h * 0.5 - text_h * 0.5),
            TooltipPlacement::Right => (frame.x + frame.w + 8.0, frame.y + frame.h * 0.5 - text_h * 0.5),
        };
        let tip = Rect::new(tx, ty, text_w, text_h);
        frame.union(&tip)
    }
}

impl Default for Tooltip { fn default() -> Self { Self::new("") } }

impl Tooltip {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), placement: TooltipPlacement::Top, hovered: false, timer: 0.0 }
    }
    pub fn placement(mut self, p: TooltipPlacement) -> Self { self.placement = p; self }
}
