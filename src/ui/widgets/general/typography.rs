//! Typography widget — 排版组件（标题/段落/文本）。
//!
//! 支持 h1-h5 标题级别、段落文本、disabled/type 等变体。
//! 与 Label 的区别：Typography 提供语义化排版和更多样式选项。

use std::cell::Cell;
use std::cell::RefCell;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::clipboard;
use crate::ui::SnapshotFields;
use crate::ui::{
    ComponentId, EventResult, KeyCode, KeyMod, MouseButton, SemanticEvent, SystemEvent, WidgetTree,
};

use super::icon::Icon;

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
        spacing: f32,
        indent: f32,
        ellipsis: bool,
        glyph_xs: RefCell<Vec<f32>>,
        glyph_char_indices: RefCell<Vec<usize>>,
        /// 每行的 (相对 y, 字形数量)，用于 y 轴命中测试。
        line_info: RefCell<Vec<(f32, usize)>>,
        selection: Cell<Option<(usize, usize)>>,
        sel_anchor: Cell<usize>,
        sel_dragging: Cell<bool>,
        draw_pos: Cell<crate::core::Point>,
        copy_rect: Cell<Option<Rect>>,
        focused: bool,
        pending_submit: Cell<bool>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        let mut intrinsic = self.intrinsic_size();
        if matches!(self.type_, TypographyType::Paragraph)
            && constraints.max.w.is_finite()
            && constraints.max.w > 0.0
        {
            let (fs, _) = self.compute_font_style();
            let copy_space = if self.copyable { 28.0 } else { 0.0 };
            let text_width = (constraints.max.w - copy_space).max(1.0);
            let indent = self.paragraph_indent(fs).min(text_width);
            let wrap_width = (text_width - indent).max(1.0);
            let estimated = crate::draw::font::text_backend::estimate_text_metrics(
                &self.content,
                wrap_width,
                fs,
            );
            intrinsic = Size::new(
                if estimated.width_wrapped {
                    text_width + copy_space
                } else {
                    (estimated.max_line_width + indent).min(text_width) + copy_space
                },
                self.paragraph_line_height(fs) * estimated.line_count as f32,
            );
        }
        constraints.clamp(intrinsic)
    }

    tab_index => (&self) -> i32 { i32::from(self.copyable && !self.disabled) }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if self.disabled {
            return EventResult::NotHandled;
        }
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                mods,
            } => {
                if self.copyable && self.copy_rect.get().is_some_and(|rect| rect.contains(*pos)) {
                    self.copy_content(true);
                    return EventResult::Handled;
                }
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
            SystemEvent::PointerUp {
                button: MouseButton::Left,
                ..
            } => {
                self.sel_dragging.set(false);
                if let Some((s, e)) = self.selection.get() {
                    if s == e { self.selection.set(None); }
                }
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                // 失去焦点时清除文字选中
                self.focused = false;
                self.selection.set(None);
                self.sel_dragging.set(false);
                EventResult::Handled
            }
            SystemEvent::FocusIn if self.copyable => {
                self.focused = true;
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
                    KeyCode::Enter | KeyCode::Space if !ctrl && self.copyable => {
                        self.copy_content(true);
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_submit
            .replace(false)
            .then(|| SemanticEvent::submit(id, "copied"))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let (fs, fw) = self.compute_font_style();
        let text_c = self.color_override.unwrap_or_else(|| {
            if self.disabled { ctx.tokens().color_text_quaternary() } else { ctx.tokens().color_text() }
        });
        let copy_space = if self.copyable { 28.0 } else { 0.0 };
        let text_width = (frame.w - copy_space).max(1.0);
        let wraps = matches!(self.type_, TypographyType::Paragraph);
        let indent = if wraps {
            self.paragraph_indent(fs).min(text_width)
        } else {
            0.0
        };
        let layout_width = if wraps {
            (text_width - indent).max(1.0)
        } else {
            text_width
        };

        // 单次布局：同时用于 hit-test 缓存、选中背景和文字绘制
        let opts = crate::draw::TextLayoutOptions {
            max_width: layout_width,
            max_height: 0.0,
            line_height: if wraps {
                self.paragraph_line_height(fs)
            } else {
                fs * 1.5
            },
            word_wrap: wraps,
            h_align: crate::draw::HAlign::Left,
            v_align: crate::draw::VAlign::Top,
            font_size: fs,
        };
        let backend_opts = crate::draw::font::text_backend::TextLayoutOptions::from(opts);
        let fh = *ctx.font();
        let mut layout = ctx.font_service().layout_text(&fh, &self.content, &backend_opts);
        if indent > 0.0 {
            if let Some(first_line) = layout.lines.first_mut() {
                let end = (first_line.glyph_start + first_line.glyph_count).min(layout.glyphs.len());
                for glyph in &mut layout.glyphs[first_line.glyph_start..end] {
                    glyph.x += indent;
                }
                first_line.width += indent;
                layout.width = layout.width.max(first_line.width);
            }
        }

        let x = 0.0;
        let y = if wraps { 0.0 } else { ctx.visual_center_y(frame, fs) - frame.y };
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

            if self.mark || self.code {
                let background = if self.code {
                    ctx.tokens().color_fill_secondary()
                } else {
                    ctx.tokens().color_warning_bg()
                };
                for line in &layout.lines {
                    if let Some(bounds) = Self::line_bounds(&layout, line, abs_pos, fs) {
                        ctx.fill_rect(
                            Rect::new(
                                bounds.x - 3.0,
                                bounds.y - 1.0,
                                bounds.w + 6.0,
                                bounds.h + 2.0,
                            ),
                            background,
                            Some(Radius::uniform(if self.code { 3.0 } else { 1.0 })),
                        );
                    }
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
            if self.strong || fw >= 600.0 {
                ctx.blit_glyph_layout(
                    &layout,
                    Point::new(abs_pos.x + 0.6, abs_pos.y),
                    text_c,
                    fs,
                );
            }
            if self.underline || self.delete {
                for line in &layout.lines {
                    if let Some(bounds) = Self::line_bounds(&layout, line, abs_pos, fs) {
                        if self.underline {
                            ctx.draw_line(
                                bounds.x,
                                bounds.y + bounds.h - 1.0,
                                bounds.x + bounds.w,
                                bounds.y + bounds.h - 1.0,
                                text_c,
                                1.0,
                            );
                        }
                        if self.delete {
                            ctx.draw_line(
                                bounds.x,
                                bounds.y + bounds.h * 0.52,
                                bounds.x + bounds.w,
                                bounds.y + bounds.h * 0.52,
                                text_c,
                                1.0,
                            );
                        }
                    }
                }
            }
        }

        // copyable 图标
        if self.copyable {
            let copy_x = frame.x + (frame.w - 24.0).max(0.0);
            let copy_y = if wraps { frame.y } else { ctx.visual_center_y(frame, 14.0) };
            self.copy_rect.set(Some(Rect::new(
                copy_x - frame.x,
                copy_y - frame.y,
                24.0,
                20.0,
            )));
            if self.focused && tree.keyboard_focus_visible() {
                ctx.stroke_rect(
                    Rect::new(copy_x - 2.0, copy_y - 2.0, 24.0, 20.0),
                    ctx.tokens().color_primary(),
                    1.0,
                    Some(Radius::uniform(3.0)),
                );
            }
            let copy_color = ctx.tokens().color_text_quaternary();
            Icon::paint_in_frame(
                ctx,
                "copy",
                Rect::new(copy_x - 2.0, copy_y - 2.0, 24.0, 20.0),
                copy_color,
                14.0,
            );
        } else {
            self.copy_rect.set(None);
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
            spacing: 0.0,
            indent: 0.0,
            ellipsis: false,
            glyph_xs: RefCell::new(Vec::new()),
            glyph_char_indices: RefCell::new(Vec::new()),
            line_info: RefCell::new(Vec::new()),
            selection: Cell::new(None),
            sel_anchor: Cell::new(0),
            sel_dragging: Cell::new(false),
            draw_pos: Cell::new(crate::core::Point::new(0.0, 0.0)),
            copy_rect: Cell::new(None),
            focused: false,
            pending_submit: Cell::new(false),
        }
    }
    pub fn heading(content: &str, level: u8) -> Self {
        Self::new(content, Self::type_for_level(level))
    }

    pub fn title(content: &str) -> Self {
        Self::heading(content, 1)
    }

    pub fn level(mut self, level: u8) -> Self {
        self.type_ = Self::type_for_level(level);
        self
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

    pub fn spacing(mut self, value: f32) -> Self {
        self.spacing = if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        };
        self
    }

    pub fn indent(mut self, value: f32) -> Self {
        self.indent = if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        };
        self
    }

    pub fn ellipsis(mut self) -> Self {
        self.ellipsis = true;
        self
    }

    pub fn selected_text(&self) -> Option<String> {
        self.selection.get().map(|(s, e)| self.slice_range(s, e))
    }

    pub fn is_copy_focused(&self) -> bool {
        self.focused
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

    fn type_for_level(level: u8) -> TypographyType {
        match level.clamp(1, 5) {
            1 => TypographyType::Heading1,
            2 => TypographyType::Heading2,
            3 => TypographyType::Heading3,
            4 => TypographyType::Heading4,
            _ => TypographyType::Heading5,
        }
    }

    fn paragraph_line_height(&self, font_size: f32) -> f32 {
        let factor = if self.spacing > 0.0 {
            self.spacing
        } else {
            1.5
        };
        let line_height = font_size * factor;
        if line_height.is_finite() && line_height > 0.0 {
            line_height
        } else {
            font_size * 1.5
        }
    }

    fn paragraph_indent(&self, font_size: f32) -> f32 {
        self.indent * font_size
    }

    fn intrinsic_size(&self) -> Size {
        let (fs, _fw) = self.compute_font_style();
        let copy_space = if self.copyable { 28.0 } else { 0.0 };
        let w = self.content.chars().count() as f32 * fs * 0.6 + copy_space;
        // 与 Label 一致：单行用视觉字高，避免光学居中后量高偏大
        let h = fs * 1.2;
        Size::new(w, h)
    }

    fn copy_content(&self, emit_submit: bool) {
        clipboard::copy_to_clipboard(&self.content);
        if emit_submit {
            self.pending_submit.set(true);
        }
    }

    fn line_bounds(
        layout: &crate::draw::font::text_backend::TextLayout,
        line: &crate::draw::font::text_backend::LineInfo,
        origin: Point,
        font_size: f32,
    ) -> Option<Rect> {
        let start = line.glyph_start;
        let end = (start + line.glyph_count).min(layout.glyphs.len());
        let glyphs = layout.glyphs.get(start..end)?;
        let first = glyphs.first()?;
        let last = glyphs.last()?;
        Some(Rect::new(
            origin.x + first.x,
            origin.y + line.y,
            (last.x + last.width - first.x).max(0.0),
            line.height.max(font_size * 1.2),
        ))
    }

    #[cfg(test)]
    pub(crate) fn copy_rect_for_test(&self) -> Option<Rect> {
        self.copy_rect.get()
    }

    #[cfg(test)]
    pub(crate) fn rendered_line_origins_for_test(&self) -> Vec<Point> {
        let glyph_xs = self.glyph_xs.borrow();
        let mut glyph_offset = 0usize;
        self.line_info
            .borrow()
            .iter()
            .map(|(y, glyph_count)| {
                let x = glyph_xs.get(glyph_offset).copied().unwrap_or_default();
                glyph_offset += *glyph_count;
                Point::new(x, *y)
            })
            .collect()
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
        self.spacing = next.spacing;
        self.indent = next.indent;
        self.ellipsis = next.ellipsis;
        if !self.copyable || self.disabled {
            self.focused = false;
            self.copy_rect.set(None);
            self.pending_submit.set(false);
        }
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
