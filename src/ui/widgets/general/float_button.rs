//! FloatButton widget — 浮动按钮，Ant Design 风格。
//!
//! 固定在屏幕角落的圆形按钮，支持图标、tooltip、badge 等。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::SnapshotFields;
use crate::ui::{
    EventResult, KeyCode, MouseButton, OverlayEntry, OverlayKind, SystemEvent, WidgetTree,
};

// FloatButton — 浮动操作按钮。
component! {
    pub struct FloatButton {
        icon: String,
        tooltip: String,
        badge_count: i32,
        size: f32,
        x: f32,
        y: f32,
        reserve_layout_space: bool,
        hovered: bool,
        pressed: bool,
        focused: bool,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    tab_index => (&self) -> i32 { 1 }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        self.button_rect(frame)
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerEnter => { self.hovered = true; EventResult::Handled }
            SystemEvent::PointerLeave => {
                self.hovered = false;
                self.pressed = false;
                EventResult::Handled
            }
            SystemEvent::PointerDown { button: MouseButton::Left, .. } => {
                self.pressed = true;
                EventResult::Handled
            }
            SystemEvent::PointerUp { button: MouseButton::Left, .. } => {
                self.pressed = false;
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.pressed = false;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key: KeyCode::Enter | KeyCode::Space, .. } => {
                self.pressed = true;
                EventResult::Handled
            }
            SystemEvent::KeyUp { key: KeyCode::Enter | KeyCode::Space, .. } => {
                self.pressed = false;
                EventResult::Handled
            }
            _ => EventResult::NotHandled
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        self.paint_bounds(frame)
    }

    overlay_entry => (&self, id: crate::ui::ComponentId, frame: Rect) -> Option<OverlayEntry> {
        if self.reserve_layout_space {
            return None;
        }
        Some(
            OverlayEntry::new(id, OverlayKind::Custom)
                .bounds(self.button_rect(frame))
                .z_index(900),
        )
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let loc = crate::ui::locale::use_locale();
        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let white = Color::white();
        let text_sec = ctx.tokens().color_text_quaternary();
        let bg = if self.pressed {
            ctx.tokens().color_primary_active()
        } else if self.hovered {
            primary_hover
        } else {
            primary
        };
        let r = Radius::uniform(self.size * 0.5);
        let btn_rect = self.button_rect(frame);
        let cx = btn_rect.x + btn_rect.w * 0.5;
        let cy = btn_rect.y + btn_rect.h * 0.5;
        // 阴影
        ctx.draw_box_shadow(btn_rect, 8.0, 0.0, 4.0, Color::from_rgba(0, 0, 0, 40), Some(r));
        ctx.fill_rect(btn_rect, bg, Some(r));
        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(btn_rect, ctx.tokens().color_primary_border(), 2.0, Some(r));
        }
        let icon_fs = 16.0;
        if self.icon == "chevron-up" {
            crate::ui::widgets::general::icon::paint_icon_in_frame(
                ctx, &self.icon, btn_rect, white, icon_fs,
            );
        } else {
            let tw = ctx.measure_text(&self.icon, icon_fs).w;
            let th = ctx.line_box_height(icon_fs);
            ctx.draw_text(
                &self.icon,
                Point::new(
                    btn_rect.x + (btn_rect.w - tw) * 0.5,
                    btn_rect.y + (btn_rect.h - th) * 0.5,
                ),
                white,
                icon_fs,
            );
        }
        // Badge
        if self.badge_count > 0 {
            let badge_count = self.badge_count.to_string();
            let badge = if self.badge_count > 99 { loc.float_badge_overflow } else { &badge_count };
            ctx.fill_circle(cx + self.size * 0.3, cy - self.size * 0.3, 10.0, ctx.tokens().color_error());
            ctx.draw_text(badge, Point::new(cx + self.size * 0.3 - 7.0, cy - self.size * 0.3 - 7.0), white, 10.0);
        }
        if (self.hovered || self.focused) && !self.tooltip.is_empty() {
            let tip = self.tooltip_rect(frame);
            let tip_radius = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
            ctx.fill_rect(tip, ctx.tokens().color_bg_elevated(), tip_radius);
            ctx.stroke_rect(tip, ctx.tokens().color_border_secondary(), 1.0, tip_radius);
            ctx.text_center(&self.tooltip, tip, text_sec, 12.0);
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
            reserve_layout_space: false,
            hovered: false,
            pressed: false,
            focused: false,
        }
    }
    /// 设置相对零布局槽左上角的视觉偏移。
    pub fn position(mut self, x: f32, y: f32) -> Self {
        self.x = finite_or_zero(x);
        self.y = finite_or_zero(y);
        self
    }
    pub fn tooltip(mut self, t: &str) -> Self {
        self.tooltip = t.to_string();
        self
    }
    pub fn badge(mut self, count: i32) -> Self {
        self.badge_count = count.max(0);
        self
    }
    pub fn size(mut self, s: f32) -> Self {
        self.size = positive_or(s, 40.0);
        self
    }

    /// 为按钮锚点保留与直径相同的布局空间；默认浮动模式仍保持零占位。
    pub fn reserve_layout_space(mut self, reserve: bool) -> Self {
        self.reserve_layout_space = reserve;
        self
    }

    fn intrinsic_size(&self) -> Size {
        if self.reserve_layout_space {
            Size::new(self.size, self.size)
        } else {
            Size::zero()
        }
    }

    fn button_rect(&self, frame: Rect) -> Rect {
        Rect::new(frame.x + self.x, frame.y + self.y, self.size, self.size)
    }

    fn tooltip_rect(&self, frame: Rect) -> Rect {
        let button = self.button_rect(frame);
        let width = (self.tooltip.chars().count() as f32 * 7.0 + 20.0).max(44.0);
        Rect::new(
            button.x - width - 8.0,
            button.y + (button.h - 28.0) * 0.5,
            width,
            28.0,
        )
    }

    fn paint_bounds(&self, frame: Rect) -> Rect {
        let button = self.button_rect(frame);
        let shadow = Rect::new(
            button.x - 10.0,
            button.y - 10.0,
            button.w + 20.0,
            button.h + 24.0,
        );
        if self.tooltip.is_empty() {
            shadow
        } else {
            shadow.union(&self.tooltip_rect(frame))
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::FloatButton {
            icon: self.icon.clone(),
            tooltip: self.tooltip.clone(),
            badge_count: self.badge_count,
            size: self.size,
            x: self.x,
            y: self.y,
            reserve_layout_space: self.reserve_layout_space,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.icon = next.icon;
        self.tooltip = next.tooltip;
        self.badge_count = next.badge_count;
        self.size = next.size;
        self.x = next.x;
        self.y = next.y;
        self.reserve_layout_space = next.reserve_layout_space;
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
        FloatButton::new("chevron-up").tooltip("回到顶部")
    }
}

fn positive_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}
