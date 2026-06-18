//! Typography widget — 排版组件（标题/段落/文本）。
//!
//! 支持 h1-h5 标题级别、段落文本、disabled/type 等变体。
//! 与 Label 的区别：Typography 提供语义化排版和更多样式选项。

use crate::base::{Rect, Size};
use crate::define_widget;
use crate::graphics::Color;
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};

/// 排版类型。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TypographyType {
    Heading1,
    Heading2,
    Heading3,
    Heading4,
    Heading5,
    Paragraph,
    Text,
}

/// Typography — 排版组件。
define_widget! {
    pub struct Typography {
        content: String,
        type_: TypographyType,
        disabled: bool,
        mark: bool,
        code: bool,
        underline: bool,
        delete: bool,
        strong: bool,
        italic: bool,
        color_override: Option<Color>,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        let (fs, _fw) = self.compute_font_style();
        let w = self.content.len() as f32 * fs * 0.6;
        let h = fs * 1.5;
        Size::new(w, h)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let (fs, fw) = self.compute_font_style();
        let text_c = self.color_override.unwrap_or_else(|| {
            if self.disabled { ctx.tokens().color_text_quaternary() } else { ctx.tokens().color_text() }
        });
        let mut text = &self.content[..];
        let mut prefix = String::new();
        let mut suffix = String::new();
        if self.mark { prefix.push_str("█"); suffix.push_str("█"); }
        if self.code { prefix.push_str("`"); suffix.push_str("`"); }
        if self.delete { prefix.push_str("~~"); suffix.push_str("~~"); }
        if self.underline { prefix.push_str("_"); suffix.push_str("_"); }
        let display = format!("{prefix}{text}{suffix}");

        let y = frame.y + (frame.h - fs) * 0.5;
        ctx.draw_text(&display, crate::base::Point::new(frame.x + 4.0, y), text_c, fs);
    }
}

impl Typography {
    pub fn new(content: &str, type_: TypographyType) -> Self {
        Self {
            content: content.to_string(),
            type_,
            disabled: false,
            mark: false, code: false, underline: false, delete: false,
            strong: false, italic: false,
            color_override: None,
        }
    }
    /// 创建标题级别（h1-h5）。
    pub fn heading(content: &str, level: u8) -> Self {
        let type_ = match level {
            1 => TypographyType::Heading1,
            2 => TypographyType::Heading2,
            3 => TypographyType::Heading3,
            4 => TypographyType::Heading4,
            _ => TypographyType::Heading5,
        };
        Self::new(content, type_)
    }
    /// 创建段落文本。
    pub fn paragraph(content: &str) -> Self { Self::new(content, TypographyType::Paragraph) }
    /// 创建行内文本。
    pub fn text(content: &str) -> Self { Self::new(content, TypographyType::Text) }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
    pub fn mark(mut self) -> Self { self.mark = true; self }
    pub fn code(mut self) -> Self { self.code = true; self }
    pub fn underline(mut self) -> Self { self.underline = true; self }
    pub fn delete(mut self) -> Self { self.delete = true; self }
    pub fn strong(mut self) -> Self { self.strong = true; self }
    pub fn italic(mut self) -> Self { self.italic = true; self }
    pub fn color(mut self, c: Color) -> Self { self.color_override = Some(c); self }

    fn compute_font_style(&self) -> (f32, f32) {
        match self.type_ {
            TypographyType::Heading1 => (38.0, 600.0),
            TypographyType::Heading2 => (30.0, 600.0),
            TypographyType::Heading3 => (24.0, 600.0),
            TypographyType::Heading4 => (20.0, 600.0),
            TypographyType::Heading5 => (16.0, 600.0),
            TypographyType::Paragraph => (14.0, 400.0),
            TypographyType::Text => (14.0, 400.0),
        }
    }
}
