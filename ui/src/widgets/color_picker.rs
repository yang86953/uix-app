//! ColorPicker widget — 颜色选择器，Ant Design 风格。
//!
//! 预设色板选择，点击触发弹出面板。

use uix_core::{Point, Rect, Size};
use crate::define_widget;
use uix_graphics::{Color, Radius};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};

const PRESET_COLORS: &[u32] = &[
    0xF52222, 0xFA541C, 0xFA8C16, 0xFADB14, 0x52C41A, 0x13C2C2, 0x1677FF, 0x2F54EB,
    0x722ED1, 0xEB2F96, 0xFF85C0, 0xFFEC3D, 0x95DE64, 0x5CDBD3, 0x85A5FF, 0xB37FEB,
    0xF0F0F0, 0xD9D9D9, 0xBFBFBF, 0x8C8C8C, 0x434343, 0x262626, 0x1F1F1F, 0x141414,
];

/// ColorPicker — 颜色选择器。
define_widget! {
    pub struct ColorPicker {
        value: Color,
        open: bool,
        preset_colors: Vec<Color>,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        Size::new(32.0, 32.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if let WidgetEvent::MouseDown { pos, .. } = event {
            if pos.y >= 0.0 && pos.y <= 32.0 {
                self.open = !self.open; return EventResult::Handled;
            }
            if self.open && pos.y > 32.0 {
                let cols = 8;
                let cell = 24.0;
                let pad = 8.0;
                let panel_x = pos.x - 0.0; // 相对面板
                let panel_y = pos.y - 40.0;
                if panel_x >= pad && panel_y >= pad {
                    let ci = ((panel_x - pad) / cell) as usize;
                    let ri = ((panel_y - pad) / cell) as usize;
                    let idx = ri * cols + ci;
                    if idx < self.preset_colors.len() {
                        self.value = self.preset_colors[idx];
                        self.open = false;
                        return EventResult::Handled;
                    }
                }
                self.open = false;
            }
        }
        EventResult::NotHandled
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let border = ctx.tokens().color_border();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        let swatch = Rect::new(frame.x, frame.y + 4.0, 24.0, 24.0);
        ctx.fill_rect(swatch, self.value, r);
        ctx.stroke_rect(swatch, border, 1.0, r);

        if self.open {
            let cols = 8;
            let cell = 24.0;
            let pad = 8.0;
            let panel_w = cols as f32 * cell + pad * 2.0;
            let rows = (self.preset_colors.len() + cols - 1) / cols;
            let panel_h = rows as f32 * cell + pad * 2.0;
            let panel_x = frame.x;
            let panel_y = frame.y + 36.0;
            let bg = ctx.tokens().color_bg_elevated();
            let panel_rect = Rect::new(panel_x, panel_y, panel_w, panel_h);
            ctx.fill_rect(panel_rect, bg, Some(Radius::uniform(ctx.tokens().border_radius())));
            ctx.stroke_rect(panel_rect, border, 1.0, Some(Radius::uniform(ctx.tokens().border_radius())));

            for (i, c) in self.preset_colors.iter().enumerate() {
                let cx = panel_x + pad + (i % cols) as f32 * cell;
                let cy = panel_y + pad + (i / cols) as f32 * cell;
                ctx.fill_rect(Rect::new(cx + 1.0, cy + 1.0, cell - 2.0, cell - 2.0), *c, Some(Radius::uniform(2.0)));
            }
        }
    }
}
impl ColorPicker {
    pub fn new(value: Color) -> Self {
        Self {
            value,
            open: false,
            preset_colors: PRESET_COLORS.iter().map(|&c| Color::from_rgba(
                ((c >> 16) & 0xFF) as u8, ((c >> 8) & 0xFF) as u8, (c & 0xFF) as u8, 255
            )).collect(),
        }
    }
    pub fn value(&self) -> Color { self.value }
    pub fn set_value(&mut self, v: Color) { self.value = v; }
}

/// Cascader — 级联选择（简化版）。
define_widget! {
    pub struct Cascader {
        placeholder: String,
        value: String,
        open: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        Size::new(200.0, 32.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if let WidgetEvent::MouseDown { pos, .. } = event {
            if pos.y >= 0.0 && pos.y <= 32.0 { self.open = !self.open; return EventResult::Handled; }
            if self.open { self.open = false; }
        }
        EventResult::NotHandled
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_quaternary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius()));
        let input_rect = Rect::new(frame.x, frame.y, frame.w, 32.0);
        ctx.fill_rect(input_rect, bg, r);
        ctx.stroke_rect(input_rect, border, 1.0, r);
        let display = if self.value.is_empty() { &self.placeholder } else { &self.value };
        ctx.draw_text(display, Point::new(frame.x + 10.0, frame.y + 8.0),
            if self.value.is_empty() { text_sec } else { text }, 13.0);
        ctx.draw_text("▼", Point::new(frame.x + frame.w - 18.0, frame.y + 8.0), text_sec, 10.0);
    }
}
impl Cascader {
    pub fn new() -> Self { Self { placeholder: "请选择".into(), value: String::new(), open: false } }
    pub fn placeholder(mut self, p: &str) -> Self { self.placeholder = p.to_string(); self }
}
impl Default for Cascader { fn default() -> Self { Self::new() } }

/// Mentions — 提及输入（简化版）。
define_widget! {
    pub struct Mentions {
        placeholder: String,
        value: String,
        options: Vec<String>,
        open: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        Size::new(200.0, 32.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if let WidgetEvent::MouseDown { pos, .. } = event {
            if pos.y >= 0.0 && pos.y <= 32.0 { self.open = !self.open; return EventResult::Handled; }
            if self.open && pos.y > 32.0 {
                let idx = ((pos.y - 32.0) / 28.0) as usize;
                if idx < self.options.len() { self.value = format!("@{}", self.options[idx]); self.open = false; return EventResult::Handled; }
                self.open = false;
            }
        }
        EventResult::NotHandled
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_quaternary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius()));
        let input_rect = Rect::new(frame.x, frame.y, frame.w, 32.0);
        ctx.fill_rect(input_rect, bg, r);
        ctx.stroke_rect(input_rect, border, 1.0, r);
        let display = if self.value.is_empty() { &self.placeholder } else { &self.value };
        ctx.draw_text(display, Point::new(frame.x + 10.0, frame.y + 8.0),
            if self.value.is_empty() { text_sec } else { text }, 13.0);
        if self.open && !self.options.is_empty() {
            let bg_elev = ctx.tokens().color_bg_elevated();
            let menu_h = self.options.len() as f32 * 28.0;
            let menu_rect = Rect::new(frame.x, frame.y + 32.0, frame.w, menu_h);
            ctx.fill_rect(menu_rect, bg_elev, Some(Radius::uniform(ctx.tokens().border_radius_sm())));
            ctx.stroke_rect(menu_rect, border, 1.0, Some(Radius::uniform(ctx.tokens().border_radius_sm())));
            for (i, opt) in self.options.iter().enumerate() {
                ctx.draw_text(opt, Point::new(frame.x + 10.0, frame.y + 36.0 + i as f32 * 28.0), text, 13.0);
            }
        }
    }
}
impl Mentions {
    pub fn new() -> Self { Self { placeholder: "@提及".into(), value: String::new(), options: Vec::new(), open: false } }
    pub fn placeholder(mut self, p: &str) -> Self { self.placeholder = p.to_string(); self }
    pub fn options(mut self, opts: Vec<impl Into<String>>) -> Self { self.options = opts.into_iter().map(|s| s.into()).collect(); self }
}
impl Default for Mentions { fn default() -> Self { Self::new() } }
