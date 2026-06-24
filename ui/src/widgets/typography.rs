//! Typography widget — 排版组件（标题/段落/文本）。
//!
//! 支持 h1-h5 标题级别、段落文本、disabled/type 等变体。
//! 与 Label 的区别：Typography 提供语义化排版和更多样式选项。

use std::cell::Cell;
use std::cell::RefCell;

use uix_core::{Point, Rect, Size};
use crate::clipboard;
use crate::define_widget;
use uix_graphics::Color;
use crate::render_context::RenderContext;
use crate::widget::{EventResult, KeyCode, KeyMod, WidgetEvent, WidgetTree};

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
        copyable: bool,
        color_override: Option<Color>,
        glyph_xs: RefCell<Vec<f32>>,
        /// 每行的 (相对 y, 字形数量)，用于 y 轴命中测试。
        line_info: RefCell<Vec<(f32, usize)>>,
        selection: Cell<Option<(usize, usize)>>,
        sel_anchor: Cell<usize>,
        sel_dragging: Cell<bool>,
        draw_pos: Cell<uix_core::Point>,
    }

    preferred_size => (&self, engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        let (fs, _fw) = self.compute_font_style();
        if let Some(eng) = engine {
            let opts = uix_graphics::TextLayoutOptions {
                max_width: f32::MAX,
                max_height: 0.0,
                line_height: fs * 1.5,
                word_wrap: false,
                h_align: uix_graphics::HAlign::Left,
                v_align: uix_graphics::VAlign::Top,
                font_size: fs,
            };
            let sz = eng.measure_text(&uix_graphics::FontHandle::default(), &self.content, &opts);
            if sz.w > 0.0 && sz.h > 0.0 {
                return Size::new(sz.w, sz.h.max(fs * 1.5));
            }
        }
        let w = self.content.len() as f32 * fs * 0.6;
        let h = fs * 1.5;
        Size::new(w, h)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos, mods, .. } => {
                let dp = self.draw_pos.get();
                let text_x = pos.x - dp.x;
                let text_y = pos.y - dp.y;
                let ci = self.char_at_xy(text_x, text_y);
                if mods.contains(KeyMod::SHIFT) {
                    let anchor = self.sel_anchor.get();
                    self.set_selection_range(anchor, ci);
                } else {
                    self.selection.set(None);
                    self.sel_anchor.set(ci);
                }
                self.sel_dragging.set(true);
                EventResult::Handled
            }
            WidgetEvent::MouseMove { pos } => {
                if !self.sel_dragging.get() { return EventResult::NotHandled; }
                let dp = self.draw_pos.get();
                let text_x = pos.x - dp.x;
                let text_y = pos.y - dp.y;
                let ci = self.char_at_xy(text_x, text_y);
                let anchor = self.sel_anchor.get();
                self.set_selection_range(anchor, ci);
                EventResult::Handled
            }
            WidgetEvent::MouseUp { .. } => {
                self.sel_dragging.set(false);
                if let Some((s, e)) = self.selection.get() {
                    if s == e { self.selection.set(None); }
                }
                EventResult::Handled
            }
            WidgetEvent::KeyDown { key, mods } => {
                let ctrl = mods.contains(KeyMod::CTRL);
                match key {
                    KeyCode::A if ctrl => {
                        let len = self.content.chars().count();
                        self.sel_anchor.set(0);
                        self.set_selection_range(0, len);
                        EventResult::Handled
                    }
                    KeyCode::C if ctrl => {
                        if let Some((s, e)) = self.selection.get() {
                            let selected = self.slice_range(s, e);
                            clipboard::copy_to_clipboard(&selected);
                        } else {
                            clipboard::copy_to_clipboard(&self.content);
                        }
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let (fs, _fw) = self.compute_font_style();
        let text_c = self.color_override.unwrap_or_else(|| {
            if self.disabled { ctx.tokens().color_text_quaternary() } else { ctx.tokens().color_text() }
        });

        // 单次布局：同时用于 hit-test 缓存、选中背景和文字绘制
        let opts = uix_graphics::TextLayoutOptions {
            max_width: frame.w.max(1.0),
            max_height: 0.0,
            line_height: fs * 1.5,
            word_wrap: false,
            h_align: uix_graphics::HAlign::Left,
            v_align: uix_graphics::VAlign::Top,
            font_size: fs,
        };
        let backend_opts = uix_graphics::text_backend::TextLayoutOptions::from(opts);
        let fh = *ctx.font();
        let layout = ctx.font_service().layout_text(&fh, &self.content, &backend_opts);

        let x = 0.0;
        let y = ctx.visual_center_y(frame, fs) - frame.y;
        let draw_pos = uix_core::Point::new(x, y);
        self.draw_pos.set(draw_pos);
        let abs_pos = uix_core::Point::new(frame.x + draw_pos.x, frame.y + draw_pos.y);

        if !self.content.is_empty() {
            // 缓存 glyph x 位置
            {
                let mut xs = self.glyph_xs.borrow_mut();
                xs.clear();
                for g in &layout.glyphs {
                    xs.push(g.x);
                }
            }

            // 缓存行信息（用于 y 轴命中测试）
            {
                let mut li = self.line_info.borrow_mut();
                li.clear();
                for l in &layout.lines {
                    li.push((l.y, l.glyph_count));
                }
            }

            // 绘制选中背景（与文字使用同一布局，保证完全对齐）
            if let Some((sel_s, sel_e)) = self.selection.get() {
                if sel_s < sel_e {
                    // 文字实际视觉高度（ascent + descent），而非行间距
                    let visual_h = ctx.font_service()
                        .horizontal_line_metrics(&fh, fs)
                        .map(|m| m.ascent + m.descent)
                        .unwrap_or(fs * 1.2);
                    let end = sel_e.min(layout.glyphs.len());
                    let start = sel_s.min(end);
                    for line in &layout.lines {
                        let gs = line.glyph_start;
                        let gc = line.glyph_count;
                        let ge = gs + gc;
                        let ls = start.max(gs);
                        let le = end.min(ge);
                        if ls >= le { continue; }
                        let glyphs = &layout.glyphs[ls..le];
                        let x0 = abs_pos.x + glyphs[0].x;
                        let last = glyphs[glyphs.len() - 1];
                        let x1 = abs_pos.x + last.x + last.width.max(0.0);
                        let y0 = abs_pos.y + line.y;
                        let h = visual_h;
                        ctx.fill_rect(
                            Rect::new(x0, y0, (x1 - x0).max(0.0), h),
                            ctx.tokens().color_primary().with_alpha(64),
                            None,
                        );
                    }
                }
            }

            // 绘制文本（使用同一布局）
            ctx.blit_glyph_layout(&layout, abs_pos, text_c, fs);
        }

        // copyable 图标
        if self.copyable {
            let copy_icon = "📋";
            let copy_x = frame.x + frame.w - 22.0;
            let copy_y = ctx.visual_center_y(frame, 14.0);
            ctx.draw_text(copy_icon, Point::new(copy_x, copy_y), ctx.tokens().color_text_quaternary(), 14.0);
        }
    }
}

impl Typography {
    pub fn new(content: &str, type_: TypographyType) -> Self {
        Self {
            content: content.to_string(),
            type_,
            disabled: false,
            mark: false, code: false, underline: false, delete: false,
            strong: false, italic: false, copyable: false,
            color_override: None,
            glyph_xs: RefCell::new(Vec::new()),
            line_info: RefCell::new(Vec::new()),
            selection: Cell::new(None),
            sel_anchor: Cell::new(0),
            sel_dragging: Cell::new(false),
            draw_pos: Cell::new(uix_core::Point::new(0.0, 0.0)),
        }
    }
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
    pub fn paragraph(content: &str) -> Self { Self::new(content, TypographyType::Paragraph) }
    pub fn text(content: &str) -> Self { Self::new(content, TypographyType::Text) }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
    pub fn mark(mut self) -> Self { self.mark = true; self }
    pub fn code(mut self) -> Self { self.code = true; self }
    pub fn underline(mut self) -> Self { self.underline = true; self }
    pub fn delete(mut self) -> Self { self.delete = true; self }
    pub fn strong(mut self) -> Self { self.strong = true; self }
    pub fn italic(mut self) -> Self { self.italic = true; self }
    pub fn color(mut self, c: Color) -> Self { self.color_override = Some(c); self }
    pub fn copyable(mut self, v: bool) -> Self { self.copyable = v; self }

    pub fn selected_text(&self) -> Option<String> {
        self.selection.get().map(|(s, e)| self.slice_range(s, e))
    }

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

    fn char_at_xy(&self, text_x: f32, text_y: f32) -> usize {
        let xs = self.glyph_xs.borrow();
        let li = self.line_info.borrow();
        if xs.is_empty() { return 0; }
        if li.is_empty() {
            for (i, &gx) in xs.iter().enumerate() {
                if text_x < gx { return i; }
            }
            return xs.len();
        }
        // 将 text_y 钳制到有效行区间，点击在文本上/下方时落在首/末行
        let mut target_y = text_y;
        let first_ly = li.first().map(|(ly, _)| *ly).unwrap_or(0.0);
        if target_y < first_ly { target_y = first_ly; }
        // 根据 y 坐标找到所在行
        let mut global_off = 0usize;
        let mut line_gc = 0usize;
        for (i, &(ly, gc)) in li.iter().enumerate() {
            let next_y = li.get(i + 1).map(|(ny, _)| *ny).unwrap_or(f32::MAX);
            if target_y >= ly && target_y < next_y {
                line_gc = gc;
                break;
            }
            global_off += gc;
        }
        // 在所在行内按 x 查找
        let end = (global_off + line_gc).min(xs.len());
        for i in global_off..end {
            if text_x < xs[i] { return i; }
        }
        end
    }

    fn set_selection_range(&self, a: usize, b: usize) {
        if a == b { self.selection.set(None); }
        else { self.selection.set(Some((a.min(b), a.max(b)))); }
    }

    fn slice_range(&self, start_char: usize, end_char: usize) -> String {
        let chars: Vec<char> = self.content.chars().collect();
        let e = end_char.min(chars.len());
        let s = start_char.min(e);
        chars[s..e].iter().collect()
    }
}
