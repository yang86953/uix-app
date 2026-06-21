//! Layout 页面布局组件 — Header / Sider / Content / Footer 骨架。
//!
//! 组合使用构建标准页面布局。

use uix_core::{Rect, Size};
use crate::define_widget;
use uix_graphics::{Color, GraphicsEngine};
use crate::render_context::RenderContext;
use crate::widget::WidgetTree;

define_widget! {
    /// Layout — 页面布局容器（flex 列）。
    pub struct Layout {
        bg_color: Option<Color>,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(300.0, 200.0)
    }

    flex_grow => (&self) -> f32 { 1.0 }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
    }
}

define_widget! {
    /// Header — 页面顶部栏。
    pub struct Header {
        height: f32,
        bg_color: Option<Color>,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(0.0, self.height)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
    }
}

define_widget! {
    /// Sider — 侧边栏。
    pub struct Sider {
        width: f32,
        bg_color: Option<Color>,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(self.width, 0.0)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
    }
}

define_widget! {
    /// Content — 内容区。
    pub struct Content {
        bg_color: Option<Color>,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(0.0, 0.0)
    }

    flex_grow => (&self) -> f32 { 1.0 }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
    }
}

define_widget! {
    /// Footer — 页面底部栏。
    pub struct Footer {
        height: f32,
        bg_color: Option<Color>,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(0.0, self.height)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
    }
}

// 构造方法
impl Layout {
    pub fn new() -> Self { Self { bg_color: None } }
    pub fn bg(mut self, c: Color) -> Self { self.bg_color = Some(c); self }
}

impl Header {
    pub fn new(height: f32) -> Self { Self { height, bg_color: None } }
    pub fn bg(mut self, c: Color) -> Self { self.bg_color = Some(c); self }
}

impl Sider {
    pub fn new(width: f32) -> Self { Self { width, bg_color: None } }
    pub fn bg(mut self, c: Color) -> Self { self.bg_color = Some(c); self }
}

impl Content {
    pub fn new() -> Self { Self { bg_color: None } }
    pub fn bg(mut self, c: Color) -> Self { self.bg_color = Some(c); self }
}

impl Footer {
    pub fn new(height: f32) -> Self { Self { height, bg_color: None } }
    pub fn bg(mut self, c: Color) -> Self { self.bg_color = Some(c); self }
}
