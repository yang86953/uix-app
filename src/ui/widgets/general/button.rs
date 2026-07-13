//! Button — 纯文本按钮，外观由 StyleSet 预设驱动；点击反馈为 Material 风格 ripple。

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::impl_widget_component;
use crate::native::traits::input::KeyCode;
use crate::ui::animation::{Animation, Easing};
use crate::ui::style::{apply_style, ColorValue, PaletteColor, Style, StyleSet, StyleState};
use crate::ui::traits::{EventHandler, WidgetAnimation, WidgetLayout, WidgetRender};
use crate::ui::{EventResult, SystemEvent, WidgetTree};
use crate::ui::{SnapshotFields, SnapshotSource};

/// Material 风格水波纹：从触点扩大到盖住按钮，松手后淡出收束。
struct ButtonRipple {
    /// 按钮局部坐标原点（相对 frame 左上角）。
    origin: Point,
    expand: Animation<f32>,
    /// 按住时保持 1；松手后 1→0 淡出。
    fade: Animation<f32>,
    held: bool,
}

impl ButtonRipple {
    const EXPAND_SECS: f64 = 0.32;
    const FADE_SECS: f64 = 0.2;

    fn start(origin: Point) -> Self {
        Self {
            origin,
            expand: Animation::new(0.0, 1.0, Self::EXPAND_SECS).with_easing(Easing::CubicOut),
            fade: Animation::new(1.0, 1.0, 0.0),
            held: true,
        }
    }

    fn release(&mut self) {
        if !self.held {
            return;
        }
        self.held = false;
        let current = self.fade.current_value();
        self.fade = Animation::new(current, 0.0, Self::FADE_SECS).with_easing(Easing::QuadOut);
    }

    fn update(&mut self, dt: f64) -> bool {
        self.expand.update(dt);
        self.fade.update(dt);
        if self.held {
            // 扩到满后静止绘制，无需空转帧（#105）。
            !self.expand.is_finished()
        } else {
            !self.expand.is_finished() || !self.fade.is_finished()
        }
    }

    fn is_visible(&self) -> bool {
        self.held || !self.fade.is_finished()
    }

    fn opacity(&self) -> f32 {
        self.fade.current_value().clamp(0.0, 1.0)
    }
}

fn cover_radius(origin: Point, size: Size) -> f32 {
    let corners = [
        (0.0, 0.0),
        (size.w, 0.0),
        (0.0, size.h),
        (size.w, size.h),
    ];
    corners
        .into_iter()
        .map(|(x, y)| {
            let dx = x - origin.x;
            let dy = y - origin.y;
            (dx * dx + dy * dy).sqrt()
        })
        .fold(0.0_f32, f32::max)
}

/// 按钮组件。业务绑定不存放在组件内，由 HandlerTable 按 ComponentId 管理。
pub struct Button {
    text: String,
    disabled: bool,
    block: bool,
    hovered: bool,
    pressed: bool,
    focused: bool,
    ripple: Option<ButtonRipple>,
    /// 上一帧 `update_animation` 是否推进了 ripple（供窄标脏）。
    ripple_dirty: bool,
    pub(crate) style_set: StyleSet,
    pub(crate) style: Style,
}

impl_widget_component!(Button; Layout, Render, Event, Animation; tab_index => 1);

impl SnapshotSource for Button {
    fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Button {
            text: self.text.clone(),
            disabled: self.disabled,
            block: self.block,
            style_set: self.style_set.clone(),
            style: self.style.clone(),
        }
    }
}

impl WidgetLayout for Button {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    fn flex_grow(&self) -> f32 {
        self.style.flex_grow
    }

    fn flex_shrink(&self) -> f32 {
        self.style.flex_shrink
    }

    fn layout_margin(&self) -> crate::core::EdgeInsets {
        self.style.margin
    }

    fn align_self(&self) -> Option<crate::ui::layout::AlignItems> {
        self.style.align_self
    }

