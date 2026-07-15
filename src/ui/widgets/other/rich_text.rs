//! RichText widget — 富文本显示组件，支持样式分段和内联元素。
//!
//! # 设计
//!
//! 数据模型为 `RichTextSegment` 列表，每段独立控制样式和类型。
//! 支持普通文本（带样式）、内联代码、可点击链接、换行符。
//! 布局自动换行，按段测量、按行排布。
//!
//! # 交互
//!
//! - 文字选择：鼠标拖选（支持 Ctrl+A 全选、Ctrl+C 复制）
//! - 链接激活：鼠标或键盘提交 URL 语义事件，由应用决定导航策略
//! - 代码复制：悬停显示 📋 按钮，点击复制
//!
//! # 与 Label/Typography 的区别
//!
//! Label 和 Typography 面向单样式文本，RichText 面向混合样式段落，
//! 适合渲染 Markdown 风格的聊天消息、文档等富文本内容。

use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex};

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::spatial::PhysicalUnit;
use crate::draw::{Color, Radius};
use crate::ui::clipboard;
use crate::ui::{
    ComponentId, EventResult, KeyCode, KeyMod, MouseButton, SemanticEvent, SnapshotFields,
    SystemEvent, WidgetTree,
};

// ════════════════════════════════════════════════════════════════════════════
// 数据类型
// ════════════════════════════════════════════════════════════════════════════

/// 富文本段类型
///
/// 一个 RichText 由多个段组成，按顺序排列。
/// 支持普通文本（独立样式控制）、内联代码、链接、换行。
#[derive(Debug, Clone, PartialEq)]
pub enum RichTextSegment {
    /// 普通文本段（带独立样式）
    Text {
        content: String,
        style: RichTextStyle,
    },
    /// 内联代码（等宽字体 + 深色背景 + 圆角）
    Code { content: String },
    /// 可点击链接（自动带下划线和交互色）
    Link { content: String, url: String },
    /// 强制换行
    NewLine,
}

/// 文本样式
///
/// 所有字段可选，未设置的字段会继承默认样式。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RichTextStyle {
    /// 粗体
    pub bold: bool,
    /// 斜体
    pub italic: bool,
    /// 下划线
    pub underline: bool,
    /// 删除线
    pub strikethrough: bool,
    /// 字体大小（默认 14.0）
    pub font_size: Option<f32>,
    /// 文字颜色（默认使用主题色）
    pub color: Option<Color>,
    /// 背景色（仅文本段，非整个行）
    pub bg_color: Option<Color>,
}

impl RichTextStyle {
    /// 以当前样式为基础，叠加另一个样式的非 None 字段
    pub fn merge(&self, over: &Self) -> Self {
        Self {
            bold: over.bold || self.bold,
            italic: over.italic || self.italic,
            underline: over.underline || self.underline,
            strikethrough: over.strikethrough || self.strikethrough,
            font_size: over.font_size.or(self.font_size),
            color: over.color.or(self.color),
            bg_color: over.bg_color.or(self.bg_color),
        }
    }

    /// 解析文字颜色（合并 fallback）
    pub fn resolved_color(&self, fallback: Color) -> Color {
        self.color.unwrap_or(fallback)
    }

