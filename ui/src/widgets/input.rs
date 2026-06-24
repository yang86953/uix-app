use std::cell::{Cell, RefCell};

use crate::clipboard;
use crate::define_widget;
use uix_graphics::{Color, GraphicsEngine, Radius};
use uix_core::{Point, Rect, Size};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, KeyCode, KeyMod, WidgetEvent, WidgetTree};

pub use uix_core::ControlSize as InputSize;

pub fn input_height(size: InputSize) -> f32 {
    match size { InputSize::Small => 24.0, InputSize::Medium => 32.0, InputSize::Large => 40.0 }
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
        glyph_xs: RefCell<Vec<f32>>,
        selection: Cell<Option<(usize, usize)>>,
        sel_anchor: Cell<usize>,
        sel_dragging: Cell<bool>,
        // 新增字段
        prefix: String,
        suffix: String,
        addon_before: String,
        addon_after: String,
        password: bool,
        password_visible: bool,
        clearable: bool,
        search: bool,
        textarea: bool,
        textarea_rows: usize,
        on_change: Option<Box<dyn FnMut(&str) + 'static>>,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        let h = if self.textarea { (self.textarea_rows as f32 * 22.0 + 16.0).max(48.0) } else { input_height(self.input_size) };
        Size::new(80.0, h)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if self.disabled { return EventResult::NotHandled; }
        match event {
            WidgetEvent::MouseDown { pos, mods, .. } => {
                self.focused = true;
                if self.search || self.password { /* click on icon area */ }
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
            WidgetEvent::FocusOut => {
                self.focused = false; self.selection.set(None);
                if let Some(ref mut cb) = self.on_change { cb(&self.value); }
                EventResult::Handled
            }
            WidgetEvent::KeyDown { key, mods } => {
                let ctrl = mods.contains(KeyMod::CTRL);
                match key {
                    KeyCode::Enter if self.search => {
                        if let Some(ref mut cb) = self.on_change { cb(&self.value); }
                        EventResult::Handled
                    }
                    KeyCode::Enter if !self.textarea => EventResult::Handled,
                    KeyCode::A if ctrl => {
                        let len = self.value.chars().count();
                        self.sel_anchor.set(0);
                        self.set_selection_range(0, len);
                        self.cursor_char = len;
                        EventResult::Handled
                    }
                    KeyCode::C if ctrl => {
                        if let Some((s, e)) = self.selection.get() {
                            clipboard::copy_to_clipboard(&self.slice_range(s, e));
                        } else { clipboard::copy_to_clipboard(&self.value); }
                        EventResult::Handled
                    }
                    KeyCode::X if ctrl => {
                        if let Some((s, e)) = self.selection.get() {
                            clipboard::copy_to_clipboard(&self.slice_range(s, e));
                            self.delete_selection();
                        }
                        EventResult::Handled
                    }
                    KeyCode::Backspace => {
                        if self.selection.get().is_some() { self.delete_selection(); }
                        else if self.cursor_char > 0 {
                            let len = self.value.chars().count();
                            if self.cursor_char > len { self.cursor_char = len; }
                            if self.cursor_char == 0 { return EventResult::NotHandled; }
                            let byte_pos = self.value.char_indices().nth(self.cursor_char - 1).map(|(i, _)| i).unwrap_or(0);
                            self.value.remove(byte_pos);
                            self.cursor_char -= 1;
                        } else { return EventResult::NotHandled; }
                        if let Some(ref mut cb) = self.on_change { cb(&self.value); }
                        EventResult::Handled
                    }
                    KeyCode::Delete => {
                        if self.selection.get().is_some() { self.delete_selection(); }
                        else {
                            let len = self.value.chars().count();
                            if self.cursor_char > len { self.cursor_char = len; }
                            if self.cursor_char < len {
                                let byte_pos = self.value.char_indices().nth(self.cursor_char).map(|(i, _)| i).unwrap_or(self.value.len());
                                self.value.remove(byte_pos);
                            } else { return EventResult::NotHandled; }
                        }
                        if let Some(ref mut cb) = self.on_change { cb(&self.value); }
                        EventResult::Handled
                    }
                    KeyCode::Left => { self.selection.set(None); if self.cursor_char > 0 { self.cursor_char -= 1; } self.sel_anchor.set(self.cursor_char); EventResult::Handled }
                    KeyCode::Right => { self.selection.set(None); let len = self.value.chars().count(); if self.cursor_char < len { self.cursor_char += 1; } self.sel_anchor.set(self.cursor_char); EventResult::Handled }
                    KeyCode::Home => {
                        if !mods.contains(KeyMod::SHIFT) { self.selection.set(None); }
                        else { self.set_selection_range(self.sel_anchor.get(), 0); }
                        self.cursor_char = 0;
                        if !mods.contains(KeyMod::SHIFT) { self.sel_anchor.set(0); }
                        EventResult::Handled
                    }
                    KeyCode::End => {
                        let len = self.value.chars().count();
                        if !mods.contains(KeyMod::SHIFT) { self.selection.set(None); }
                        else { self.set_selection_range(self.sel_anchor.get(), len); }
                        self.cursor_char = len;
                        if !mods.contains(KeyMod::SHIFT) { self.sel_anchor.set(len); }
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
            WidgetEvent::KeyPress { text } => {
                if text.chars().any(|c| c.is_control()) { return EventResult::NotHandled; }
                if self.selection.get().is_some() { self.delete_selection(); }
                for ch in text.chars() {
                    let byte_pos = self.value.char_indices().nth(self.cursor_char).map(|(i, _)| i).unwrap_or(self.value.len());
                    self.value.insert(byte_pos, ch);
                    self.cursor_char += 1;
                }
                self.sel_anchor.set(self.cursor_char);
                if let Some(ref mut cb) = self.on_change { cb(&self.value); }
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let h = if self.textarea { frame.h } else { input_height(self.input_size).min(frame.h) };
        let input_frame = Rect::new(frame.x, frame.y, frame.w, h);

        let fill_tertiary = ctx.tokens().color_fill_tertiary();
        let border_color = ctx.tokens().color_border();
        let text_quaternary = ctx.tokens().color_text_quaternary();
        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let text_color_token = ctx.tokens().color_text();
        let text_tertiary = ctx.tokens().color_text_tertiary();
        let border_radius_sm = ctx.tokens().border_radius_sm();
        let text_sec = ctx.tokens().color_text_secondary();

        // addon 区域
        let addon_left_w = if self.addon_before.is_empty() { 0.0 } else { self.addon_before.len() as f32 * 8.0 + 16.0 };
        let addon_right_w = if self.addon_after.is_empty() { 0.0 } else { self.addon_after.len() as f32 * 8.0 + 16.0 };

        if !self.addon_before.is_empty() {
            let addon_rect = Rect::new(input_frame.x, input_frame.y, addon_left_w, input_frame.h);
            ctx.fill_rect(addon_rect, fill_tertiary, Some(Radius::uniform(border_radius_sm)));
            let ay = ctx.visual_center_y(addon_rect, 13.0);
            ctx.draw_text(&self.addon_before, Point::new(addon_rect.x + 8.0, ay), text_sec, 13.0);
        }
        if !self.addon_after.is_empty() {
            let addon_rect = Rect::new(input_frame.x + input_frame.w - addon_right_w, input_frame.y, addon_right_w, input_frame.h);
            ctx.fill_rect(addon_rect, fill_tertiary, Some(Radius::uniform(border_radius_sm)));
            let ay = ctx.visual_center_y(addon_rect, 13.0);
            ctx.draw_text(&self.addon_after, Point::new(addon_rect.x + 8.0, ay), text_sec, 13.0);
        }

        let inner_frame = Rect::new(input_frame.x + addon_left_w, input_frame.y, input_frame.w - addon_left_w - addon_right_w, input_frame.h);

        let prefix_w = if self.prefix.is_empty() { 0.0 } else { 20.0 };
        let suffix_w = if self.suffix.is_empty() { 0.0 } else { 20.0 };
        let clear_w = if self.clearable && !self.value.is_empty() { 20.0 } else { 0.0 };
        let pwd_w = if self.password { 24.0 } else { 0.0 };
        let search_w = if self.search { 24.0 } else { 0.0 };
        let right_extra = suffix_w + clear_w + pwd_w + search_w;

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
        ctx.fill_rect(inner_frame, bg, radius);
        ctx.stroke_rect(inner_frame, border, if self.focused { 2.0 } else { 1.0 }, radius);

        // prefix 图标
        if !self.prefix.is_empty() {
            let px = inner_frame.x + 6.0;
            let py = ctx.visual_center_y(inner_frame, 12.0);
            let icon_str = crate::widgets::icon::icon_char(&self.prefix);
            let saved = *ctx.font();
            if let Some(fh) = crate::widgets::icon::lucide_handle() { ctx.set_font(fh); }
            ctx.draw_text(icon_str, Point::new(px, py), text_sec, 12.0);
            ctx.set_font(saved);
        }

        // suffix 图标
        if !self.suffix.is_empty() {
            let sx = inner_frame.x + inner_frame.w - suffix_w - right_extra + 4.0 + suffix_w;
            let sy = ctx.visual_center_y(inner_frame, 12.0);
            let icon_str = crate::widgets::icon::icon_char(&self.suffix);
            let saved = *ctx.font();
            if let Some(fh) = crate::widgets::icon::lucide_handle() { ctx.set_font(fh); }
            ctx.draw_text(icon_str, Point::new(sx, sy), text_sec, 12.0);
            ctx.set_font(saved);
        }

        let display_text = if self.value.is_empty() { &self.placeholder } else { &self.value };
        let disp_color = if self.value.is_empty() && !self.focused { text_tertiary } else { text_color };

        let text_area_x = inner_frame.x + PAD + prefix_w;
        let text_area_w = (inner_frame.w - PAD * 2.0 - prefix_w - right_extra).max(20.0);
        if text_area_w <= 0.0 { return; }
        let text_area = Rect::new(text_area_x, inner_frame.y, text_area_w, inner_frame.h);
        ctx.engine().push_clip_rect(text_area);

        let mut scroll_off = self.scroll_offset_x.get();
        let total_text_w = if !self.value.is_empty() { ctx.measure_text(&self.value, FONT_SIZE).w } else { 0.0 };

        let cursor_byte_pos = self.value.char_indices().nth(self.cursor_char).map(|(i, _)| i).unwrap_or(self.value.len());
        let text_before = &self.value[..cursor_byte_pos];
        let text_before_w = if !text_before.is_empty() { ctx.measure_text(text_before, FONT_SIZE).w } else { 0.0 };

        let right_margin = 10.0;
        if text_before_w - scroll_off > text_area_w - right_margin {
            scroll_off = text_before_w - text_area_w + right_margin;
        }
        if text_before_w - scroll_off < 0.0 { scroll_off = text_before_w; }
        scroll_off = scroll_off.min(total_text_w - 1.0).max(0.0);
        self.scroll_offset_x.set(scroll_off);

        let draw_x = text_area_x - scroll_off;
        let draw_y = ctx.visual_center_y(text_area, FONT_SIZE);

        if !display_text.is_empty() {
            let opts = uix_graphics::TextLayoutOptions {
                max_width: f32::MAX, max_height: 0.0, line_height: FONT_SIZE * 1.5,
                word_wrap: false, h_align: uix_graphics::HAlign::Left,
                v_align: uix_graphics::VAlign::Top, font_size: FONT_SIZE,
            };
            let backend_opts = uix_graphics::text_backend::TextLayoutOptions::from(opts);
            let fh = *ctx.font();
            let layout = ctx.font_service().layout_text(&fh, display_text, &backend_opts);

            let abs_pos = Point::new(draw_x, draw_y);
            {
                let mut xs = self.glyph_xs.borrow_mut();
                xs.clear();
                for g in &layout.glyphs { xs.push(g.x); }
            }
            if !self.value.is_empty() {
                if let Some((sel_s, sel_e)) = self.selection.get() {
                    if sel_s < sel_e {
                        let visual_h = ctx.font_service().horizontal_line_metrics(&fh, FONT_SIZE)
                            .map(|m| m.ascent + m.descent).unwrap_or(FONT_SIZE * 1.2);
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
                            ctx.fill_rect(Rect::new(x0, abs_pos.y + line.y, (x1 - x0).max(0.0), visual_h), primary.with_alpha(64), None);
                        }
                    }
                }
            }
            ctx.blit_glyph_layout(&layout, abs_pos, disp_color, FONT_SIZE);
        }

        ctx.engine().pop_clip_rect();

        if self.focused && self.selection.get().is_none() {
            let cursor_x = text_area_x + text_before_w - scroll_off;
            ctx.fill_rect(Rect::new(cursor_x, inner_frame.y + 4.0, 1.5, inner_frame.h - 8.0), primary, None);
        }

        // clearable 按钮
        if self.clearable && !self.value.is_empty() && self.focused {
            let cx = inner_frame.x + inner_frame.w - 20.0;
            let cy = ctx.visual_center_y(inner_frame, 12.0);
            ctx.draw_text("✕", Point::new(cx, cy), text_sec, 12.0);
        }

        // password toggle
        if self.password {
            let px = inner_frame.x + inner_frame.w - pwd_w;
            let py = ctx.visual_center_y(inner_frame, 12.0);
            ctx.draw_text(if self.password_visible { "◎" } else { "◉" }, Point::new(px, py), text_sec, 14.0);
        }

        // search icon
        if self.search {
            let sx = inner_frame.x + inner_frame.w - search_w;
            let sy = ctx.visual_center_y(inner_frame, 12.0);
            ctx.draw_text("🔍", Point::new(sx, sy), text_sec, 12.0);
        }
    }
}

impl Input {
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            value: String::new(), placeholder: placeholder.into(),
            input_size: InputSize::Medium, disabled: false, focused: false, hovered: false,
            cursor_char: 0, scroll_offset_x: Cell::new(0.0), glyph_xs: RefCell::new(Vec::new()),
            selection: Cell::new(None), sel_anchor: Cell::new(0), sel_dragging: Cell::new(false),
            prefix: String::new(), suffix: String::new(), addon_before: String::new(), addon_after: String::new(),
            password: false, password_visible: false, clearable: false, search: false,
            textarea: false, textarea_rows: 3, on_change: None,
        }
    }
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into(); self.cursor_char = self.value.chars().count(); self.scroll_offset_x.set(0.0); self
    }
    pub fn size(mut self, s: InputSize) -> Self { self.input_size = s; self }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
    pub fn value(&self) -> &str { &self.value }
    pub fn set_value(&mut self, v: impl Into<String>) { self.value = v.into(); self.cursor_char = self.value.chars().count(); self.scroll_offset_x.set(0.0); self.selection.set(None); }
    pub fn prefix(mut self, s: &str) -> Self { self.prefix = s.to_string(); self }
    pub fn suffix(mut self, s: &str) -> Self { self.suffix = s.to_string(); self }
    pub fn addon_before(mut self, s: &str) -> Self { self.addon_before = s.to_string(); self }
    pub fn addon_after(mut self, s: &str) -> Self { self.addon_after = s.to_string(); self }
    pub fn password(mut self, v: bool) -> Self { self.password = v; self }
    pub fn clearable(mut self, v: bool) -> Self { self.clearable = v; self }
    pub fn search(mut self, v: bool) -> Self { self.search = v; self }
    pub fn textarea(mut self, v: bool) -> Self { self.textarea = v; self }
    pub fn on_change<F: FnMut(&str) + 'static>(mut self, f: F) -> Self { self.on_change = Some(Box::new(f)); self }

    fn char_at_x(&self, text_x: f32) -> usize {
        let xs = self.glyph_xs.borrow();
        if xs.is_empty() { return 0; }
        for (i, &gx) in xs.iter().enumerate() { if text_x < gx { return i; } }
        xs.len()
    }
    fn set_selection_range(&self, a: usize, b: usize) {
        if a == b { self.selection.set(None); } else { self.selection.set(Some((a.min(b), a.max(b)))); }
    }
    fn slice_range(&self, start_char: usize, end_char: usize) -> String {
        let chars: Vec<char> = self.value.chars().collect();
        let e = end_char.min(chars.len()); let s = start_char.min(e);
        chars[s..e].iter().collect()
    }
    fn delete_selection(&mut self) {
        if let Some((s, e)) = self.selection.get() {
            let chars: Vec<char> = self.value.chars().collect();
            let es = e.min(chars.len()); let ss = s.min(es);
            let byte_s = chars[..ss].iter().map(|c| c.len_utf8()).sum::<usize>();
            let byte_e = byte_s + chars[ss..es].iter().map(|c| c.len_utf8()).sum::<usize>();
            self.value.replace_range(byte_s..byte_e, "");
            self.cursor_char = ss; self.selection.set(None);
        }
    }
}