    fn grid_cell(&self) -> Option<usize> {
        self.style.grid_cell
    }

    fn grid_column_span(&self) -> u32 {
        self.style.grid_column_span
    }

    fn grid_row_span(&self) -> u32 {
        self.style.grid_row_span
    }
}

impl EventHandler for Button {
    fn on_event(&mut self, event: &SystemEvent) -> EventResult {
        if self.disabled {
            return EventResult::NotHandled;
        }

        match event {
            SystemEvent::PointerDown { pos, .. } => {
                self.pressed = true;
                self.ripple = Some(ButtonRipple::start(*pos));
                EventResult::Handled
            }
            SystemEvent::PointerUp { .. } => {
                self.pressed = false;
                if let Some(ripple) = self.ripple.as_mut() {
                    ripple.release();
                }
                EventResult::Handled
            }
            SystemEvent::PointerEnter => {
                self.hovered = true;
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                self.hovered = false;
                self.pressed = false;
                if let Some(ripple) = self.ripple.as_mut() {
                    ripple.release();
                }
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.pressed = false;
                if let Some(ripple) = self.ripple.as_mut() {
                    ripple.release();
                }
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } if matches!(*key, KeyCode::Enter | KeyCode::Space) => {
                self.pressed = true;
                // 键盘激活：哨兵原点 → 绘制时取按钮中心。
                self.ripple = Some(ButtonRipple::start(Self::CENTER_ORIGIN));
                EventResult::Handled
            }
            SystemEvent::KeyUp { key, .. } if matches!(*key, KeyCode::Enter | KeyCode::Space) => {
                self.pressed = false;
                if let Some(ripple) = self.ripple.as_mut() {
                    ripple.release();
                }
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }
}

impl WidgetRender for Button {
    fn render(&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let style = self.resolve_style();
        apply_style(ctx, frame, &style);
        self.paint_ripple(frame, ctx, &style);
        if !self.text.is_empty() {
            let content = frame.inset(style.padding);
            let font_size = style.resolve_font_size(ctx.tokens());
            let color = style.resolve_color(ctx.tokens());
            // 布局职责：在 content 内交叉轴居中行盒；绘制只顶对齐 blit。
            let text_w = ctx.measure_text(&self.text, font_size).w;
            let text_h = ctx.line_box_height(font_size);
            let text_rect = Rect::new(
                content.x + (content.w - text_w) * 0.5,
                content.y + (content.h - text_h) * 0.5,
                text_w.max(0.0),
                text_h.max(0.0),
            );
            ctx.draw_text(
                &self.text,
                Point::new(text_rect.x, text_rect.y),
                color,
                font_size,
            );
        }
    }

    fn dirty_rect(&self, frame: Rect) -> Rect {
        frame
    }
}

impl WidgetAnimation for Button {
    fn update_animation(&mut self, dt: f64) -> bool {
        let Some(ripple) = self.ripple.as_mut() else {
            self.ripple_dirty = false;
            return false;
        };
        let before_expand = ripple.expand.current_value();
        let before_fade = ripple.fade.current_value();
        let active = ripple.update(dt);
        self.ripple_dirty = (ripple.expand.current_value() - before_expand).abs() > f32::EPSILON
            || (ripple.fade.current_value() - before_fade).abs() > f32::EPSILON;
        if !ripple.is_visible() {
            self.ripple = None;
            self.ripple_dirty = true;
            return false;
        }
        active
    }

    fn dirty_bounds(&self, frame: Rect) -> Rect {
        if self.ripple_dirty {
            frame
        } else {
            Rect::zero()
        }
    }
}

impl Button {
    /// 键盘激活用的中心原点哨兵（局部坐标不可能为负）。
    const CENTER_ORIGIN: Point = Point::new(-1.0, -1.0);

    pub fn new(text: impl Into<String>) -> Self {
        Self::assemble(text.into(), StyleSet::button_default(), false, false)
    }

    pub(crate) fn assemble(text: String, style_set: StyleSet, disabled: bool, block: bool) -> Self {
        Self {
            text,
            disabled,
            block,
            hovered: false,
            pressed: false,
            focused: false,
            ripple: None,
            ripple_dirty: false,
            style_set,
            style: Style::default(),
        }
    }

    pub fn style_set(mut self, style_set: StyleSet) -> Self {
        self.style_set = style_set;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn text(&self) -> &str {
        self.text.as_str()
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.text = next.text;
        self.disabled = next.disabled;
        self.block = next.block;
        self.style_set = next.style_set;
        self.style = next.style;
    }

    pub fn primary(self) -> Self {
        self.style_set(StyleSet::button_primary())
    }

    pub fn ghost(self) -> Self {
        self.style_set(StyleSet::button_ghost())
    }

    pub fn danger(self) -> Self {
        self.style_set(StyleSet::button_danger())
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn block(mut self, block: bool) -> Self {
        self.block = block;
        self
    }

    fn resolve_style(&self) -> Style {
        self.style_set
            .resolve(StyleState {
                hovered: self.hovered,
                pressed: self.pressed,
                // focused/pressed 预设无色变（#176）；仍传 flags 供自定义 StyleSet。
                focused: self.focused && !self.pressed,
                disabled: self.disabled,
            })
            .apply(self.style.clone())
    }

    fn intrinsic_size(&self) -> Size {
        let base = self.style_set.normal.clone().apply(self.style.clone());
        let font_size = base.font_size.default_size().max(1.0);
        // 按钮外框高度由 Style 固定；文字行盒在 render 时于 content 内居中。
        let height = base.height.unwrap_or(32.0);
        // 无 FontService 时用字符估算宽；真实宽在 paint 用 measure_text。
        let text_w = self.text.chars().count() as f32 * font_size * 0.55;
        let width = base
            .width
            .unwrap_or(text_w + base.padding.horizontal())
            .max(32.0);
        if self.block {
            Size::new(f32::MAX, height)
        } else {
            Size::new(width, height)
        }
    }

    fn paint_ripple(&self, frame: Rect, ctx: &mut PaintContext<'_>, style: &Style) {
        let Some(ripple) = self.ripple.as_ref() else {
            return;
        };
        if !ripple.is_visible() {
            return;
        }

        let size = Size::new(frame.w, frame.h);
        let local = if ripple.origin.x < 0.0 || ripple.origin.y < 0.0 {
            Point::new(size.w * 0.5, size.h * 0.5)
        } else {
            ripple.origin
        };
        let max_r = cover_radius(local, size);
        let radius = max_r * ripple.expand.current_value();
        if radius <= 0.0 {
            return;
        }

        let ink = Self::ripple_ink_color(style, ctx);
        let alpha = (ink.a as f32 * ripple.opacity()).round() as u8;
        if alpha == 0 {
            return;
        }

        ctx.push_clip(frame);
        ctx.fill_circle(
            frame.x + local.x,
            frame.y + local.y,
            radius,
            ink.with_alpha(alpha),
        );
        ctx.pop_clip();
    }

    fn ripple_ink_color(style: &Style, ctx: &PaintContext<'_>) -> Color {
        // 实心强调色按钮用浅色波；描边/浅底用深色波。
        let filled_dark = match style.background {
            Some(ColorValue::Palette(PaletteColor::Primary))
            | Some(ColorValue::Palette(PaletteColor::PrimaryHover))
            | Some(ColorValue::Palette(PaletteColor::PrimaryActive))
            | Some(ColorValue::Palette(PaletteColor::Error))
            | Some(ColorValue::Palette(PaletteColor::ErrorBorder)) => true,
            Some(bg) => {
                let c = bg.resolve(ctx.tokens());
                c.a > 200 && !c.is_light()
            }
            None => false,
        };
        if filled_dark {
            Color::white().with_alpha(56)
        } else {
            Color::black().with_alpha(36)
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/ui/widgets/general/button.rs"]
mod tests;