    /// 解析字体大小
    pub fn resolved_font_size(&self, default: f32) -> f32 {
        self.font_size.unwrap_or(default)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 布局引擎（子模块）
// ════════════════════════════════════════════════════════════════════════════
mod rich_text_layout;
pub(crate) use rich_text_layout::*;

// ════════════════════════════════════════════════════════════════════════════
// Widget
// ════════════════════════════════════════════════════════════════════════════

component! {
    /// 富文本显示组件
    ///
    /// 支持带样式的分段文本、内联代码、可点击链接、自动换行和文字选择。
    pub struct RichText {
        /// 富文本段列表
        pub segments: Vec<RichTextSegment>,

        /// 默认字体大小（像素）
        pub default_font_size: f32,
        /// 默认字体大小（物理单位，可选，优先级高于 default_font_size）
        pub default_font_size_unit: Option<PhysicalUnit>,

        /// 默认文字颜色
        pub default_color: Color,

        /// 布局行缓存
        layout_lines: RefCell<Vec<LayoutLine>>,

        /// 布局总高度缓存
        layout_height: Cell<f32>,

        /// 内容最大行宽（用于 measure 返回合理宽度）
        content_width: Cell<f32>,

        /// 上次布局使用的宽度（用于 measure 复用估计值）
        last_layout_width: Cell<f32>,

        /// 是否需要重新布局
        layout_dirty: Cell<bool>,

        // ── 选择状态 ──
        selection: Cell<Option<(usize, usize)>>,
        sel_anchor: Cell<usize>,
        sel_dragging: Cell<bool>,

        // ── 链接交互 ──
        hovered_link: Cell<Option<usize>>,
        focused: bool,
        focused_link: usize,
        pending_submit: RefCell<Option<String>>,

        // ── 代码块复制 ──
        code_regions: RefCell<Vec<CodeCopyRegion>>,
        hovered_code: Cell<Option<usize>>,
        /// 待复制的代码内容（外部主循环拉取）
        pub pending_copy: Arc<Mutex<Option<String>>>,
    }

    @new -> Self {
        Self {
            segments: Vec::new(),
            default_font_size: 14.0,
            default_font_size_unit: None,
            default_color: Color::from_rgb(200, 200, 200),
            layout_lines: RefCell::new(Vec::new()),
            layout_height: Cell::new(0.0),
            content_width: Cell::new(0.0),
            last_layout_width: Cell::new(0.0),
            layout_dirty: Cell::new(true),
            selection: Cell::new(None),
            sel_anchor: Cell::new(0),
            sel_dragging: Cell::new(false),
            hovered_link: Cell::new(None),
            focused: false,
            focused_link: 0,
            pending_submit: RefCell::new(None),
            code_regions: RefCell::new(Vec::new()),
            hovered_code: Cell::new(None),
            pending_copy: Arc::new(Mutex::new(None)),
        }
    }

    measure => (&self, constraints: Constraints) -> Size {
        // Use the explicit measure constraint first; fall back to the last
        // rendered width so first layout and render stay close.
        let est_width = if constraints.max.w.is_finite() && constraints.max.w > 0.0 {
            constraints.max.w
        } else if self.last_layout_width.get() > 0.0 {
            self.last_layout_width.get()
        } else {
            400.0_f32
        };

        if self.layout_dirty.get() || self.layout_height.get() <= 0.0 {
            let dpi = 96.0;
            let fs = self.resolved_font_size_px(dpi);
            let (_, total_h, max_w) = layout_rich_text(
                &self.segments, est_width, fs, self.default_color,
            );
            self.layout_height.set(total_h);
            self.content_width.set(max_w);
            self.layout_dirty.set(false);
        }
        constraints.clamp(Size::new(self.content_width.get(), self.layout_height.get()))
    }

    tab_index => (&self) -> i32 { i32::from(self.link_count() > 0) }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                // 代码块复制按钮点击
                for region in self.code_regions.borrow().iter() {
                    if region.rect.contains(*pos) {
                        if let Ok(mut pc) = self.pending_copy.lock() {
                            *pc = Some(region.content.clone());
                        }
                        return EventResult::Handled;
                    }
                }

                // 链接点击
                if let Some((segment_idx, url)) = self.link_at_pos(*pos) {
                    self.focused = true;
                    self.focused_link = self.link_ordinal(segment_idx).unwrap_or(0);
                    self.pending_submit.replace(Some(url));
                    self.selection.set(None);
                    self.sel_dragging.set(false);
                    return EventResult::Handled;
                }

                // 文字选择
                let lines = self.layout_lines.borrow();
                let char_idx = self.char_at_pos(*pos, &lines);
                self.selection.set(None);
                self.sel_anchor.set(char_idx);
                self.sel_dragging.set(true);
                EventResult::Handled
            }

            SystemEvent::PointerMove { pos, .. } => {
                let lines = self.layout_lines.borrow();

                // 代码块悬停
                let old_code = self.hovered_code.get();
                let mut new_code: Option<usize> = None;
                for (i, region) in self.code_regions.borrow().iter().enumerate() {
                    if region.rect.contains(*pos) { new_code = Some(i); break; }
                }
                self.hovered_code.set(new_code);
                if new_code != old_code { return EventResult::Handled; }

                let old_link = self.hovered_link.get();
                let new_link = self.link_at_pos(*pos).map(|(segment_idx, _)| segment_idx);
                self.hovered_link.set(new_link);
                if new_link != old_link { return EventResult::Handled; }

                // 选择拖拽
                if !self.sel_dragging.get() { return EventResult::NotHandled; }
                let char_idx = self.char_at_pos(*pos, &lines);
                let anchor = self.sel_anchor.get();
                self.set_selection_range(anchor, char_idx);
                EventResult::Handled
            }

            SystemEvent::PointerUp { .. } => {
                self.sel_dragging.set(false);
                if let Some((s, e)) = self.selection.get() {
                    if s == e { self.selection.set(None); }
                }
                EventResult::Handled
            }

            SystemEvent::PointerLeave => {
                let changed = self.hovered_code.get().is_some() || self.hovered_link.get().is_some();
                self.hovered_code.set(None);
                self.hovered_link.set(None);
                if changed { EventResult::Handled } else { EventResult::NotHandled }
            }

            SystemEvent::FocusIn if self.link_count() > 0 => {
                self.focused = true;
                self.focused_link = self.focused_link.min(self.link_count() - 1);
                EventResult::Handled
            }

            SystemEvent::FocusOut => {
                // 失去焦点时清除文字选中
                self.focused = false;
                self.selection.set(None);
                self.sel_dragging.set(false);
                EventResult::Handled
            }

            SystemEvent::KeyDown { key, mods } => {
                let ctrl = mods.contains(KeyMod::CTRL);
                match key {
                    KeyCode::A if ctrl => {
                        let total: usize = self.segments.iter()
                            .map(|s| match s {
                                RichTextSegment::Text { content, .. } => content.chars().count(),
                                RichTextSegment::Code { content } => content.chars().count(),
                                RichTextSegment::Link { content, .. } => content.chars().count(),
                                RichTextSegment::NewLine => 1,
                            }).sum();
                        self.sel_anchor.set(0);
                        self.set_selection_range(0, total);
                        EventResult::Handled
                    }
                    KeyCode::C if ctrl => {
                        if let Some((s, e)) = self.selection.get() {
                            let selected = self.extract_text_range(s, e);
                            clipboard::copy_to_clipboard(&selected);
                        }
                        EventResult::Handled
                    }
                    KeyCode::Left | KeyCode::Up if !ctrl && self.link_count() > 0 => {
                        self.focused_link = self.focused_link.saturating_sub(1);
                        EventResult::Handled
                    }
                    KeyCode::Right | KeyCode::Down if !ctrl && self.link_count() > 0 => {
                        self.focused_link = (self.focused_link + 1).min(self.link_count() - 1);
                        EventResult::Handled
                    }
                    KeyCode::Home if !ctrl && self.link_count() > 0 => {
                        self.focused_link = 0;
                        EventResult::Handled
                    }
                    KeyCode::End if !ctrl && self.link_count() > 0 => {
                        self.focused_link = self.link_count() - 1;
                        EventResult::Handled
                    }
                    KeyCode::Enter | KeyCode::Space if !ctrl => {
                        if let Some((_, url)) = self.link_at_ordinal(self.focused_link) {
                            self.pending_submit.replace(Some(url.to_string()));
                            EventResult::Handled
                        } else {
                            EventResult::NotHandled
                        }
                    }
                    _ => EventResult::NotHandled,
                }
            }

            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_submit
            .borrow_mut()
            .take()
            .map(|url| SemanticEvent::submit(id, url))
    }

