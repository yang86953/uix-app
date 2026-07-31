//! Badge — 徽章组件。
//!
//! 支持物理单位：`offset()` 接受 mm/cm/pt，自动适配 DPI。

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::geometry::spatial::PhysicalUnit;
use crate::draw::{Color, FillRule, PathBuilder, Radius};
use crate::ui::core::paint_context::PaintContext;
use crate::ui::core::widget::WidgetTree;
use crate::ui::SnapshotFields;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BadgeStatus {
    Success,
    Processing,
    Default,
    Error,
    Warning,
}

/// 预设徽章颜色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BadgeColor {
    Blue,
    Green,
    Orange,
    Red,
    Purple,
}

impl BadgeColor {
    /// 映射为对应的 `Color`。
    pub fn to_color(self) -> Color {
        match self {
            BadgeColor::Blue => Color::hex("#1677ff"),
            BadgeColor::Green => Color::hex("#52c41a"),
            BadgeColor::Orange => Color::hex("#fa8c16"),
            BadgeColor::Red => Color::hex("#f5222d"),
            BadgeColor::Purple => Color::hex("#722ed1"),
        }
    }
}

/// 可用于 [`Badge::color`] 的颜色输入。
pub trait IntoBadgeColor {
    fn into_badge_color(self) -> (Color, bool);
}

impl IntoBadgeColor for Color {
    fn into_badge_color(self) -> (Color, bool) {
        (self, false)
    }
}

impl IntoBadgeColor for BadgeColor {
    fn into_badge_color(self) -> (Color, bool) {
        (self.to_color(), true)
    }
}

