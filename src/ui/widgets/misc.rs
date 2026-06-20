//! QRCode, Transfer, Upload, Watermark 组件。
//! Ant Design 5 补充实现。

use crate::base::{Point, Rect, Size};
use crate::define_widget;
use crate::graphics::{Color, Radius};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};

// ════════════════════════════════════════════════════════════════════════════
// QRCode
// ════════════════════════════════════════════════════════════════════════════

/// QRCode — 二维码显示（简化：绘制棋盘格占位，实际需集成二维码库）。
define_widget! {
    pub struct QRCode {
        value: String,
        size: f32,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        Size::new(self.size, self.size)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let bg = Color::white();
        let fg = Color::black();
        let r = Some(Radius::uniform(ctx.tokens().border_radius()));
        ctx.fill_rect(frame, bg, r);
        let cells = 21; // 简化 QR 网格
        let cell_s = frame.w / cells as f32;
        for y in 0..cells {
            for x in 0..cells {
                let is_pattern = (x < 7 && y < 7)
                    || (x >= cells - 7 && y < 7)
                    || (x < 7 && y >= cells - 7);
                let is_filled = if is_pattern {
                    !((x == 0 || x == 6 || y == 0 || y == 6) && is_pattern && !(x >= 2 && x <= 4 && y >= 2 && y <= 4))
                } else {
                    (x * 7 + y * 13 + x * y * 3) % 5 == 0
                };
                if is_filled {
                    ctx.fill_rect(Rect::new(frame.x + x as f32 * cell_s, frame.y + y as f32 * cell_s, cell_s, cell_s), fg, None);
                }
            }
        }
        // 中心标记
        ctx.fill_rect(Rect::new(frame.x + frame.w * 0.4, frame.y + frame.h * 0.4, frame.w * 0.2, frame.h * 0.2), bg, Some(Radius::uniform(3.0)));
        ctx.draw_text("UIX", Point::new(frame.x + frame.w * 0.4 + 4.0, frame.y + frame.h * 0.43), fg, 9.0);
    }
}
impl QRCode {
    pub fn new(value: &str) -> Self { Self { value: value.to_string(), size: 160.0 } }
    pub fn size(mut self, s: f32) -> Self { self.size = s; self }
}

// ════════════════════════════════════════════════════════════════════════════
// Transfer
// ════════════════════════════════════════════════════════════════════════════

/// TransferItem — 穿梭框项目。
#[derive(Debug, Clone)]
pub struct TransferItem {
    pub key: String,
    pub title: String,
    pub selected: bool,
}

