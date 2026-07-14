//! Badge — 徽章组件。
//!
//! 支持物理单位：`offset()` 接受 mm/cm/pt，自动适配 DPI。

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::spatial::PhysicalUnit;
use crate::draw::{Color, Radius};
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

component! {
    pub struct Badge {
        count: i32,
        max: i32,
        dot: bool,
        color: Option<Color>,
        _size: f32,
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

    picture_policy => (&self) -> crate::draw::compositor::PicturePolicy {
        crate::draw::compositor::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        // 计算实际偏移：物理单位优先
        let (off_x, off_y) = if let Some((ux, uy)) = self.offset_unit {
            let dpi = ctx.dpi();
            (ux.to_dip(dpi), uy.to_dip(dpi))
        } else {
            (self.offset_x, self.offset_y)
        };
        let actual_frame = Rect::new(frame.x + off_x, frame.y + off_y, frame.w, frame.h);

        if let Some(status) = self.status {
            let sc = match status {
                BadgeStatus::Success => ctx.tokens().color_success(),
                BadgeStatus::Processing => ctx.tokens().color_primary(),
                BadgeStatus::Default => ctx.tokens().color_text_quaternary(),
                BadgeStatus::Error => ctx.tokens().color_error(),
                BadgeStatus::Warning => ctx.tokens().color_warning(),
            };
            let r = actual_frame.h * 0.5;
            ctx.fill_circle(actual_frame.x + r, actual_frame.y + r, r, sc);
            return;
        }

        if self.count == 0 && !self.show_zero && self.text.is_empty() { return; }
        let bg = self.color.unwrap_or(ctx.tokens().color_error());
        let display = self.count.min(self.max);
        if self.dot {
            let r = actual_frame.h * 0.5;
            ctx.fill_circle(actual_frame.x + r, actual_frame.y + r, r, bg);
        } else if !self.text.is_empty() {
            let r = Some(Radius::uniform(actual_frame.h * 0.5));
            ctx.fill_rect(actual_frame, bg, r);
            let fs = 11.0;
            let tw = ctx.measure_text(&self.text, fs).w;
            let th = ctx.line_box_height(fs);
            ctx.draw_text(
                &self.text,
                crate::core::Point::new(
                    actual_frame.x + (actual_frame.w - tw) * 0.5,
                    actual_frame.y + (actual_frame.h - th) * 0.5,
                ),
                Color::white(),
                fs,
            );
        } else {
            let r = Some(Radius::uniform(actual_frame.h * 0.5));
            ctx.fill_rect(actual_frame, bg, r);
            let text = format!("{}", display);
            let over = if self.count > self.max { "+" } else { "" };
            let label = format!("{}{}", text, over);
            let fs = 11.0;
            let tw = ctx.measure_text(&label, fs).w;
            let th = ctx.line_box_height(fs);
            ctx.draw_text(
                &label,
                crate::core::Point::new(
                    actual_frame.x + (actual_frame.w - tw) * 0.5,
                    actual_frame.y + (actual_frame.h - th) * 0.5,
                ),
                Color::white(),
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
    fn intrinsic_size(&self) -> Size {
        if self.dot || self.status.is_some() {
            Size::new(10.0, 10.0)
        } else if self.count > 0 || (self.count == 0 && self.show_zero) {
            let text = format!("{}", self.count.min(self.max));
            let w = (text.len() as f32) * 7.0 + 12.0;
            Size::new(w.max(20.0), 20.0)
        } else if !self.text.is_empty() {
            let w = self.text.len() as f32 * 7.0 + 12.0;
            Size::new(w, 20.0)
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
            _size: 16.0,
            status: None,
            show_zero: false,
            text: String::new(),
            offset_x: 0.0,
            offset_y: 0.0,
            offset_unit: None,
        }
    }
    pub fn count(mut self, n: i32) -> Self {
        self.count = n;
        self
    }
    pub fn max(mut self, n: i32) -> Self {
        self.max = n;
        self
    }
    pub fn dot(mut self) -> Self {
        self.dot = true;
        self.count = 1;
        self
    }
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
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
        self.offset_x = x;
        self.offset_y = y;
        self
    }

    /// 物理单位偏移（优先级高于 `offset()`）。
    /// 自动适配 DPI：`10.mm()` 在不同屏幕上物理尺寸一致。
    pub fn offset_unit(mut self, x: PhysicalUnit, y: PhysicalUnit) -> Self {
        self.offset_unit = Some((x, y));
        self
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.count = next.count;
        self.max = next.max;
        self.dot = next.dot;
        self.color = next.color;
        self._size = next._size;
        self.status = next.status;
        self.show_zero = next.show_zero;
        self.text = next.text;
        self.offset_x = next.offset_x;
        self.offset_y = next.offset_y;
        self.offset_unit = next.offset_unit;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Badge {
            count: self.count,
            max: self.max,
            dot: self.dot,
            color: self.color,
            size: self._size,
            status: self.status,
            show_zero: self.show_zero,
            text: self.text.clone(),
            offset_x: self.offset_x,
            offset_y: self.offset_y,
            offset_unit: self.offset_unit,
        }
    }
}
