//! Image widget — 图片组件，Ant Design 风格。
//!
//! 支持占位图、fallback、描述、圆角。

use uix_core::{Point, Rect, Size};
use crate::define_widget;
use uix_graphics::{Color, Radius};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};

/// Image — 图片显示组件。
define_widget! {
    pub struct Image {
        src: String,
        alt: String,
        fallback: String,
        width: f32,
        height: f32,
        radius: f32,
        preview: bool,
        loaded: bool,
        error: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        Size::new(self.width, self.height)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let fill = ctx.tokens().color_fill_tertiary();
        let text_sec = ctx.tokens().color_text_quaternary();
        let r = Some(Radius::uniform(self.radius));

        if self.error || !self.loaded {
            // 占位/错误态：灰色背景 + 图标
            ctx.fill_rect(frame, fill, r);
            ctx.stroke_rect(frame, ctx.tokens().color_border_secondary(), 1.0, r);
            let placeholder = if self.error && !self.fallback.is_empty() {
                &self.fallback
            } else if !self.alt.is_empty() {
                &self.alt
            } else {
                "🖼"
            };
            ctx.text_center(placeholder, frame, text_sec, if self.error { 13.0 } else { 24.0 });
        } else {
            // 已加载：绘制背景色 + 图片（通过 GraphicsEngine 渲染）
            ctx.fill_rect(frame, fill, r);
        }

        // 描述文字（底部）
        if !self.alt.is_empty() && self.loaded {
            ctx.draw_text(&self.alt, Point::new(frame.x + 4.0, frame.y + frame.h + 4.0), text_sec, 11.0);
        }
    }
}

impl Image {
    pub fn new(src: &str, w: f32, h: f32) -> Self {
        Self {
            src: src.to_string(), alt: String::new(), fallback: String::new(),
            width: w, height: h, radius: 6.0,
            preview: true, loaded: false, error: false,
        }
    }
    pub fn alt(mut self, a: &str) -> Self { self.alt = a.to_string(); self }
    pub fn fallback(mut self, f: &str) -> Self { self.fallback = f.to_string(); self }
    pub fn radius(mut self, r: f32) -> Self { self.radius = r; self }
    pub fn preview(mut self, v: bool) -> Self { self.preview = v; self }
    /// 标记为加载成功（由外部加载后调用）。
    pub fn mark_loaded(&mut self) { self.loaded = true; self.error = false; }
    /// 标记为加载失败。
    pub fn mark_error(&mut self) { self.loaded = true; self.error = true; }
}