    flex_grow => (&self) -> f32 { 1.0 }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let max_w = frame.w.max(1.0);

        // 布局缓存：仅在内容或宽度变化时重新布局，否则复用上次结果
        let need_relayout = self.layout_dirty.get()
            || (self.last_layout_width.get() - max_w).abs() > 0.5;

        let (layout_lines, _total_h, _max_line_w) = if need_relayout {
            let font = *ctx.font();
            let fs = self.resolved_font_size_px(ctx.dpi());
            let (lines, h, w) = layout_rich_text_real(
                &self.segments, max_w, fs, self.default_color,
                ctx.font_service(), &font,
            );
            let mut lines = lines;
            assign_global_indices(&mut lines, &self.segments);
            // 缓存相对坐标（y 从 0 开始），渲染时再加 frame.y
            self.layout_lines.replace(lines.clone());
            self.layout_height.set(h);
            self.content_width.set(w);
            self.last_layout_width.set(max_w);
            self.layout_dirty.set(false);
            (lines, h, w)
        } else {
            // 复用缓存
            let lines = self.layout_lines.borrow().clone();
            let h = self.layout_height.get();
            let w = self.content_width.get();
            (lines, h, w)
        };

        // 调整行位置到 frame 内（缓存存储的是相对 y）
        let mut layout_lines = layout_lines;
        for line in &mut layout_lines {
            line.y += frame.y;
        }

