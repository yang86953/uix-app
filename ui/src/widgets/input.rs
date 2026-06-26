//! Input widget — 单行/多行文本输入框
//!
//! 支持单行 Input 和多行 Textarea 两种模式。
//! textarea 模式：Enter 提交（可通过 on_submit 回调），Shift+Enter 换行。
//! 支持文字选择、粘贴、键盘导航、前缀/后缀图标等。

use std::cell::{Cell, RefCell};

use crate::clipboard;
use crate::define_widget;
use uix_graphics::{GraphicsEngine, Radius};
use uix_core::{Point, Rect, Size};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, KeyCode, KeyMod, WidgetEvent, WidgetTree};

pub use uix_core::ControlSize as InputSize;

pub fn input_height(size: InputSize) -> f32 {
    match size { InputSize::Small => 24.0, InputSize::Medium => 32.0, InputSize::Large => 40.0 }
}

const PAD: f32 = 12.0;
pub(crate) const FONT_SIZE: f32 = 14.0;
const LINE_HEIGHT: f32 = 22.0;

define_widget! {
    pub struct Input {
        value: String,
        placeholder: String,
        input_size: InputSize,
        disabled: bool,
        focused: bool,
        hovered: bool,
        /// 当前光标所在的字符索引（全文本平展）
        cursor_char: usize,
        /// 水平滚动偏移（单行模式）
        scroll_offset_x: Cell<f32>,
        /// 垂直滚动行偏移（多行模式）
        scroll_line: Cell<usize>,
        glyph_xs: RefCell<Vec<f32>>,
        selection: Cell<Option<(usize, usize)>>,
        sel_anchor: Cell<usize>,
        sel_dragging: Cell<bool>,
        // 扩展字段
        prefix: String,
        suffix: String,
        addon_before: String,
        addon_after: String,
        password: bool,
        password_visible: bool,
        clearable: bool,
        search: bool,
        /// 多行模式
        textarea: bool,
        /// 默认显示行数
        textarea_rows: usize,
        /// 值变更回调
        on_change: Option<Box<dyn FnMut(&str) + 'static>>,
        /// 提交回调（Enter 触发）
        on_submit: Option<Box<dyn FnMut(&str) + 'static>>,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        if self.textarea {
            let line_count = self.value.lines().count().max(self.textarea_rows);
            let h = (line_count as f32 * LINE_HEIGHT + 16.0).max(48.0);
            Size::new(80.0, h)
        } else {
            Size::new(80.0, input_height(self.input_size))
        }
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if self.disabled { return EventResult::NotHandled; }
        match event {
            WidgetEvent::MouseDown { pos, mods, .. } => {
                self.focused = true;
                let ci = if self.textarea {
                    self.char_at_xy(pos.x - PAD, pos.y)
                } else {
                    let text_x = pos.x - PAD + self.scroll_offset_x.get();
                    self.char_at_x(text_x)
                };
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
                let ci = if self.textarea {
                    self.char_at_xy(pos.x - PAD, pos.y)
                } else {
                    let text_x = pos.x - PAD + self.scroll_offset_x.get();
                    self.char_at_x(text_x)
                };
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
                let shift = mods.contains(KeyMod::SHIFT);
                match key {
                    KeyCode::Enter if self.search => {
                        if let Some(ref mut cb) = self.on_submit { cb(&self.value); }
                        self.value.clear();
                        self.cursor_char = 0;
                        EventResult::Handled
                    }
                    // textarea: Shift+Enter 换行, Enter 提交
                    KeyCode::Enter if self.textarea && shift => {
                        self.insert_at_cursor('\n');
                        if let Some(ref mut cb) = self.on_change { cb(&self.value); }
                        EventResult::Handled
                    }
                    KeyCode::Enter if self.textarea => {
                        if let Some(ref mut cb) = self.on_submit { cb(&self.value); }
                        // 提交后清空值（聊天场景的通用行为）
                        self.value.clear();
                        self.cursor_char = 0;
                        self.scroll_line.set(0);
                        self.selection.set(None);
                        EventResult::Handled
                    }
                    // 单行: Enter 提交
                    KeyCode::Enter => {
                        if let Some(ref mut cb) = self.on_submit { cb(&self.value); }
                        self.value.clear();
                        self.cursor_char = 0;
                        EventResult::Handled
                    }
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
                            let chars: Vec<char> = self.value.chars().collect();
                            let len = chars.len();
                            if self.cursor_char > len { self.cursor_char = len; }
                            if self.cursor_char == 0 { return EventResult::NotHandled; }
                            let byte_start: usize = chars[..self.cursor_char - 1].iter().map(|c| c.len_utf8()).sum();
                            let byte_end = byte_start + chars[self.cursor_char - 1].len_utf8();
                            self.value.replace_range(byte_start..byte_end, "");
                            self.cursor_char -= 1;
                        } else { return EventResult::NotHandled; }
                        if let Some(ref mut cb) = self.on_change { cb(&self.value); }
                        EventResult::Handled
                    }
                    KeyCode::Delete => {
                        if self.selection.get().is_some() { self.delete_selection(); }
                        else {
                            let chars: Vec<char> = self.value.chars().collect();
                            let len = chars.len();
                            if self.cursor_char > len { self.cursor_char = len; }
                            if self.cursor_char < len {
                                let byte_start: usize = chars[..self.cursor_char].iter().map(|c| c.len_utf8()).sum();
                                let byte_end = byte_start + chars[self.cursor_char].len_utf8();
                                self.value.replace_range(byte_start..byte_end, "");
                            } else { return EventResult::NotHandled; }
                        }
                        if let Some(ref mut cb) = self.on_change { cb(&self.value); }
                        EventResult::Handled
                    }
                    KeyCode::Left => { self.move_cursor_left(ctrl); EventResult::Handled }
                    KeyCode::Right => { self.move_cursor_right(ctrl); EventResult::Handled }
                    KeyCode::Up if self.textarea => { self.move_cursor_up(); EventResult::Handled }
                    KeyCode::Down if self.textarea => { self.move_cursor_down(); EventResult::Handled }
                    KeyCode::Home => {
                        if !shift { self.selection.set(None); }
                        else { self.set_selection_range(self.sel_anchor.get(), 0); }
                        self.cursor_char = 0;
                        if !shift { self.sel_anchor.set(0); }
                        EventResult::Handled
                    }
                    KeyCode::End => {
                        let len = self.value.chars().count();
                        if !shift { self.selection.set(None); }
                        else { self.set_selection_range(self.sel_anchor.get(), len); }
                        self.cursor_char = len;
                        if !shift { self.sel_anchor.set(len); }
                        EventResult::Handled
                    }
                    // Ctrl+V 粘贴（待实现：需要 platform clipboard read）
                    KeyCode::V if ctrl => EventResult::NotHandled,
                    _ => EventResult::NotHandled,
                }
            }
            WidgetEvent::KeyPress { text } => {
                if self.textarea {
                    // textarea 模式：允许 '\n', '\r' 等
                    let chars: Vec<char> = text.chars().filter(|&c| c >= ' ' || c == '\n' || c == '\r').collect();
                    if chars.is_empty() { return EventResult::NotHandled; }
                    if self.selection.get().is_some() { self.delete_selection(); }
                    for ch in &chars {
                        let byte_pos = self.value.char_indices().nth(self.cursor_char).map(|(i, _)| i).unwrap_or(self.value.len());
                        self.value.insert(byte_pos, *ch);
                        self.cursor_char += 1;
                    }
                } else {
                    // 单行模式：过滤控制字符
                    let chars: Vec<char> = text.chars().filter(|c| !c.is_control()).collect();
                    if chars.is_empty() { return EventResult::NotHandled; }
                    if self.selection.get().is_some() { self.delete_selection(); }
                    for ch in &chars {
                        let byte_pos = self.value.char_indices().nth(self.cursor_char).map(|(i, _)| i).unwrap_or(self.value.len());
                        self.value.insert(byte_pos, *ch);
                        self.cursor_char += 1;
                    }
                }
                self.sel_anchor.set(self.cursor_char);
                if let Some(ref mut cb) = self.on_change { cb(&self.value); }
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if self.textarea {
            self.render_textarea(frame, ctx);
        } else {
            self.render_singleline(frame, ctx);
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 多行渲染
// ════════════════════════════════════════════════════════════════════════════

impl Input {
    fn render_textarea(&self, frame: Rect, ctx: &mut RenderContext) {
        let fill_tertiary = ctx.tokens().color_fill_tertiary();
        let border_color = ctx.tokens().color_border();
        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let text_color = ctx.tokens().color_text();
        let text_tertiary = ctx.tokens().color_text_tertiary();
        let border_radius_sm = ctx.tokens().border_radius_sm();

        let inner_frame = Rect::new(frame.x, frame.y, frame.w, frame.h);

        let (bg, border) = if self.disabled {
            (fill_tertiary, border_color)
        } else if self.focused {
            (ctx.tokens().color_bg_elevated(), primary)
        } else if self.hovered {
            (ctx.tokens().color_bg_elevated(), primary_hover)
        } else {
            (ctx.tokens().color_bg_container(), border_color)
        };

        let radius = Some(Radius::uniform(border_radius_sm));
        ctx.fill_rect(inner_frame, bg, radius);
        ctx.stroke_rect(inner_frame, border, if self.focused { 2.0 } else { 1.0 }, radius);

        let text_area = Rect::new(inner_frame.x + PAD, inner_frame.y + 6.0,
            (inner_frame.w - PAD * 2.0).max(20.0), (inner_frame.h - 12.0).max(20.0));
        ctx.canvas_2d().push_clip(text_area);

        let display_text = if self.value.is_empty() && !self.focused { &self.placeholder } else { &self.value };
        let disp_color = if self.value.is_empty() && !self.focused { text_tertiary } else { text_color };

        let lines: Vec<&str> = if display_text == &self.placeholder {
            vec![self.placeholder.as_str()]
        } else {
            display_text.lines().collect()
        };

        // 计算光标所在行
        let cursor_line = self.cursor_line_col().0;

        // 垂直滚动：确保光标行可见
        let vis_lines = (text_area.h / LINE_HEIGHT) as usize;
        let scroll_line = self.scroll_line.get();
        let adj_scroll = if cursor_line >= scroll_line + vis_lines {
            cursor_line.saturating_sub(vis_lines).saturating_add(1)
        } else if cursor_line < scroll_line {
            cursor_line
        } else {
            scroll_line
        };
        self.scroll_line.set(adj_scroll);

        // 逐行绘制
        let line_h = LINE_HEIGHT;
        let mut y = text_area.y;
        for (li, line) in lines.iter().enumerate() {
            if li < adj_scroll {
                continue;
            }
            if y + line_h > text_area.y + text_area.h {
                break;
            }
            // 计算该行的字符范围
            let line_start: usize = lines[..li].iter().map(|s| s.chars().count()).sum();
            // 加上换行符的数量
            let line_start = line_start + li; // each '\n' adds 1 char
            let line_end = line_start + line.chars().count();

            // 选中高亮
            if let Some((sel_s, sel_e)) = self.selection.get() {
                if sel_s < sel_e && sel_s < line_end && sel_e > line_start {
                    let sel_in_line_start = if sel_s > line_start { sel_s - line_start } else { 0 };
                    let sel_in_line_end = if sel_e < line_end { sel_e - line_start } else { line.chars().count() };
                    // 估算选中区域的 x 位置
                    let before_sel: String = line.chars().take(sel_in_line_start).collect();
                    let sel_text: String = line.chars().skip(sel_in_line_start).take(sel_in_line_end - sel_in_line_start).collect();
                    let x0 = text_area.x + ctx.measure_text(&before_sel, FONT_SIZE).w;
                    let sel_w = ctx.measure_text(&sel_text, FONT_SIZE).w;
                    ctx.fill_rect(Rect::new(x0, y, sel_w, line_h),
                        primary.with_alpha(64), None);
                }
            }

            ctx.draw_text(line, Point::new(text_area.x, y + 2.0), disp_color, FONT_SIZE);

            // 光标（在当前行且 focused）
            if self.focused && self.selection.get().is_none() && li == cursor_line {
                let col = self.cursor_line_col().1;
                let before: String = line.chars().take(col).collect();
                let cx = text_area.x + ctx.measure_text(&before, FONT_SIZE).w;
                ctx.fill_rect(Rect::new(cx, y + 2.0, 1.5, line_h - 4.0), primary, None);
            }

            y += line_h;
        }

        // 如果没有任何行且 focused，在顶部画光标
        if self.focused && lines.is_empty() {
            ctx.fill_rect(Rect::new(text_area.x, text_area.y + 2.0, 1.5, line_h - 4.0), primary, None);
        }

        ctx.canvas_2d().pop_clip();
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 单行渲染（原逻辑精简）
// ════════════════════════════════════════════════════════════════════════════

impl Input {
    fn render_singleline(&self, frame: Rect, ctx: &mut RenderContext) {
        let h = input_height(self.input_size).min(frame.h);
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
            (ctx.tokens().color_bg_elevated(), primary, text_color_token)
        } else if self.hovered {
            (ctx.tokens().color_bg_elevated(), primary_hover, text_color_token)
        } else {
            (ctx.tokens().color_bg_container(), border_color, text_color_token)
        };

        let radius = Some(Radius::uniform(border_radius_sm));
        ctx.fill_rect(inner_frame, bg, radius);
        ctx.stroke_rect(inner_frame, border, if self.focused { 2.0 } else { 1.0 }, radius);

        if !self.prefix.is_empty() {
            let px = inner_frame.x + 6.0;
            let py = ctx.visual_center_y(inner_frame, 12.0);
            let icon_str = crate::widgets::icon::icon_char(&self.prefix);
            let saved = *ctx.font();
            if let Some(fh) = crate::widgets::icon::lucide_handle() { ctx.set_font(fh); }
            ctx.draw_text(icon_str, Point::new(px, py), text_sec, 12.0);
            ctx.set_font(saved);
        }

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
        ctx.canvas_2d().push_clip(text_area);

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

        ctx.canvas_2d().pop_clip();

        if self.focused && self.selection.get().is_none() {
            let cursor_x = text_area_x + text_before_w - scroll_off;
            ctx.fill_rect(Rect::new(cursor_x, inner_frame.y + 4.0, 1.5, inner_frame.h - 8.0), primary, None);
        }

        if self.clearable && !self.value.is_empty() && self.focused {
            let cx = inner_frame.x + inner_frame.w - 20.0;
            let cy = ctx.visual_center_y(inner_frame, 12.0);
            ctx.draw_text("✕", Point::new(cx, cy), text_sec, 12.0);
        }

        if self.password {
            let px = inner_frame.x + inner_frame.w - pwd_w;
            let py = ctx.visual_center_y(inner_frame, 12.0);
            ctx.draw_text(if self.password_visible { "◎" } else { "◉" }, Point::new(px, py), text_sec, 14.0);
        }

        if self.search {
            let sx = inner_frame.x + inner_frame.w - search_w;
            let sy = ctx.visual_center_y(inner_frame, 12.0);
            ctx.draw_text("🔍", Point::new(sx, sy), text_sec, 12.0);
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 公共方法
// ════════════════════════════════════════════════════════════════════════════

impl Input {
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            value: String::new(), placeholder: placeholder.into(),
            input_size: InputSize::Medium, disabled: false, focused: false, hovered: false,
            cursor_char: 0, scroll_offset_x: Cell::new(0.0), scroll_line: Cell::new(0),
            glyph_xs: RefCell::new(Vec::new()),
            selection: Cell::new(None), sel_anchor: Cell::new(0), sel_dragging: Cell::new(false),
            prefix: String::new(), suffix: String::new(), addon_before: String::new(), addon_after: String::new(),
            password: false, password_visible: false, clearable: false, search: false,
            textarea: false, textarea_rows: 3, on_change: None, on_submit: None,
        }
    }
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into(); self.cursor_char = self.value.chars().count(); self.scroll_offset_x.set(0.0); self
    }
    pub fn size(mut self, s: InputSize) -> Self { self.input_size = s; self }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
    pub fn value(&self) -> &str { &self.value }
    pub fn set_value(&mut self, v: impl Into<String>) { self.value = v.into(); self.cursor_char = self.value.chars().count(); self.scroll_offset_x.set(0.0); self.scroll_line.set(0); self.selection.set(None); }
    /// 设置输入框的聚焦状态（供 tree.build 后恢复焦点用）
    pub fn set_focused(&mut self, v: bool) { self.focused = v; }
    pub fn prefix(mut self, s: &str) -> Self { self.prefix = s.to_string(); self }
    pub fn suffix(mut self, s: &str) -> Self { self.suffix = s.to_string(); self }
    pub fn addon_before(mut self, s: &str) -> Self { self.addon_before = s.to_string(); self }
    pub fn addon_after(mut self, s: &str) -> Self { self.addon_after = s.to_string(); self }
    pub fn password(mut self, v: bool) -> Self { self.password = v; self }
    pub fn clearable(mut self, v: bool) -> Self { self.clearable = v; self }
    pub fn search(mut self, v: bool) -> Self { self.search = v; self }
    pub fn textarea(mut self, v: bool) -> Self { self.textarea = v; self.textarea_rows = if v { 3 } else { 0 }; self }
    pub fn textarea_rows(mut self, n: usize) -> Self { self.textarea_rows = n; self }
    pub fn on_change<F: FnMut(&str) + 'static>(mut self, f: F) -> Self { self.on_change = Some(Box::new(f)); self }
    pub fn on_submit<F: FnMut(&str) + 'static>(mut self, f: F) -> Self { self.on_submit = Some(Box::new(f)); self }

    // ── 内部：光标移动 ──

    fn move_cursor_left(&mut self, ctrl: bool) {
        self.selection.set(None);
        if ctrl {
            // 跳到前一个单词
            let chars: Vec<char> = self.value.chars().collect();
            let mut pos = self.cursor_char.min(chars.len());
            if pos > 0 { pos -= 1; }
            while pos > 0 && chars[pos] == ' ' { pos -= 1; }
            while pos > 0 && chars[pos - 1] != ' ' { pos -= 1; }
            self.cursor_char = pos;
        } else {
            if self.cursor_char > 0 { self.cursor_char -= 1; }
        }
        self.sel_anchor.set(self.cursor_char);
    }

    fn move_cursor_right(&mut self, ctrl: bool) {
        self.selection.set(None);
        let len = self.value.chars().count();
        if ctrl {
            let chars: Vec<char> = self.value.chars().collect();
            let mut pos = self.cursor_char.min(chars.len());
            while pos < len && chars[pos] == ' ' { pos += 1; }
            while pos < len && chars[pos] != ' ' { pos += 1; }
            self.cursor_char = pos;
        } else {
            if self.cursor_char < len { self.cursor_char += 1; }
        }
        self.sel_anchor.set(self.cursor_char);
    }

    fn move_cursor_up(&mut self) {
        let (line, col) = self.cursor_line_col();
        if line == 0 { return; }
        let lines: Vec<&str> = self.value.lines().collect();
        let prev_line = lines[line - 1];
        let col = col.min(prev_line.chars().count());
        // 计算光标位置：之前所有行的字符数 + 换行符数 + col
        let prev_chars: usize = lines[..line - 1].iter().map(|s| s.chars().count()).sum();
        self.cursor_char = prev_chars + (line - 1) + col; // + (line-1) for newlines
        self.sel_anchor.set(self.cursor_char);
        self.selection.set(None);
    }

    fn move_cursor_down(&mut self) {
        let (line, col) = self.cursor_line_col();
        let lines: Vec<&str> = self.value.lines().collect();
        if line + 1 >= lines.len() { return; }
        let next_line = lines[line + 1];
        let col = col.min(next_line.chars().count());
        let prev_chars: usize = lines[..line + 1].iter().map(|s| s.chars().count()).sum();
        self.cursor_char = prev_chars + (line + 1) + col; // + (line+1) for newlines
        self.sel_anchor.set(self.cursor_char);
        self.selection.set(None);
    }

    /// 返回 (行号, 列号) 对应 cursor_char 的位置
    fn cursor_line_col(&self) -> (usize, usize) {
        let lines: Vec<&str> = self.value.lines().collect();
        let mut remaining = self.cursor_char;
        for (i, line) in lines.iter().enumerate() {
            let line_len = line.chars().count();
            // 每个换行符消耗 1 个字符位置（'\n'）
            if remaining <= line_len {
                return (i, remaining);
            }
            remaining -= line_len + 1; // +1 for the newline
        }
        (lines.len().saturating_sub(1), lines.last().map(|l| l.chars().count()).unwrap_or(0))
    }

    fn insert_at_cursor(&mut self, ch: char) {
        let byte_pos = self.value.char_indices().nth(self.cursor_char).map(|(i, _)| i).unwrap_or(self.value.len());
        self.value.insert(byte_pos, ch);
        self.cursor_char += 1;
    }

    fn char_at_x(&self, text_x: f32) -> usize {
        let xs = self.glyph_xs.borrow();
        if xs.is_empty() { return 0; }
        for (i, &gx) in xs.iter().enumerate() { if text_x < gx { return i; } }
        xs.len()
    }

    /// 多行模式下根据 (x, y) 找字符索引
    fn char_at_xy(&self, _x: f32, y: f32) -> usize {
        let lines: Vec<&str> = self.value.lines().collect();
        // y < 6.0 时（点击顶部 padding 区）映射到第 0 行，防止负数转 usize panic
        if y < 6.0 {
            return 0;
        }
        let line_idx = ((y - 6.0) / LINE_HEIGHT) as usize + self.scroll_line.get();
        let line_idx = line_idx.min(lines.len().saturating_sub(1));
        let prev: usize = lines[..line_idx].iter().map(|s| s.chars().count()).sum();
        prev + line_idx // + newlines before this line
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
