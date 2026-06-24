//! Input widget — Ant Design style text input with placeholder, focus, and states.

use std::cell::Cell;
use std::cell::RefCell;

use crate::clipboard;
use crate::define_widget;
use uix_graphics::{Color, GraphicsEngine, Radius};
use uix_core::{Rect, Size};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, KeyCode, KeyMod, WidgetEvent, WidgetTree};

/// 输入框尺寸。
/// （已统一为 uix_core::ControlSize，保留别名以兼容旧代码。）
pub use uix_core::ControlSize as InputSize;

/// Input 尺寸对应的高度。
pub fn input_height(size: InputSize) -> f32 {
    match size {
        InputSize::Small => 24.0,
        InputSize::Medium => 32.0,
        InputSize::Large => 40.0,
    }
}

const PAD: f32 = 12.0;
const FONT_SIZE: f32 = 14.0;

define_widget! {
    pub struct Input {
        value: String,
        placeholder: String,
        input_size: InputSize,
        disabled: bool,
        focused: bool,
        hovered: bool,
        cursor_char: usize,
        scroll_offset_x: Cell<f32>,
        /// 渲染时缓存的字形 x 位置（文本局部坐标），用于事件命中测试。
        glyph_xs: RefCell<Vec<f32>>,
        /// 选中区间 (start, end)，start < end。None 表示无选中。
        selection: Cell<Option<(usize, usize)>>,
        sel_anchor: Cell<usize>,
        sel_dragging: Cell<bool>,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        let h = input_height(self.input_size);
        Size::new(80.0, h)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if self.disabled { return EventResult::NotHandled; }
        match event {
            WidgetEvent::MouseDown { pos, mods, .. } => {
                self.focused = true;

                let text_x = pos.x - PAD + self.scroll_offset_x.get();
                let ci = self.char_at_x(text_x);
                self.cursor_char = ci;

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
                let text_x = pos.x - PAD + self.scroll_offset_x.get();
                let ci = self.char_at_x(text_x);
                self.cursor_char = ci;
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
            WidgetEvent::HoverEnter => { self.hovered = true; EventResult::Handled }
            WidgetEvent::HoverLeave => { self.hovered = false; EventResult::Handled }
            WidgetEvent::FocusOut => { self.focused = false; self.selection.set(None); EventResult::Handled }
            WidgetEvent::KeyDown { key, mods } => {
                let ctrl = mods.contains(KeyMod::CTRL);
                match key {
                    KeyCode::A if ctrl => {
                        let len = self.value.chars().count();
                        self.sel_anchor.set(0);
                        self.set_selection_range(0, len);
                        self.cursor_char = len;
                        EventResult::Handled
                    }
                    KeyCode::C if ctrl => {
                        if let Some((s, e)) = self.selection.get() {
                            let selected = self.slice_range(s, e);
                            clipboard::copy_to_clipboard(&selected);
                        } else {
                            clipboard::copy_to_clipboard(&self.value);
                        }
                        EventResult::Handled
                    }
                    KeyCode::X if ctrl => {
                        if let Some((s, e)) = self.selection.get() {
                            let selected = self.slice_range(s, e);
                            clipboard::copy_to_clipboard(&selected);
                            self.delete_selection();
                        }
                        EventResult::Handled
                    }
                    KeyCode::Backspace => {
                        if self.selection.get().is_some() {
                            self.delete_selection();
                        } else if self.cursor_char > 0 {
                            // 防御：确保 cursor_char 不超过文本长度
                            let len = self.value.chars().count();
                            if self.cursor_char > len {
                                self.cursor_char = len;
                            }
                            if self.cursor_char == 0 {
                                return EventResult::NotHandled;
                            }
                            let byte_pos = self.value.char_indices()
                                .nth(self.cursor_char - 1)
                                .map(|(i, _)| i)
                                .unwrap_or(0);
                            self.value.remove(byte_pos);
                            self.cursor_char -= 1;
                        } else {
                            return EventResult::NotHandled;
                        }
                        EventResult::Handled
                    }
                    KeyCode::Delete => {
                        if self.selection.get().is_some() {
                            self.delete_selection();
                        } else {
                            let len = self.value.chars().count();
                            // 防御：确保 cursor_char 不越界
                            if self.cursor_char > len {
                                self.cursor_char = len;
                            }
                            if self.cursor_char < len {
                                let byte_pos = self.value.char_indices()
                                    .nth(self.cursor_char)
                                    .map(|(i, _)| i)
                                    .unwrap_or(self.value.len());
                                self.value.remove(byte_pos);
                            } else {
                                return EventResult::NotHandled;
                            }
                        }
                        EventResult::Handled
                    }
                    KeyCode::Left => {
                        self.selection.set(None);
                        if self.cursor_char > 0 { self.cursor_char -= 1; }
                        self.sel_anchor.set(self.cursor_char);
                        EventResult::Handled
                    }
                    KeyCode::Right => {
                        self.selection.set(None);
                        let len = self.value.chars().count();
                        if self.cursor_char < len { self.cursor_char += 1; }
                        self.sel_anchor.set(self.cursor_char);
                        EventResult::Handled
                    }
                    KeyCode::Home => {
                        if !mods.contains(KeyMod::SHIFT) { self.selection.set(None); }
                        else {
                            let anchor = self.sel_anchor.get();
                            self.set_selection_range(anchor, 0);
                        }
                        self.cursor_char = 0;
                        if !mods.contains(KeyMod::SHIFT) { self.sel_anchor.set(0); }
                        EventResult::Handled
                    }
                    KeyCode::End => {
                        let len = self.value.chars().count();
                        if !mods.contains(KeyMod::SHIFT) { self.selection.set(None); }
                        else {
                            let anchor = self.sel_anchor.get();
                            self.set_selection_range(anchor, len);
                        }
                        self.cursor_char = len;
                        if !mods.contains(KeyMod::SHIFT) { self.sel_anchor.set(len); }
                        EventResult::Handled
                    }
                    KeyCode::Enter => EventResult::Handled,
                    _ => EventResult::NotHandled,
                }
            }
            WidgetEvent::KeyPress { text } => {
                if text.chars().any(|c| c.is_control()) {
                    return EventResult::NotHandled;
                }
                if self.selection.get().is_some() {
                    self.delete_selection();
                }
                for ch in text.chars() {
                    let byte_pos = self.value.char_indices()
                        .nth(self.cursor_char)
                        .map(|(i, _)| i)
                        .unwrap_or(self.value.len());
                    self.value.insert(byte_pos, ch);
                    self.cursor_char += 1;
                }
                self.sel_anchor.set(self.cursor_char);
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let h = input_height(self.input_size);
        let input_frame = Rect::new(frame.x, frame.y, frame.w, h.min(frame.h));

        let fill_tertiary = ctx.tokens().color_fill_tertiary();
        let border_color = ctx.tokens().color_border();
        let text_quaternary = ctx.tokens().color_text_quaternary();
        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let text_color_token = ctx.tokens().color_text();
        let text_tertiary = ctx.tokens().color_text_tertiary();
        let border_radius_sm = ctx.tokens().border_radius_sm();

        let (bg, border, text_color) = if self.disabled {
            (fill_tertiary, border_color, text_quaternary)
        } else if self.focused {
            (Color::white(), primary, text_color_token)
        } else if self.hovered {
            (Color::white(), primary_hover, text_color_token)
        } else {
            (Color::white(), border_color, text_color_token)
        };

        let radius = Some(Radius::uniform(border_radius_sm));
        ctx.fill_rect(input_frame, bg, radius);
        ctx.stroke_rect(input_frame, border, if self.focused { 2.0 } else { 1.0 }, radius);

        let display_text = if self.value.is_empty() { &self.placeholder } else { &self.value };
        let text_color = if self.value.is_empty() && !self.focused { text_tertiary } else { text_color };

        let text_area_w = input_frame.w - PAD * 2.0;
        if text_area_w <= 0.0 { return; }
        let text_area = Rect::new(input_frame.x + PAD, input_frame.y, text_area_w, input_frame.h);
        ctx.engine().push_clip_rect(text_area);

        let mut scroll_off = self.scroll_offset_x.get();
        let total_text_w = if !self.value.is_empty() {
            ctx.measure_text(&self.value, FONT_SIZE).w
        } else { 0.0 };

        let cursor_byte_pos = self.value.char_indices()
            .nth(self.cursor_char)
            .map(|(i, _)| i)
            .unwrap_or(self.value.len());
        let text_before = &self.value[..cursor_byte_pos];
        let text_before_w = if !text_before.is_empty() {
            ctx.measure_text(text_before, FONT_SIZE).w
        } else { 0.0 };

        let right_margin = 10.0;
        if text_before_w - scroll_off > text_area_w - right_margin {
            scroll_off = text_before_w - text_area_w + right_margin;
        }
        if text_before_w - scroll_off < 0.0 {
            scroll_off = text_before_w;
        }
        scroll_off = scroll_off.min(total_text_w - 1.0).max(0.0);
        self.scroll_offset_x.set(scroll_off);

        let draw_x = input_frame.x + PAD - scroll_off;
        let text_rect_for_y = Rect::new(input_frame.x + PAD, input_frame.y, text_area_w, input_frame.h);
        let draw_y = ctx.visual_center_y(text_rect_for_y, FONT_SIZE);

        if !display_text.is_empty() {
            // 单次布局：同时用于 hit-test 缓存、选中背景和文字绘制
            let opts = uix_graphics::TextLayoutOptions {
                max_width: f32::MAX,
                max_height: 0.0,
                line_height: FONT_SIZE * 1.5,
                word_wrap: false,
                h_align: uix_graphics::HAlign::Left,
                v_align: uix_graphics::VAlign::Top,
                font_size: FONT_SIZE,
            };
            let backend_opts = uix_graphics::text_backend::TextLayoutOptions::from(opts);
            let fh = *ctx.font();
            let layout = ctx.font_service().layout_text(&fh, display_text, &backend_opts);

            let abs_pos = uix_core::Point::new(draw_x, draw_y);

            // 缓存 glyph x 位置
            {
                let mut xs = self.glyph_xs.borrow_mut();
                xs.clear();
                for g in &layout.glyphs {
                    xs.push(g.x);
                }
            }

            // 绘制选中背景（仅当有实际值且选区非空时）
            if !self.value.is_empty() {
                if let Some((sel_s, sel_e)) = self.selection.get() {
                    if sel_s < sel_e {
                        // 文字实际视觉高度（ascent + descent），而非行间距
                        let visual_h = ctx.font_service()
                            .horizontal_line_metrics(&fh, FONT_SIZE)
                            .map(|m| m.ascent + m.descent)
                            .unwrap_or(FONT_SIZE * 1.2);
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
                                primary.with_alpha(64),
                                None,
                            );
                        }
                    }
                }
            }

            // 绘制文本（使用同一布局）
            ctx.blit_glyph_layout(&layout, abs_pos, text_color, FONT_SIZE);
        }

        ctx.engine().pop_clip_rect();

        if self.focused && self.selection.get().is_none() {
            let cursor_x = input_frame.x + PAD + text_before_w - scroll_off;
            ctx.fill_rect(Rect::new(cursor_x, input_frame.y + 4.0, 1.5, input_frame.h - 8.0), primary, None);
        }
    }
}

impl Input {
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            value: String::new(),
            placeholder: placeholder.into(),
            input_size: InputSize::Medium,
            disabled: false,
            focused: false,
            hovered: false,
            cursor_char: 0,
            scroll_offset_x: Cell::new(0.0),
            glyph_xs: RefCell::new(Vec::new()),
            selection: Cell::new(None),
            sel_anchor: Cell::new(0),
            sel_dragging: Cell::new(false),
        }
    }
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self.cursor_char = self.value.chars().count();
        self.scroll_offset_x.set(0.0);
        self
    }
    pub fn size(mut self, s: InputSize) -> Self { self.input_size = s; self }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
    pub fn value(&self) -> &str { &self.value }
    pub fn set_value(&mut self, v: impl Into<String>) {
        self.value = v.into();
        self.cursor_char = self.value.chars().count();
        self.scroll_offset_x.set(0.0);
        self.selection.set(None);
    }

    fn char_at_x(&self, text_x: f32) -> usize {
        let xs = self.glyph_xs.borrow();
        if xs.is_empty() { return 0; }
        for (i, &gx) in xs.iter().enumerate() {
            if text_x < gx {
                return i;
            }
        }
        xs.len()
    }

    fn set_selection_range(&self, a: usize, b: usize) {
        if a == b { self.selection.set(None); }
        else { self.selection.set(Some((a.min(b), a.max(b)))); }
    }

    fn slice_range(&self, start_char: usize, end_char: usize) -> String {
        let chars: Vec<char> = self.value.chars().collect();
        let e = end_char.min(chars.len());
        let s = start_char.min(e);
        chars[s..e].iter().collect()
    }

    fn delete_selection(&mut self) {
        if let Some((s, e)) = self.selection.get() {
            let chars: Vec<char> = self.value.chars().collect();
            let es = e.min(chars.len());
            let ss = s.min(es);
            let byte_s = chars[..ss].iter().map(|c| c.len_utf8()).sum::<usize>();
            let byte_e = byte_s + chars[ss..es].iter().map(|c| c.len_utf8()).sum::<usize>();
            self.value.replace_range(byte_s..byte_e, "");
            self.cursor_char = ss;
            self.selection.set(None);
        }
    }
}
