//! Form widget — 表单容器与校验。
//!
//! 提供 Form 容器 + FormItem 包装，支持必填标记、标签、校验状态反馈。
//! Form 使用 flex column 布局排列多个 FormItem。

use uix_core::{EdgeInsets, Point, Rect, Size};
use crate::define_widget;
use uix_graphics::{Color, Radius};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};

/// 校验状态。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValidateStatus {
    None,
    Success,
    Warning,
    Error,
    Validating,
}

/// FormItem — 单个表单项（标签 + 内容 + 校验反馈）。
define_widget! {
    pub struct FormItem {
        label: String,
        required: bool,
        status: ValidateStatus,
        help: String,
        label_width: f32,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        // 高度由子内容决定，宽度填满父容器
        Size::new(400.0, 44.0)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let error = ctx.tokens().color_error();
        let warning = ctx.tokens().color_warning();
        let success = ctx.tokens().color_success();

        // 标签 + 必填红星
        if !self.label.is_empty() {
            let label_x = frame.x + 8.0;
            let label_y = frame.y + 4.0;
            if self.required {
                ctx.draw_text("*", Point::new(label_x, label_y), error, 14.0);
                ctx.draw_text(&self.label, Point::new(label_x + 10.0, label_y), text, 14.0);
            } else {
                ctx.draw_text(&self.label, Point::new(label_x, label_y), text, 14.0);
            }
        }

        // 校验反馈颜色 - 左边框指示
        let status_color = match self.status {
            ValidateStatus::Error => error,
            ValidateStatus::Warning => warning,
            ValidateStatus::Success => success,
            _ => Color::transparent(),
        };
        if status_color.a > 0 {
            ctx.fill_rect(Rect::new(frame.x, frame.y, 2.0, frame.h), status_color, None);
        }

        // 帮助/错误文字
        if !self.help.is_empty() {
            let help_color = match self.status {
                ValidateStatus::Error => error,
                ValidateStatus::Warning => warning,
                ValidateStatus::Success => success,
                _ => text_sec,
            };
            ctx.draw_text(&self.help, Point::new(frame.x + 8.0, frame.y + frame.h - 16.0), help_color, 11.0);
        }
    }

    layout_children => (&self, frame: Rect, children: &[crate::widget::WidgetId], _tree: &WidgetTree)
        -> Vec<(crate::widget::WidgetId, Rect)>
    {
        if children.is_empty() { return Vec::new(); }
        let pad = 8.0;
        let label_w = if self.label.is_empty() { 0.0 } else { self.label_width.max(60.0) };
        let content_x = frame.x + label_w + pad;
        let content_w = (frame.w - label_w - pad).max(100.0);
        children.iter().map(|&cid| (cid, Rect::new(content_x, frame.y + 2.0, content_w, frame.h - 18.0))).collect()
    }
}

impl FormItem {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            required: false,
            status: ValidateStatus::None,
            help: String::new(),
            label_width: 80.0,
        }
    }
    pub fn required(mut self, v: bool) -> Self { self.required = v; self }
    pub fn status(mut self, s: ValidateStatus) -> Self { self.status = s; self }
    pub fn help(mut self, h: &str) -> Self { self.help = h.to_string(); self }
    pub fn label_width(mut self, w: f32) -> Self { self.label_width = w; self }
    pub fn get_status(&self) -> ValidateStatus { self.status }
    pub fn set_status(&mut self, s: ValidateStatus) { self.status = s; }
}

/// Form — 表单容器（flex column 排列 FormItem）。
define_widget! {
    pub struct Form {
        label_width: f32,
        gap: f32,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        Size::new(400.0, 200.0)
    }

    render => (&self, _frame: Rect, _ctx: &mut RenderContext, _tree: &WidgetTree) {}

    layout_children => (&self, frame: Rect, children: &[crate::widget::WidgetId], tree: &WidgetTree)
        -> Vec<(crate::widget::WidgetId, Rect)>
    {
        let mut result = Vec::new();
        let mut y = frame.y;
        for &cid in children {
            let pref = tree.get(cid).map(|c| c.preferred_size(None)).unwrap_or(Size::new(frame.w, 44.0));
            let item_h = pref.h.max(44.0);
            result.push((cid, Rect::new(frame.x, y, frame.w, item_h)));
            y += item_h + self.gap;
        }
        result
    }
}

impl Form {
    pub fn new() -> Self { Self { label_width: 80.0, gap: 8.0 } }
    pub fn label_width(mut self, w: f32) -> Self { self.label_width = w; self }
    pub fn gap(mut self, g: f32) -> Self { self.gap = g; self }
}
