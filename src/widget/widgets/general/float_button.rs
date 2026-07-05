//! FloatButton widget — 浮动按钮，Ant Design 风格。
//!
//! 固定在屏幕角落的圆形按钮，支持图标、tooltip、badge 等。

use crate::define_widget;
use crate::widget::scene::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};
use crate::render::{Color, Radius};
use crate::platform::{Point, Rect, Size};

// FloatButton — 浮动操作按钮。
define_widget! {
    pub struct FloatButton {
        icon: String,
        tooltip: String,
        badge_count: i32,
        size: f32,
        x: f32,
        y: f32,
        hovered: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::render::traits::GraphicsEngine>) -> Size {
        Size::zero() // 不占用布局空间
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::HoverEnter => { self.hovered = true; EventResult::Handled }
            WidgetEvent::HoverLeave => { self.hovered = false; EventResult::Handled }
            _ => EventResult::NotHandled
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let loc = crate::widget::locale::use_locale();
        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let white = Color::white();
        let text_sec = ctx.tokens().color_text_quaternary();
        let bg = if self.hovered { primary_hover } else { primary };
        let r = Radius::uniform(self.size * 0.5);
        let cx = frame.x + self.x;
        let cy = frame.y + self.y;
        let btn_rect = Rect::new(cx - self.size * 0.5, cy - self.size * 0.5, self.size, self.size);
        // 阴影
        ctx.draw_box_shadow(btn_rect, 8.0, 0.0, 4.0, Color::from_rgba(0, 0, 0, 40), Some(r));
        ctx.fill_rect(btn_rect, bg, Some(r));
        ctx.text_center(&self.icon, btn_rect, white, 16.0);
        // Badge
        if self.badge_count > 0 {
            let badge = if self.badge_count > 99 { loc.float_badge_overflow } else { &self.badge_count.to_string() };
            ctx.fill_circle(cx + self.size * 0.3, cy - self.size * 0.3, 10.0, ctx.tokens().color_error());
            ctx.draw_text(badge, Point::new(cx + self.size * 0.3 - 7.0, cy - self.size * 0.3 - 7.0), white, 10.0);
        }
        // Tooltip（简化 hover 时显示）
        if self.hovered && !self.tooltip.is_empty() {
            ctx.draw_text(&self.tooltip, Point::new(cx - 100.0, cy - self.size * 0.5 - 18.0), text_sec, 12.0);
        }
    }
}

impl FloatButton {
    pub fn new(icon: &str) -> Self {
        Self {
            icon: icon.to_string(),
            tooltip: String::new(),
            badge_count: 0,
            size: 40.0,
            x: 0.0,
            y: 0.0,
            hovered: false,
        }
    }
    pub fn position(mut self, x: f32, y: f32) -> Self {
        self.x = x;
        self.y = y;
        self
    }
    pub fn tooltip(mut self, t: &str) -> Self {
        self.tooltip = t.to_string();
        self
    }
    pub fn badge(mut self, count: i32) -> Self {
        self.badge_count = count;
        self
    }
    pub fn size(mut self, s: f32) -> Self {
        self.size = s;
        self
    }
}

impl Default for FloatButton {
    fn default() -> Self {
        Self::new("+")
    }
}

/// FloatButtonBackTop — 回到顶部按钮（FloatButton 的便捷封装）。
pub struct FloatButtonBackTop;

impl FloatButtonBackTop {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> FloatButton {
        FloatButton::new("↑").tooltip("回到顶部").position(0.0, 0.0)
    }
}