        let mut code_regions = self.code_regions.borrow_mut();
        code_regions.clear();

        // ── 逐行绘制 ──
        for line in &layout_lines {
            for glyph in &line.glyphs {
                let gx = frame.x + glyph.x;
                let gy = line.y + (line.height - glyph.font_size) * 0.5;

                // 背景色（代码段背景）
                if let Some(bg) = glyph.bg_color {
                    let pad = 2.0;
                    ctx.fill_rect(
                        Rect::new(gx - pad, gy - pad, glyph.width + pad * 2.0, glyph.font_size + pad * 2.0),
                        bg,
                        Some(Radius::uniform(3.0)),
                    );
                }

                // 选中背景
                if let Some((sel_s, sel_e)) = self.selection.get() {
                    if glyph.global_char_idx >= sel_s && glyph.global_char_idx < sel_e {
                        ctx.fill_rect(
                            Rect::new(gx, gy, glyph.width, glyph.font_size),
                            ctx.tokens().color_primary().with_alpha(64),
                            None,
                        );
                    }
                }

                let style = match self.segments.get(glyph.segment_idx) {
                    Some(RichTextSegment::Text { style, .. }) => Some(style),
                    _ => None,
                };
                let focused_link = self.focused
                    && self.link_segment_at_ordinal(self.focused_link) == Some(glyph.segment_idx);
                let active_link = self.hovered_link.get() == Some(glyph.segment_idx) || focused_link;
                if active_link {
                    ctx.fill_rect(
                        Rect::new(gx, gy, glyph.width, glyph.font_size),
                        ctx.tokens().color_primary().with_alpha(24),
                        None,
                    );
                }

                // 链接或显式下划线
                if glyph.is_link || style.is_some_and(|style| style.underline) {
                    ctx.fill_rect(
                        Rect::new(gx, gy + glyph.font_size * 0.95, glyph.width, 1.0),
                        if active_link { ctx.tokens().color_primary() } else { glyph.color.with_alpha(180) },
                        None,
                    );
                }
                if style.is_some_and(|style| style.strikethrough) {
                    ctx.fill_rect(
                        Rect::new(gx, gy + glyph.font_size * 0.52, glyph.width, 1.0),
                        glyph.color.with_alpha(180),
                        None,
                    );
                }
            }
        }