fn finite_badge_offset(value: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

component! {
    pub struct Badge {
        count: i32,
        max: i32,
        dot: bool,
        color: Option<Color>,
        adaptive_foreground: bool,
        ribbon: bool,
        status: Option<BadgeStatus>,
        show_zero: bool,
        text: String,

        // ── 2D 偏移（f32 像素）──
        offset_x: f32,
        offset_y: f32,

        // ── 物理单位偏移（优先级高于 offset_x/y）──
        offset_unit: Option<(PhysicalUnit, PhysicalUnit)>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        // 计算实际偏移：物理单位优先
        let (off_x, off_y) = if let Some((ux, uy)) = self.offset_unit {
            let dpi = ctx.dpi();
            (ux.to_dip(dpi), uy.to_dip(dpi))
        } else {
            (self.offset_x, self.offset_y)
        };
        let (off_x, off_y) = (finite_badge_offset(off_x), finite_badge_offset(off_y));
        let actual_frame = Rect::new(frame.x + off_x, frame.y + off_y, frame.w, frame.h);

        if let Some(status) = self.status {
            let marker_color = match status {
                BadgeStatus::Success => ctx.tokens().color_success(),
                BadgeStatus::Processing => ctx.tokens().color_primary(),
                BadgeStatus::Default => ctx.tokens().color_text_quaternary(),
                BadgeStatus::Error => ctx.tokens().color_error(),
                BadgeStatus::Warning => ctx.tokens().color_warning(),
            };
            self.render_marker_label(ctx, actual_frame, marker_color);
            return;
        }

        let bg = self.color.unwrap_or(ctx.tokens().color_error());
        let foreground = if self.adaptive_foreground && bg.is_light() {
            Color::black()
        } else {
            Color::white()
        };
        if self.ribbon {
            self.render_ribbon(ctx, actual_frame, bg, foreground);
            return;
        }
        if self.dot {
            self.render_marker_label(ctx, actual_frame, bg);
            return;
        }
        if self.count == 0 && !self.show_zero && self.text.is_empty() { return; }
        if !self.text.is_empty() {
            let r = Some(Radius::uniform(actual_frame.h * 0.5));
            ctx.fill_rect(actual_frame, bg, r);
            let fs = Self::PILL_FONT_SIZE;
            let tw = ctx.measure_text(&self.text, fs).w;
            let th = ctx.line_box_height(fs);
            ctx.draw_text(
                &self.text,
                crate::core::Point::new(
                    actual_frame.x + (actual_frame.w - tw) * 0.5,
                    actual_frame.y + (actual_frame.h - th) * 0.5,
                ),
                foreground,
                fs,
            );
        } else {
            let r = Some(Radius::uniform(actual_frame.h * 0.5));
            ctx.fill_rect(actual_frame, bg, r);
            let label = self.count_label();
            let fs = Self::PILL_FONT_SIZE;
            let tw = ctx.measure_text(&label, fs).w;
            let th = ctx.line_box_height(fs);
            ctx.draw_text(
                &label,
                crate::core::Point::new(
                    actual_frame.x + (actual_frame.w - tw) * 0.5,
                    actual_frame.y + (actual_frame.h - th) * 0.5,
                ),
                foreground,
                fs,
            );
        }
    }
}

impl Default for Badge {
    fn default() -> Self {
        Self::new()
    }
}

impl Badge {
    const MARKER_DIAMETER: f32 = 10.0;
    const MARKER_TEXT_GAP: f32 = 8.0;
    const PILL_HEIGHT: f32 = 20.0;
    const PILL_FONT_SIZE: f32 = 11.0;
    const MARKER_LABEL_FONT_SIZE: f32 = 13.0;
    const TEXT_HORIZONTAL_PADDING: f32 = 12.0;
    const RIBBON_HEIGHT: f32 = 24.0;
    const RIBBON_HORIZONTAL_PADDING: f32 = 24.0;

    fn intrinsic_size(&self) -> Size {
        if self.dot || self.status.is_some() {
            if self.text.is_empty() {
                Size::new(Self::MARKER_DIAMETER, Self::MARKER_DIAMETER)
            } else {
                Size::new(
                    Self::MARKER_DIAMETER
                        + Self::MARKER_TEXT_GAP
                        + Self::estimated_text_width(&self.text, Self::MARKER_LABEL_FONT_SIZE),
                    Self::PILL_HEIGHT,
                )
            }
        } else if self.ribbon {
            let w = Self::estimated_text_width(&self.text, Self::PILL_FONT_SIZE)
                + Self::RIBBON_HORIZONTAL_PADDING;
            Size::new(w.max(Self::RIBBON_HEIGHT), Self::RIBBON_HEIGHT)
        } else if !self.text.is_empty() {
            let w = Self::estimated_text_width(&self.text, Self::PILL_FONT_SIZE)
                + Self::TEXT_HORIZONTAL_PADDING;
            Size::new(w.max(Self::PILL_HEIGHT), Self::PILL_HEIGHT)
        } else if self.count > 0 || (self.count == 0 && self.show_zero) {
            let w = Self::estimated_text_width(&self.count_label(), Self::PILL_FONT_SIZE)
                + Self::TEXT_HORIZONTAL_PADDING;
            Size::new(w.max(Self::PILL_HEIGHT), Self::PILL_HEIGHT)
        } else {
            Size::zero()
        }
    }

    pub fn new() -> Self {
        Self {
            count: 0,
            max: 99,
            dot: false,
            color: None,
            adaptive_foreground: false,
            ribbon: false,
            status: None,
            show_zero: false,
            text: String::new(),
            offset_x: 0.0,
            offset_y: 0.0,
            offset_unit: None,
        }
    }
    pub fn count(mut self, n: i32) -> Self {
        self.count = n.max(0);
        self
    }
    pub fn max(mut self, n: i32) -> Self {
        self.max = n.max(1);
        self
    }
    pub fn dot(mut self) -> Self {
        self.dot = true;
        self.count = 1;
        self
    }
    pub fn color(mut self, c: impl IntoBadgeColor) -> Self {
        let (color, adaptive_foreground) = c.into_badge_color();
        self.color = Some(color);
        self.adaptive_foreground = adaptive_foreground;
        self
    }

    /// 使用预设颜色变体设置徽章颜色。
    pub fn preset_color(self, c: BadgeColor) -> Self {
        self.color(c)
    }

    /// 创建角标丝带（绝对定位，不参与父级正常布局流）。
    pub fn ribbon(text: impl Into<String>, color: BadgeColor) -> Self {
        let text = text.into();
        let mut badge = Self::new().color(color).text(&text);
        badge.ribbon = true;
        badge
    }
    pub fn status(mut self, s: BadgeStatus) -> Self {
        self.status = Some(s);
        self
    }
    pub fn show_zero(mut self, v: bool) -> Self {
        self.show_zero = v;
        self
    }
    pub fn text(mut self, t: &str) -> Self {
        self.text = t.to_string();
        self
    }

    /// 像素偏移。
    pub fn offset(mut self, x: f32, y: f32) -> Self {
        self.offset_x = finite_badge_offset(x);
        self.offset_y = finite_badge_offset(y);
        self
    }

    /// 物理单位偏移（优先级高于 `offset()`）。
    /// 自动适配 DPI：`10.mm()` 在不同屏幕上物理尺寸一致。
    pub fn offset_unit(mut self, x: PhysicalUnit, y: PhysicalUnit) -> Self {
        self.offset_unit = Some((x, y));
        self
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.count = next.count.max(0);
        self.max = next.max.max(1);
        self.dot = next.dot;
        self.color = next.color;
        self.adaptive_foreground = next.adaptive_foreground;
        self.ribbon = next.ribbon;
        self.status = next.status;
        self.show_zero = next.show_zero;
        self.text = next.text;
        self.offset_x = finite_badge_offset(next.offset_x);
        self.offset_y = finite_badge_offset(next.offset_y);
        self.offset_unit = next.offset_unit;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Badge {
            count: self.count,
            max: self.max,
            dot: self.dot,
            color: self.color,
            adaptive_foreground: self.adaptive_foreground,
            ribbon: self.ribbon,
            size: self.intrinsic_size().h,
            status: self.status,
            show_zero: self.show_zero,
            text: self.text.clone(),
            offset_x: self.offset_x,
            offset_y: self.offset_y,
            offset_unit: self.offset_unit,
        }
    }

    fn render_marker_label(&self, ctx: &mut PaintContext, frame: Rect, marker_color: Color) {
        let radius = Self::MARKER_DIAMETER * 0.5;
        let center_y = frame.y + frame.h * 0.5;
        ctx.fill_circle(frame.x + radius, center_y, radius, marker_color);
        if !self.text.is_empty() {
            let font_size = Self::MARKER_LABEL_FONT_SIZE;
            let text_y = ctx.visual_center_y(frame, font_size);
            ctx.draw_text(
                &self.text,
                crate::core::Point::new(
                    frame.x + Self::MARKER_DIAMETER + Self::MARKER_TEXT_GAP,
                    text_y,
                ),
                ctx.tokens().color_text(),
                font_size,
            );
        }
    }

    fn render_ribbon(
        &self,
        ctx: &mut PaintContext,
        frame: Rect,
        background: Color,
        foreground: Color,
    ) {
        let slant = (frame.h * 0.22).min(frame.w * 0.2);
        let mut path = PathBuilder::new();
        path.move_to(frame.x + slant, frame.y)
            .line_to(frame.x + frame.w, frame.y)
            .line_to(frame.x + frame.w - slant, frame.y + frame.h)
            .line_to(frame.x, frame.y + frame.h)
            .close();
        ctx.fill_path(&path.build(), background, FillRule::NonZero);

        let font_size = Self::PILL_FONT_SIZE;
        let text_width = ctx.measure_text(&self.text, font_size).w;
        let text_height = ctx.line_box_height(font_size);
        ctx.draw_text(
            &self.text,
            crate::core::Point::new(
                frame.x + (frame.w - text_width) * 0.5,
                frame.y + (frame.h - text_height) * 0.5,
            ),
            foreground,
            font_size,
        );
    }

    fn count_label(&self) -> String {
        format!(
            "{}{}",
            self.count.min(self.max),
            if self.count > self.max { "+" } else { "" }
        )
    }

    fn estimated_text_width(text: &str, font_size: f32) -> f32 {
        crate::draw::resources::font::text_backend::estimate_text_metrics(
            text,
            f32::INFINITY,
            font_size,
        )
        .max_line_width
    }
}
