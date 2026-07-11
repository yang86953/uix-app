//! Typography widget — 排版组件（标题/段落/文本）。
//!
//! 支持 h1-h5 标题级别、段落文本、disabled/type 等变体。
//! 与 Label 的区别：Typography 提供语义化排版和更多样式选项。

use std::cell::Cell;
use std::cell::RefCell;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::ui::clipboard;
use crate::ui::SnapshotFields;
use crate::ui::{EventResult, KeyCode, KeyMod, SystemEvent, WidgetTree};

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

component! {
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
        glyph_char_indices: RefCell<Vec<usize>>,
        /// 每行的 (相对 y, 字形数量)，用于 y 轴命中测试。
        line_info: RefCell<Vec<(f32, usize)>>,
        selection: Cell<Option<(usize, usize)>>,
        sel_anchor: Cell<usize>,
        sel_dragging: Cell<bool>,
        draw_pos: Cell<crate::core::Point>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown { pos, mods, .. } => {
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
            SystemEvent::PointerMove { pos, .. } => {
                if !self.sel_dragging.get() { return EventResult::NotHandled; }
                let dp = self.draw_pos.get();
                let text_x = pos.x - dp.x;
                let text_y = pos.y - dp.y;
                let ci = self.char_at_xy(text_x, text_y);
                let anchor = self.sel_anchor.get();
                self.set_selection_range(anchor, ci);
                EventResult::Handled
            }
            SystemEvent::PointerUp { .. } => {
                self.sel_dragging.set(false);
                if let Some((s, e)) = self.selection.get() {
                    if s == e { self.selection.set(None); }
                }
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                // 失去焦点时清除文字选中
                self.selection.set(None);
                self.sel_dragging.set(false);
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, mods } => {
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

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let (fs, _fw) = self.compute_font_style();
        let text_c = self.color_override.unwrap_or_else(|| {
            if self.disabled { ctx.tokens().color_text_quaternary() } else { ctx.tokens().color_text() }
        });

        // 单次布局：同时用于 hit-test 缓存、选中背景和文字绘制
        let opts = crate::draw::TextLayoutOptions {
            max_width: frame.w.max(1.0),
            max_height: 0.0,
            line_height: fs * 1.5,
            word_wrap: false,
            h_align: crate::draw::HAlign::Left,
            v_align: crate::draw::VAlign::Top,
            font_size: fs,
        };
        let backend_opts = crate::draw::font::text_backend::TextLayoutOptions::from(opts);
        let fh = *ctx.font();
        let layout = ctx.font_service().layout_text(&fh, &self.content, &backend_opts);

        let x = 0.0;
        let y = ctx.visual_center_y(frame, fs) - frame.y;
        let draw_pos = crate::core::Point::new(x, y);
        self.draw_pos.set(draw_pos);
        let abs_pos = crate::core::Point::new(frame.x + draw_pos.x, frame.y + draw_pos.y);

        if !self.content.is_empty() {
            {
                let mut xs = self.glyph_xs.borrow_mut();
                let mut cis = self.glyph_char_indices.borrow_mut();
                xs.clear();
                cis.clear();
                for g in &layout.glyphs {
                    xs.push(g.x);
                    cis.push(g.char_index);
                }
            }

            {
                let mut li = self.line_info.borrow_mut();
                li.clear();
                for l in &layout.lines {
                    li.push((l.y, l.glyph_count));
                }
            }

            if let Some((sel_s, sel_e)) = self.selection.get() {
                if sel_s < sel_e {
                    let visual_h = ctx.font_service()
                        .horizontal_line_metrics(&fh, fs)
                        .map(|m| m.ascent + m.descent)
                        .unwrap_or(fs * 1.2);
                    for line in &layout.lines {
                        let gs = line.glyph_start;
                        let ge = (gs + line.glyph_count).min(layout.glyphs.len());
                        let glyphs: Vec<_> = layout.glyphs[gs..ge]
                            .iter()
                            .filter(|g| g.char_index >= sel_s && g.char_index < sel_e)
                            .collect();
                        if glyphs.is_empty() {
                            continue;
                        }
                        let x0 = abs_pos.x + glyphs[0].x;
                        let last = glyphs[glyphs.len() - 1];
                        let x1 = abs_pos.x + last.x + last.width.max(0.0);
                        let y0 = abs_pos.y + line.y;
                        ctx.fill_rect(
                            Rect::new(x0, y0, (x1 - x0).max(0.0), visual_h),
                            ctx.tokens().color_primary().with_alpha(64),
                            None,
                        );
                    }
                }
            }

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
            mark: false,
            code: false,
            underline: false,
            delete: false,
            strong: false,
            italic: false,
            copyable: false,
            color_override: None,
            glyph_xs: RefCell::new(Vec::new()),
            glyph_char_indices: RefCell::new(Vec::new()),
            line_info: RefCell::new(Vec::new()),
            selection: Cell::new(None),
            sel_anchor: Cell::new(0),
            sel_dragging: Cell::new(false),
            draw_pos: Cell::new(crate::core::Point::new(0.0, 0.0)),
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
    pub fn paragraph(content: &str) -> Self {
        Self::new(content, TypographyType::Paragraph)
    }
    pub fn text(content: &str) -> Self {
        Self::new(content, TypographyType::Text)
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn mark(mut self) -> Self {
        self.mark = true;
        self
    }
    pub fn code(mut self) -> Self {
        self.code = true;
        self
    }
    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }
    pub fn delete(mut self) -> Self {
        self.delete = true;
        self
    }
    pub fn strong(mut self) -> Self {
        self.strong = true;
        self
    }
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
    pub fn color(mut self, c: Color) -> Self {
        self.color_override = Some(c);
        self
    }
    pub fn copyable(mut self, v: bool) -> Self {
        self.copyable = v;
        self
    }

    pub fn selected_text(&self) -> Option<String> {
        self.selection.get().map(|(s, e)| self.slice_range(s, e))
    }

    pub(crate) fn is_cross_text_dragging(&self) -> bool {
        self.sel_dragging.get()
    }

    pub(crate) fn cross_text_len(&self) -> usize {
        self.content.chars().count()
    }

    pub(crate) fn cross_text_anchor(&self) -> usize {
        self.sel_anchor.get()
    }

    pub(crate) fn set_cross_text_range(&self, range: Option<(usize, usize)>) {
        match range {
            Some((a, b)) if a != b => self.selection.set(Some((a.min(b), a.max(b)))),
            _ => self.selection.set(None),
        }
    }

    pub(crate) fn cross_text_char_at(&self, frame_local: crate::core::Point) -> usize {
        let dp = self.draw_pos.get();
        self.char_at_xy(frame_local.x - dp.x, frame_local.y - dp.y)
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

    fn intrinsic_size(&self) -> Size {
        let (fs, _fw) = self.compute_font_style();
        let w = self.content.chars().count() as f32 * fs * 0.6;
        // 与 Label 一致：单行用视觉字高，避免光学居中后量高偏大
        let h = fs * 1.2;
        Size::new(w, h)
    }

    fn char_at_xy(&self, text_x: f32, text_y: f32) -> usize {
        let xs = self.glyph_xs.borrow();
        let cis = self.glyph_char_indices.borrow();
        let li = self.line_info.borrow();
        if xs.is_empty() {
            return 0;
        }
        if li.is_empty() {
            for i in 0..xs.len() {
                if text_x < xs[i] {
                    return cis.get(i).copied().unwrap_or(i);
                }
            }
            return cis
                .last()
                .map(|c| c + 1)
                .unwrap_or(self.content.chars().count());
        }
        let mut target_y = text_y;
        let first_ly = li.first().map(|(ly, _)| *ly).unwrap_or(0.0);
        if target_y < first_ly {
            target_y = first_ly;
        }
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
        let end = (global_off + line_gc).min(xs.len());
        for i in global_off..end {
            if text_x < xs[i] {
                return cis.get(i).copied().unwrap_or(i);
            }
        }
        if end > 0 {
            cis.get(end - 1).map(|c| c + 1).unwrap_or(end)
        } else {
            0
        }
    }

    fn set_selection_range(&self, a: usize, b: usize) {
        if a == b {
            self.selection.set(None);
        } else {
            self.selection.set(Some((a.min(b), a.max(b))));
        }
    }

    fn slice_range(&self, start_char: usize, end_char: usize) -> String {
        let chars: Vec<char> = self.content.chars().collect();
        let e = end_char.min(chars.len());
        let s = start_char.min(e);
        chars[s..e].iter().collect()
    }
}

impl Typography {
    pub(crate) fn sync_from(&mut self, next: Self) {
        if self.content != next.content {
            self.content = next.content;
            self.glyph_xs.borrow_mut().clear();
            self.glyph_char_indices.borrow_mut().clear();
            self.line_info.borrow_mut().clear();
            self.selection.set(None);
            self.sel_anchor.set(0);
            self.sel_dragging.set(false);
        }
        self.type_ = next.type_;
        self.disabled = next.disabled;
        self.mark = next.mark;
        self.code = next.code;
        self.underline = next.underline;
        self.delete = next.delete;
        self.strong = next.strong;
        self.italic = next.italic;
        self.copyable = next.copyable;
        self.color_override = next.color_override;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Typography {
            content: self.content.clone(),
            type_: self.type_,
            disabled: self.disabled,
            mark: self.mark,
            code: self.code,
            underline: self.underline,
            delete: self.delete,
            strong: self.strong,
            italic: self.italic,
            copyable: self.copyable,
            color_override: self.color_override,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::traits::WidgetLayout;

    #[test]
    fn measure_clamps_typography_text_size() {
        let measured =
            Typography::heading("abcdef", 1).measure(Constraints::loose(Size::new(60.0, 40.0)));

        assert_eq!(measured, Size::new(60.0, 40.0));
    }
}