        // 按实际折行结果绘制连续 run，保证命中、选择与像素使用同一布局。
        for line in &layout_lines {
            let mut start = 0;
            while start < line.glyphs.len() {
                let segment_idx = line.glyphs[start].segment_idx;
                let mut end = start + 1;
                while end < line.glyphs.len() && line.glyphs[end].segment_idx == segment_idx {
                    end += 1;
                }
                let run = &line.glyphs[start..end];
                let content = run.iter().map(|glyph| glyph.ch).collect::<String>();
                let first = &run[0];
                let fs = first.font_size;
                let gx = frame.x + first.x;
                let gy = line.y + (line.height - fs) * 0.5
                    + if matches!(self.segments.get(segment_idx), Some(RichTextSegment::Code { .. })) { 2.0 } else { 0.0 };
                let focused_link = self.focused
                    && self.link_segment_at_ordinal(self.focused_link) == Some(segment_idx);
                let color = if first.is_link
                    && (self.hovered_link.get() == Some(segment_idx) || focused_link)
                {
                    ctx.tokens().color_primary()
                } else {
                    first.color
                };
                ctx.draw_text(&content, Point::new(gx, gy), color, fs);
                if matches!(
                    self.segments.get(segment_idx),
                    Some(RichTextSegment::Text { style, .. }) if style.bold
                ) {
                    ctx.draw_text(&content, Point::new(gx + 0.6, gy), color, fs);
                }
                start = end;
            }
        }