/// Transfer — 穿梭框（双栏选择）。
define_widget! {
    pub struct Transfer {
        source: Vec<TransferItem>,
        target: Vec<TransferItem>,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        Size::new(500.0, 200.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if let WidgetEvent::MouseDown { pos, .. } = event {
            let half = 220.0;
            let item_h = 28.0;
            if pos.x < half {
                let idx = (pos.y / item_h) as usize;
                if idx < self.source.len() { self.source[idx].selected = !self.source[idx].selected; }
            } else if pos.x > half + 60.0 {
                let idx = (pos.y / item_h) as usize;
                let target_start = half + 60.0;
                let idx2 = (pos.y / item_h) as usize;
                if idx2 < self.target.len() { self.target[idx2].selected = !self.target[idx2].selected; }
            } else {
                // 中间按钮区域
                if pos.y >= 80.0 && pos.y < 100.0 {
                    // 向右移动选中
                    let mut i = 0;
                    while i < self.source.len() {
                        if self.source[i].selected {
                            let mut item = self.source.remove(i);
                            item.selected = false;
                            self.target.push(item);
                        } else {
                            i += 1;
                        }
                    }
                    return EventResult::Handled;
                }
                if pos.y >= 100.0 && pos.y < 120.0 {
                    let mut i = 0;
                    while i < self.target.len() {
                        if self.target[i].selected {
                            let mut item = self.target.remove(i);
                            item.selected = false;
                            self.source.push(item);
                        } else {
                            i += 1;
                        }
                    }
                    return EventResult::Handled;
                }
            }
        }
        EventResult::NotHandled
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_quaternary();
        let primary = ctx.tokens().color_primary();
        let fill = ctx.tokens().color_fill_tertiary();
        let half = 220.0;
        let item_h = 28.0;
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        // 左侧面板
        let left_rect = Rect::new(frame.x, frame.y, half, frame.h);
        ctx.fill_rect(left_rect, bg, r);
        ctx.stroke_rect(left_rect, border, 1.0, r);
        ctx.draw_text(&format!("源 ({}项)", self.source.len()), Point::new(frame.x + 8.0, frame.y + 6.0), text_sec, 12.0);
        for (i, item) in self.source.iter().enumerate() {
            let y = frame.y + 24.0 + i as f32 * item_h;
            if item.selected { ctx.fill_rect(Rect::new(frame.x, y, half, item_h), fill, None); }
            ctx.draw_text(if item.selected { "☑" } else { "☐" }, Point::new(frame.x + 8.0, y + 5.0), text, 12.0);
            ctx.draw_text(&item.title, Point::new(frame.x + 26.0, y + 5.0), text, 13.0);
        }

        // 中间按钮
        let btn_y = frame.y + frame.h * 0.5 - 20.0;
        ctx.fill_rect(Rect::new(frame.x + half + 8.0, btn_y, 44.0, 20.0), primary, Some(Radius::uniform(3.0)));
        ctx.draw_text("→", Point::new(frame.x + half + 24.0, btn_y + 2.0), Color::white(), 14.0);
        ctx.fill_rect(Rect::new(frame.x + half + 8.0, btn_y + 24.0, 44.0, 20.0), border, Some(Radius::uniform(3.0)));
        ctx.draw_text("←", Point::new(frame.x + half + 24.0, btn_y + 26.0), text, 14.0);

        // 右侧面板
        let right_x = frame.x + half + 60.0;
        let right_rect = Rect::new(right_x, frame.y, half, frame.h);
        ctx.fill_rect(right_rect, bg, r);
        ctx.stroke_rect(right_rect, border, 1.0, r);
        ctx.draw_text(&format!("目标 ({}项)", self.target.len()), Point::new(right_x + 8.0, frame.y + 6.0), text_sec, 12.0);
        for (i, item) in self.target.iter().enumerate() {
            let y = frame.y + 24.0 + i as f32 * item_h;
            if item.selected { ctx.fill_rect(Rect::new(right_x, y, half, item_h), fill, None); }
            ctx.draw_text(if item.selected { "☑" } else { "☐" }, Point::new(right_x + 8.0, y + 5.0), text, 12.0);
            ctx.draw_text(&item.title, Point::new(right_x + 26.0, y + 5.0), text, 13.0);
        }
    }
}
impl Transfer {
    pub fn new() -> Self { Self { source: Vec::new(), target: Vec::new() } }
    pub fn source(mut self, items: Vec<TransferItem>) -> Self { self.source = items; self }
    pub fn target(mut self, items: Vec<TransferItem>) -> Self { self.target = items; self }
}
impl Default for Transfer { fn default() -> Self { Self::new() } }

// ════════════════════════════════════════════════════════════════════════════
// Upload
// ════════════════════════════════════════════════════════════════════════════

/// Upload — 上传组件（简化版，点击触发选择 + 文件列表展示）。
define_widget! {
    pub struct Upload {
        accept: String,
        multiple: bool,
        file_list: Vec<String>,
        drag: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        let h = if self.file_list.is_empty() { 100.0 } else { 100.0 + self.file_list.len() as f32 * 28.0 };
        Size::new(300.0, h)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border();
        let text_sec = ctx.tokens().color_text_quaternary();
        let text = ctx.tokens().color_text();
        let primary = ctx.tokens().color_primary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius()));
        let upload_rect = Rect::new(frame.x, frame.y, frame.w, 100.0);
        ctx.fill_rect(upload_rect, bg, r);
        ctx.stroke_rect(upload_rect, border, if self.drag { 2.0 } else { 1.0 }, r);
        if self.drag {
            ctx.stroke_rect(Rect::new(frame.x + 4.0, frame.y + 4.0, frame.w - 8.0, 92.0),
                primary, 1.0, Some(Radius::uniform(ctx.tokens().border_radius_sm())));
        }
        ctx.draw_text("📁", Point::new(frame.x + frame.w * 0.5 - 12.0, frame.y + 24.0), text_sec, 24.0);
        ctx.draw_text("点击或拖拽上传", Point::new(frame.x + frame.w * 0.5 - 48.0, frame.y + 60.0), text_sec, 13.0);
        ctx.draw_text(&format!("支持: {}", self.accept), Point::new(frame.x + frame.w * 0.5 - 36.0, frame.y + 78.0), text_sec, 10.0);

        // 文件列表
        for (i, f) in self.file_list.iter().enumerate() {
            let y = frame.y + 104.0 + i as f32 * 28.0;
            ctx.draw_text("📄", Point::new(frame.x + 8.0, y + 4.0), text_sec, 14.0);
            ctx.draw_text(f, Point::new(frame.x + 28.0, y + 5.0), text, 13.0);
        }
    }
}
impl Upload {
    pub fn new() -> Self { Self { accept: "*".into(), multiple: false, file_list: Vec::new(), drag: false } }
    pub fn accept(mut self, a: &str) -> Self { self.accept = a.to_string(); self }
    pub fn multiple(mut self, v: bool) -> Self { self.multiple = v; self }
    pub fn drag(mut self, v: bool) -> Self { self.drag = v; self }
    pub fn add_file(&mut self, name: &str) { self.file_list.push(name.to_string()); }
}
impl Default for Upload { fn default() -> Self { Self::new() } }

// ════════════════════════════════════════════════════════════════════════════
// Watermark
// ════════════════════════════════════════════════════════════════════════════

/// Watermark — 水印组件。
define_widget! {
    pub struct Watermark {
        text: String,
        color: Color,
        font_size: f32,
        opacity: f32,
        rotate: f32,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        Size::zero()
    }

    render => (&self, _frame: Rect, _ctx: &mut RenderContext, _tree: &WidgetTree) {}

    post_render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if self.text.is_empty() { return; }
        let step_x = 200.0;
        let step_y = 160.0;
        let mut c = self.color;
        c.a = (self.opacity * 255.0) as u8;
        let text_w = self.text.len() as f32 * self.font_size * 0.6;
        for y in (0..(frame.h as i32)).step_by(step_y as usize) {
            for x in (0..(frame.w as i32)).step_by(step_x as usize) {
                ctx.draw_text(&self.text, Point::new(x as f32 + (y as f32 * 0.3) % step_x, y as f32), c, self.font_size);
            }
        }
    }
}
impl Watermark {
    pub fn new(text: &str) -> Self {
        Self { text: text.to_string(), color: Color::from_rgba(0, 0, 0, 255), font_size: 14.0, opacity: 0.15, rotate: -22.0 }
    }
    pub fn color(mut self, c: Color) -> Self { self.color = c; self }
    pub fn font_size(mut self, s: f32) -> Self { self.font_size = s; self }
    pub fn opacity(mut self, o: f32) -> Self { self.opacity = o; self }
}


