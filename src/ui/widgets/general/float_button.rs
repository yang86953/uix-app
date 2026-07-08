//! FloatButton widget — 浮动按钮，Ant Design 风格。
//!
//! 固定在屏幕角落的圆形按钮，支持图标、tooltip、badge 等。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::SnapshotFields;
use crate::ui::{EventResult, SystemEvent, WidgetTree};

// FloatButton — 浮动操作按钮。
component! {
    pub struct FloatButton {
        icon: String,
        tooltip: String,
        badge_count: i32,
        size: f32,
        x: f32,
        y: f32,
        hovered: bool,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerEnter => { self.hovered = true; EventResult::Handled }
            SystemEvent::PointerLeave => { self.hovered = false; EventResult::Handled }
            _ => EventResult::NotHandled
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let loc = crate::ui::locale::use_locale();
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

    fn intrinsic_size(&self) -> Size {
        Size::zero() // 不占用布局空间
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::FloatButton {
            icon: self.icon.clone(),
            tooltip: self.tooltip.clone(),
            badge_count: self.badge_count,
            size: self.size,
            x: self.x,
            y: self.y,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.icon = next.icon;
        self.tooltip = next.tooltip;
        self.badge_count = next.badge_count;
        self.size = next.size;
        self.x = next.x;
        self.y = next.y;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::traits::WidgetLayout;

    #[test]
    fn measure_preserves_float_button_zero_layout_footprint() {
        let measured = FloatButton::new("+").measure(Constraints::loose(Size::new(40.0, 40.0)));

        assert_eq!(measured, Size::zero());
    }
}