        // 每个代码段只登记一个复制按钮，折行时锚定最后一个可见字形。
        for (segment_idx, segment) in self.segments.iter().enumerate() {
            let RichTextSegment::Code { content } = segment else { continue; };
            let last = layout_lines.iter().rev().find_map(|line| {
                line.glyphs
                    .iter()
                    .rev()
                    .find(|glyph| glyph.segment_idx == segment_idx)
                    .map(|glyph| (line, glyph))
            });
            if let Some((line, glyph)) = last {
                let gx = frame.x + glyph.x + glyph.width;
                let gy = line.y + (line.height - glyph.font_size) * 0.5 + 2.0;
                let btn = Rect::new(gx - 4.0, gy, 24.0, 16.0);
                let cidx = code_regions.len();
                if self.hovered_code.get() == Some(cidx) {
                    ctx.fill_rect(btn, Color::from_rgb(55, 55, 62), Some(Radius::uniform(3.0)));
                    ctx.draw_text(
                        "📋",
                        Point::new(btn.x + 5.0, btn.y + 1.0),
                        Color::from_rgb(200, 200, 200),
                        10.0,
                    );
                }
                code_regions.push(CodeCopyRegion {
                    rect: Rect::new(btn.x - frame.x, btn.y - frame.y, btn.w, btn.h),
                    content: content.clone(),
                });
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 公有方法
// ════════════════════════════════════════════════════════════════════════════

impl Default for RichText {
    fn default() -> Self {
        Self::new()
    }
}

impl RichText {
    /// 获取解析后的默认字体大小（像素）。
    /// 如果设置了物理单位，通过 DPI 转换。
    pub fn resolved_font_size_px(&self, dpi: f32) -> f32 {
        self.default_font_size_unit
            .map(|u| u.to_dip(dpi))
            .unwrap_or(self.default_font_size)
    }

    /// 设置富文本内容
    pub fn content(mut self, segments: Vec<RichTextSegment>) -> Self {
        self.segments = segments;
        self.layout_dirty.set(true);
        self
    }

    /// 设置默认字体大小
    pub fn font_size(mut self, size: f32) -> Self {
        self.default_font_size = if size.is_finite() && size > 0.0 {
            size
        } else {
            14.0
        };
        self.default_font_size_unit = None;
        self.layout_dirty.set(true);
        self
    }

    /// 设置物理单位默认字体大小（优先级高于 `font_size()`）
    pub fn font_size_unit(mut self, unit: PhysicalUnit) -> Self {
        self.default_font_size_unit = Some(unit);
        self.layout_dirty.set(true);
        self
    }

    /// 设置默认文字颜色
    pub fn color(mut self, c: Color) -> Self {
        self.default_color = c;
        self.layout_dirty.set(true);
        self
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let segments_changed = self.segments != next.segments;
        let layout_config_changed = segments_changed
            || self.default_font_size != next.default_font_size
            || self.default_font_size_unit != next.default_font_size_unit
            || self.default_color != next.default_color;

        self.segments = next.segments;
        self.default_font_size = next.default_font_size;
        self.default_font_size_unit = next.default_font_size_unit;
        self.default_color = next.default_color;

        if layout_config_changed {
            self.layout_lines.borrow_mut().clear();
            self.layout_height.set(0.0);
            self.content_width.set(0.0);
            self.last_layout_width.set(0.0);
            self.code_regions.borrow_mut().clear();
            self.hovered_code.set(None);
            self.layout_dirty.set(true);
        }
        if segments_changed {
            self.selection.set(None);
            self.sel_anchor.set(0);
            self.sel_dragging.set(false);
            self.hovered_link.set(None);
            self.pending_submit.borrow_mut().take();
            self.focused_link = if self.link_count() == 0 {
                0
            } else {
                self.focused_link.min(self.link_count() - 1)
            };
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::RichText {
            segments: self.segments.clone(),
            default_font_size: self.default_font_size,
            default_font_size_unit: self.default_font_size_unit,
            default_color: self.default_color,
            focused_link: (self.link_count() > 0).then_some(self.focused_link),
        }
    }

    /// 返回当前键盘焦点链接的显示文本与 URL。
    pub fn focused_link(&self) -> Option<(&str, &str)> {
        self.link_at_ordinal(self.focused_link)
    }

    /// 获取选中的文本
    pub fn selected_text(&self) -> Option<String> {
        self.selection
            .get()
            .map(|(s, e)| self.extract_text_range(s, e))
    }

    pub(crate) fn is_cross_text_dragging(&self) -> bool {
        self.sel_dragging.get()
    }

    pub(crate) fn cross_text_len(&self) -> usize {
        self.segments
            .iter()
            .map(|s| match s {
                RichTextSegment::Text { content, .. } => content.chars().count(),
                RichTextSegment::Code { content } => content.chars().count(),
                RichTextSegment::Link { content, .. } => content.chars().count(),
                RichTextSegment::NewLine => 1,
            })
            .sum()
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

    pub(crate) fn cross_text_char_at(&self, frame_local: Point) -> usize {
        let lines = self.layout_lines.borrow();
        self.char_at_pos(frame_local, &lines)
    }

    /// 获取待复制的代码内容（由主循环调用）
    pub fn take_pending_copy(&self) -> Option<String> {
        self.pending_copy.lock().ok().and_then(|mut pc| pc.take())
    }

    #[cfg(test)]
    pub(crate) fn code_copy_rect_for_test(&self, index: usize) -> Option<Rect> {
        self.code_regions
            .borrow()
            .get(index)
            .map(|region| region.rect)
    }

    #[cfg(test)]
    pub(crate) fn layout_line_texts_for_test(&self) -> Vec<String> {
        self.layout_lines
            .borrow()
            .iter()
            .map(|line| line.glyphs.iter().map(|glyph| glyph.ch).collect())
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn link_point_for_test(&self, ordinal: usize) -> Option<Point> {
        let segment_idx = self.link_segment_at_ordinal(ordinal)?;
        self.layout_lines.borrow().iter().find_map(|line| {
            line.glyphs
                .iter()
                .find(|glyph| glyph.segment_idx == segment_idx)
                .map(|glyph| Point::new(glyph.x + glyph.width * 0.5, line.y + line.height * 0.5))
        })
    }

    // ── 内部 ──

    fn char_at_pos(&self, pos: Point, lines: &[LayoutLine]) -> usize {
        for line in lines {
            // 精确匹配当前行
            if pos.y < line.y || pos.y >= line.y + line.height {
                continue;
            }
            for glyph in &line.glyphs {
                let gx = glyph.x;
                if pos.x < gx + glyph.width * 0.5 {
                    return glyph.global_char_idx;
                }
            }
            if let Some(last) = line.glyphs.last() {
                return last.global_char_idx + 1;
            }
        }
        // 点在行间空白区域：找到最近的行
        if lines.is_empty() {
            return 0;
        }
        let mut best_line = 0;
        let mut best_dist = f32::MAX;
        for (i, line) in lines.iter().enumerate() {
            let closest_y = if pos.y < line.y {
                line.y
            } else {
                line.y + line.height
            };
            let dist = (pos.y - closest_y).abs();
            if dist < best_dist {
                best_dist = dist;
                best_line = i;
            }
        }
        // 返回最近行的末尾字符索引
        lines[best_line]
            .glyphs
            .last()
            .map(|g| g.global_char_idx + if pos.x > g.x + g.width * 0.5 { 1 } else { 0 })
            .unwrap_or(0)
    }

    fn link_count(&self) -> usize {
        self.segments
            .iter()
            .filter(|segment| {
                matches!(segment, RichTextSegment::Link { url, .. } if !url.trim().is_empty())
            })
            .count()
    }

    fn link_at_ordinal(&self, ordinal: usize) -> Option<(&str, &str)> {
        self.segments
            .iter()
            .filter_map(|segment| match segment {
                RichTextSegment::Link { content, url } if !url.trim().is_empty() => {
                    Some((content.as_str(), url.as_str()))
                }
                _ => None,
            })
            .nth(ordinal)
    }

    fn link_segment_at_ordinal(&self, ordinal: usize) -> Option<usize> {
        self.segments
            .iter()
            .enumerate()
            .filter_map(|(index, segment)| match segment {
                RichTextSegment::Link { url, .. } if !url.trim().is_empty() => Some(index),
                _ => None,
            })
            .nth(ordinal)
    }

    fn link_ordinal(&self, segment_idx: usize) -> Option<usize> {
        self.segments
            .iter()
            .enumerate()
            .filter(|(_, segment)| {
                matches!(segment, RichTextSegment::Link { url, .. } if !url.trim().is_empty())
            })
            .position(|(index, _)| index == segment_idx)
    }

    fn link_at_pos(&self, pos: Point) -> Option<(usize, String)> {
        let lines = self.layout_lines.borrow();
        for line in lines.iter() {
            if pos.y < line.y || pos.y >= line.y + line.height {
                continue;
            }
            for glyph in &line.glyphs {
                if glyph.is_link && pos.x >= glyph.x && pos.x < glyph.x + glyph.width {
                    if let Some(url) = glyph
                        .link_url
                        .as_deref()
                        .filter(|url| !url.trim().is_empty())
                    {
                        return Some((glyph.segment_idx, url.to_string()));
                    }
                }
            }
        }
        None
    }

    fn set_selection_range(&self, a: usize, b: usize) {
        if a == b {
            self.selection.set(None);
        } else {
            self.selection.set(Some((a.min(b), a.max(b))));
        }
    }

    fn extract_text_range(&self, start: usize, end: usize) -> String {
        let mut result = String::new();
        let mut offset: usize = 0;
        for segment in &self.segments {
            let seg_len = match segment {
                RichTextSegment::NewLine => 1,
                RichTextSegment::Text { content, .. } => content.chars().count(),
                RichTextSegment::Code { content } => content.chars().count(),
                RichTextSegment::Link { content, .. } => content.chars().count(),
            };
            let seg_start = offset;
            let seg_end = offset + seg_len;
            if seg_end > start && seg_start < end {
                let local_start = start.saturating_sub(seg_start);
                let local_end = if end < seg_end {
                    end - seg_start
                } else {
                    seg_len
                };
                let chars: Vec<char> = match segment {
                    RichTextSegment::NewLine => vec!['\n'],
                    RichTextSegment::Text { content, .. } => content.chars().collect(),
                    RichTextSegment::Code { content } => content.chars().collect(),
                    RichTextSegment::Link { content, .. } => content.chars().collect(),
                };
                let s: String = chars[local_start..local_end].iter().collect();
                result.push_str(&s);
            }
            offset = seg_end;
        }
        result
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 公共渲染辅助函数（供非 RichText widget 直接渲染富文本内容使用）
// ════════════════════════════════════════════════════════════════════════════

/// 计算富文本布局并返回尺寸信息
///
/// 可用于非 RichText widget 中直接测量富文本尺寸。
/// 返回 (总高度, 总字符数, 最大行宽)。
pub fn layout_rich_text_segments(
    segments: &[RichTextSegment],
    max_width: f32,
    default_font_size: f32,
    default_color: Color,
) -> (f32, usize, f32) {
    let (_, total_h, max_w) =
        layout_rich_text(segments, max_width, default_font_size, default_color);
    let char_count: usize = segments
        .iter()
        .map(|s| match s {
            RichTextSegment::NewLine => 1,
            RichTextSegment::Text { content, .. } => content.chars().count(),
            RichTextSegment::Code { content } => content.chars().count(),
            RichTextSegment::Link { content, .. } => content.chars().count(),
        })
        .sum();
    (total_h, char_count, max_w)
}

/// 将纯文本内容解析为 RichTextSegment 列表（支持 ``` 围栏代码块和内联反引号 ` `）
///
/// 围栏代码块 → RichTextSegment::Code，内联反引号 → RichTextSegment::Code，其余文本 → RichTextSegment::Text。
pub fn parse_rich_text(content: &str) -> Vec<RichTextSegment> {
    let mut segments = Vec::new();
    let mut rest = content;
    while let Some(pos) = rest.find("```") {
        let before = &rest[..pos];
        if !before.is_empty() {
            parse_inline_text(before, &mut segments);
        }
        rest = &rest[pos + 3..];
        if let Some(end) = rest.find("```") {
            // 跳过语言标注行
            let code_start = rest.find('\n').map(|n| n + 1).unwrap_or(0);
            let code = if code_start < end {
                &rest[code_start..end]
            } else {
                &rest[..end]
            };
            segments.push(RichTextSegment::Code {
                content: code.to_string(),
            });
            rest = &rest[end + 3..];
        } else {
            parse_inline_text(rest, &mut segments);
            rest = "";
        }
    }
    if !rest.is_empty() {
        parse_inline_text(rest, &mut segments);
    }
    segments
}

/// 解析内联反引号 `code` 并将它们转为 RichTextSegment::Code
fn parse_inline_text(text: &str, segments: &mut Vec<RichTextSegment>) {
    let mut remaining = text;
    while let Some(start) = remaining.find('`') {
        let before = &remaining[..start];
        if !before.is_empty() {
            segments.push(RichTextSegment::Text {
                content: before.to_string(),
                style: RichTextStyle::default(),
            });
        }
        remaining = &remaining[start + 1..];
        if let Some(end) = remaining.find('`') {
            let code = &remaining[..end];
            segments.push(RichTextSegment::Code {
                content: code.to_string(),
            });
            remaining = &remaining[end + 1..];
        } else {
            // 不成对的反引号当作普通文本
            segments.push(RichTextSegment::Text {
                content: format!("`{}", remaining),
                style: RichTextStyle::default(),
            });
            remaining = "";
        }
    }
    if !remaining.is_empty() {
        segments.push(RichTextSegment::Text {
            content: remaining.to_string(),
            style: RichTextStyle::default(),
        });
    }
}
