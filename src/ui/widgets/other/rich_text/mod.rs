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
//! - 代码复制：悬停显示 copy 图标按钮，点击复制
//!
//! # 与 Label/Typography 的区别
//!
//! Label 和 Typography 面向单样式文本，RichText 面向混合样式段落，
//! 适合渲染 Markdown 风格的聊天消息、文档等富文本内容。

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::geometry::spatial::PhysicalUnit;
use crate::draw::{Color, Radius};
use crate::ui::component::clipboard;
use crate::ui::component::paint_context::PaintContext;
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
// 集中保存富文本布局字形、行与代码复制区域类型。
mod layout_types;
// 向富文本组件根暴露内部布局类型。
pub(crate) use layout_types::*;
// 集中保存估算字符宽度、行刷新与完整逻辑源拼接。
mod layout_metrics;
mod parse;
mod rich_text_interaction;
// 将 shaping cluster 到富文本 advance 的映射隔离为小型内部模块。
mod shaped_advance;
// 将 UAX #14 富文本验收矩阵放入独立测试模块，保持生产文件规模受控。
#[cfg(test)]
mod line_break_tests;

pub use self::parse::{layout_rich_text_segments, parse_rich_text};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RichTextPointerAction {
    Link(usize),
    CopyCode(usize),
}

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

        /// 显式文字颜色；未调用 `color()` 时绘制使用主题正文色
        pub default_color: Color,
        use_theme_color: bool,

        /// 布局行缓存
        layout_lines: RefCell<Vec<LayoutLine>>,

        /// 布局总高度缓存
        layout_height: Cell<f32>,

        /// 内容最大行宽（用于 measure 返回合理宽度）
        content_width: Cell<f32>,

        /// 上次布局使用的宽度（用于 measure 复用估计值）
        last_layout_width: Cell<f32>,

        /// 上次真实布局解析出的默认色（主题切换时使缓存失效）
        last_layout_color: Cell<Option<Color>>,

        /// 是否需要重新布局
        layout_dirty: Cell<bool>,

        // ── 选择状态 ──
        selection: Cell<Option<(usize, usize)>>,
        sel_anchor: Cell<usize>,
        sel_dragging: Cell<bool>,

        // ── 链接交互 ──
        hovered_link: Cell<Option<usize>>,
        pressed_action: Option<RichTextPointerAction>,
        focused: bool,
        focused_link: usize,
        pending_submit: RefCell<Option<String>>,
        on_link: Option<Rc<dyn Fn(&str)>>,

        // ── 代码块复制 ──
        code_regions: RefCell<Vec<CodeCopyRegion>>,
        hovered_code: Cell<Option<usize>>,
        last_frame: Cell<Option<Rect>>,
        /// 待复制的代码内容（外部主循环拉取）
        pub pending_copy: Arc<Mutex<Option<String>>>,
    }

    @new -> Self {
        Self {
            segments: Vec::new(),
            default_font_size: 14.0,
            default_font_size_unit: None,
            default_color: Color::from_rgb(200, 200, 200),
            use_theme_color: true,
            layout_lines: RefCell::new(Vec::new()),
            layout_height: Cell::new(0.0),
            content_width: Cell::new(0.0),
            last_layout_width: Cell::new(0.0),
            last_layout_color: Cell::new(None),
            layout_dirty: Cell::new(true),
            selection: Cell::new(None),
            sel_anchor: Cell::new(0),
            sel_dragging: Cell::new(false),
            hovered_link: Cell::new(None),
            pressed_action: None,
            focused: false,
            focused_link: 0,
            pending_submit: RefCell::new(None),
            on_link: None,
            code_regions: RefCell::new(Vec::new()),
            hovered_code: Cell::new(None),
            last_frame: Cell::new(None),
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

        // 宽度约束变化会改变折行与固有高度，必须参与测量缓存失效。
        let width_changed = (self.last_layout_width.get() - est_width).abs() > 0.5;
        if self.layout_dirty.get() || self.layout_height.get() <= 0.0 || width_changed {
            let dpi = 96.0;
            let fs = self.resolved_font_size_px(dpi);
            let (_, total_h, max_w) = layout_rich_text(
                &self.segments, est_width, fs, self.default_color,
            );
            self.layout_height.set(total_h);
            self.content_width.set(max_w);
            // 旧行坐标不再对应当前约束，等待绘制阶段用真实字体重新建立。
            self.layout_lines.borrow_mut().clear();
            // 旧代码复制区域同样不能继续参与新宽度下的命中。
            self.code_regions.borrow_mut().clear();
            // 记录本轮估算宽度，避免同一布局收敛周期重复测量。
            self.last_layout_width.set(est_width);
            // 清除真实布局颜色键，强制下一次绘制刷新真实字体几何。
            self.last_layout_color.set(None);
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
                if let Some(action) = self.pointer_action_at(*pos) {
                    if let RichTextPointerAction::Link(segment_idx) = action {
                        self.focused = true;
                        self.focused_link = self.link_ordinal(segment_idx).unwrap_or(0);
                    }
                    self.pressed_action = Some(action);
                    self.selection.set(None);
                    self.sel_dragging.set(false);
                    return EventResult::Handled;
                }

                // 文字选择
                if !self.local_frame().contains(*pos) {
                    return EventResult::NotHandled;
                }
                let lines = self.layout_lines.borrow();
                let char_idx = self.char_at_pos(*pos, &lines);
                self.selection.set(None);
                self.sel_anchor.set(char_idx);
                self.sel_dragging.set(true);
                EventResult::Handled
            }

            SystemEvent::PointerMove { pos, .. } => {
                let lines = self.layout_lines.borrow();
                let old_code = self.hovered_code.get();
                let action = self.pointer_action_at(*pos);
                let new_code = match action {
                    Some(RichTextPointerAction::CopyCode(segment_idx)) => Some(segment_idx),
                    _ => None,
                };
                self.hovered_code.set(new_code);
                let old_link = self.hovered_link.get();
                let new_link = match action {
                    Some(RichTextPointerAction::Link(segment_idx)) => Some(segment_idx),
                    _ => None,
                };
                self.hovered_link.set(new_link);
                let hover_changed = new_code != old_code || new_link != old_link;

                // 选择拖拽
                if !self.sel_dragging.get() {
                    return if hover_changed {
                        EventResult::Handled
                    } else {
                        EventResult::NotHandled
                    };
                }
                let char_idx = self.char_at_pos(*pos, &lines);
                let anchor = self.sel_anchor.get();
                self.set_selection_range(anchor, char_idx);
                EventResult::Handled
            }

            SystemEvent::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(pressed) = self.pressed_action.take() {
                    let released = self.pointer_action_at(*pos);
                    if released == Some(pressed) {
                        self.commit_pointer_action(pressed);
                    }
                    return EventResult::Handled;
                }
                if !self.sel_dragging.get() {
                    return EventResult::NotHandled;
                }
                self.sel_dragging.set(false);
                if let Some((s, e)) = self.selection.get() {
                    if s == e { self.selection.set(None); }
                }
                EventResult::Handled
            }

            SystemEvent::PointerLeave => {
                let changed = self.hovered_code.get().is_some()
                    || self.hovered_link.get().is_some()
                    || self.pressed_action.take().is_some();
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
                self.pressed_action = None;
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
        self.pending_submit.borrow_mut().take().map(|url| {
            // E-07：on_link 便捷回调与既有 SemanticKind::Submit 语义事件共存。
            if let Some(callback) = &self.on_link {
                callback(&url);
            }
            SemanticEvent::submit(id, url)
        })
    }

    flex_grow => (&self) -> f32 { 1.0 }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let frame = Self::normalized_frame(frame);
        self.last_frame.set(Some(frame));
        self.code_regions.borrow_mut().clear();
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let max_w = frame.w;
        let resolved_default_color = if self.use_theme_color {
            ctx.tokens().color_text()
        } else {
            self.default_color
        };

        // 布局缓存：仅在内容或宽度变化时重新布局，否则复用上次结果
        let need_relayout = self.layout_dirty.get()
            || (self.last_layout_width.get() - max_w).abs() > 0.5
            || self.last_layout_color.get() != Some(resolved_default_color);

        let (layout_lines, _total_h, _max_line_w) = if need_relayout {
            let font = *ctx.font();
            let fs = self.resolved_font_size_px(ctx.dpi());
            let (lines, h, w) = layout_rich_text_real(
                &self.segments, max_w, fs, resolved_default_color,
                ctx.font_service(), &font,
            );
            // 缓存相对坐标（y 从 0 开始），渲染时再加 frame.y
            self.layout_lines.replace(lines.clone());
            self.layout_height.set(h);
            self.content_width.set(w);
            self.last_layout_width.set(max_w);
            self.last_layout_color.set(Some(resolved_default_color));
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
        ctx.push_clip(frame);

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
                let pressed_link = self.pressed_action
                    == Some(RichTextPointerAction::Link(glyph.segment_idx));
                let active_link = self.hovered_link.get() == Some(glyph.segment_idx)
                    || focused_link
                    || pressed_link;
                if active_link {
                    ctx.fill_rect(
                        Rect::new(gx, gy, glyph.width, glyph.font_size),
                        ctx.tokens()
                            .color_primary()
                            .with_alpha(if pressed_link { 48 } else { 24 }),
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
                    && (self.hovered_link.get() == Some(segment_idx)
                        || focused_link
                        || self.pressed_action == Some(RichTextPointerAction::Link(segment_idx)))
                {
                    ctx.tokens().color_primary()
                } else {
                    first.color
                };
                // 通过统一 run 绘制入口应用粗体与斜体，保持 DisplayList 可回放。
                draw_rich_text_run(ctx, &content, Point::new(gx, gy), color, fs, self.segments.get(segment_idx));
                start = end;
            }
        }

        // 每个代码段只登记一个复制按钮，折行时锚定最后一个可见字形。
        for (segment_idx, segment) in self.segments.iter().enumerate() {
            let RichTextSegment::Code { .. } = segment else { continue; };
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
                let btn_width = 24.0_f32.min(frame.w);
                let btn = Rect::new(
                    (gx - 4.0).min(frame.x + frame.w - btn_width).max(frame.x),
                    gy,
                    btn_width,
                    16.0,
                );
                if let Some(visible_btn) = btn.intersect(&frame) {
                    let hovered = self.hovered_code.get() == Some(segment_idx);
                    let pressed = self.pressed_action
                        == Some(RichTextPointerAction::CopyCode(segment_idx));
                    if hovered || pressed {
                        ctx.fill_rect(
                            visible_btn,
                            if pressed {
                                Color::from_rgb(35, 35, 40)
                            } else {
                                Color::from_rgb(55, 55, 62)
                            },
                            Some(Radius::uniform(3.0)),
                        );
                        crate::ui::widgets::icon::Icon::paint_in_frame(
                            ctx,
                            "copy",
                            visible_btn,
                            Color::from_rgb(200, 200, 200),
                            10.0_f32.min(visible_btn.h * 0.65),
                        );
                    }
                    code_regions.push(CodeCopyRegion {
                        rect: Rect::new(
                            visible_btn.x - frame.x,
                            visible_btn.y - frame.y,
                            visible_btn.w,
                            visible_btn.h,
                        ),
                        segment_idx,
                    });
                }
            }
        }
        ctx.pop_clip();
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
        self.use_theme_color = false;
        self.layout_dirty.set(true);
        self
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let segments_changed = self.segments != next.segments;
        let layout_config_changed = segments_changed
            || self.default_font_size != next.default_font_size
            || self.default_font_size_unit != next.default_font_size_unit
            || self.default_color != next.default_color
            || self.use_theme_color != next.use_theme_color;

        self.segments = next.segments;
        self.default_font_size = next.default_font_size;
        self.default_font_size_unit = next.default_font_size_unit;
        self.default_color = next.default_color;
        self.use_theme_color = next.use_theme_color;
        if next.on_link.is_some() {
            self.on_link = next.on_link;
        }

        if layout_config_changed {
            self.layout_lines.borrow_mut().clear();
            self.layout_height.set(0.0);
            self.content_width.set(0.0);
            self.last_layout_width.set(0.0);
            self.last_layout_color.set(None);
            self.code_regions.borrow_mut().clear();
            self.hovered_code.set(None);
            self.layout_dirty.set(true);
        }
        if segments_changed {
            self.selection.set(None);
            self.sel_anchor.set(0);
            self.sel_dragging.set(false);
            self.hovered_link.set(None);
            self.pressed_action = None;
            self.pending_submit.borrow_mut().take();
            if let Ok(mut pending_copy) = self.pending_copy.lock() {
                pending_copy.take();
            }
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

    /// 注册链接激活回调（E-07）：链接经键盘 Enter/Space 或指针点击激活时
    /// 调用，导航策略由应用决定（内部路由或系统浏览器）；与
    /// `SemanticKind::Submit` 语义事件共存，不替代既有事件流。
    pub fn on_link<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) + 'static,
    {
        self.on_link = Some(Rc::new(callback));
        self
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

    // 测试目标保留代码复制区域观测入口，供富文本交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn code_copy_rect_for_test(&self, index: usize) -> Option<Rect> {
        self.code_regions
            .borrow()
            .get(index)
            .map(|region| region.rect)
    }

    // 测试目标保留布局行文本观测入口，供富文本换行测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn layout_line_texts_for_test(&self) -> Vec<String> {
        self.layout_lines
            .borrow()
            .iter()
            .map(|line| line.glyphs.iter().map(|glyph| glyph.ch).collect())
            .collect()
    }

    // 测试目标保留链接命中点观测入口，供富文本链接测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
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
}

// ════════════════════════════════════════════════════════════════════════════
// 公共渲染辅助函数（供非 RichText widget 直接渲染富文本内容使用）
// ════════════════════════════════════════════════════════════════════════════

/// 计算富文本布局并返回尺寸信息
///
/// 可用于非 RichText widget 中直接测量富文本尺寸。
/// 返回 (总高度, 总字符数, 最大行宽)。
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use crate::ui::component::traits::EventHandler;
    use crate::ui::event::SystemEvent;
    use crate::ui::{KeyMod, MouseButton};

    fn dummy_event() -> SystemEvent {
        SystemEvent::PointerUp {
            pos: Point::new(0.0, 0.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }
    }

    #[test]
    fn on_link_callback_fires_when_submit_emitted() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let hook = calls.clone();
        let rich = RichText::new().on_link(move |url| hook.borrow_mut().push(url.to_string()));
        rich.pending_submit
            .replace(Some("https://example.com".to_string()));

        let event = rich.semantic_event(ComponentId::default(), &dummy_event());
        assert!(event.is_some(), "应发出 Submit 语义事件");
        assert_eq!(*calls.borrow(), vec!["https://example.com".to_string()]);
    }

    #[test]
    fn on_link_not_called_without_pending_submit() {
        let calls = Rc::new(RefCell::new(0usize));
        let hook = calls.clone();
        let rich = RichText::new().on_link(move |_| *hook.borrow_mut() += 1);

        let event = rich.semantic_event(ComponentId::default(), &dummy_event());
        assert!(event.is_none());
        assert_eq!(*calls.borrow(), 0, "无待提交链接时不应触发回调");
    }

    #[test]
    fn on_link_and_submit_semantic_event_coexist() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let hook = calls.clone();
        let rich = RichText::new().on_link(move |url| hook.borrow_mut().push(url.to_string()));
        rich.pending_submit
            .replace(Some("https://uix.dev/route".to_string()));

        let Some(event) = rich.semantic_event(ComponentId::default(), &dummy_event()) else {
            panic!("Submit 语义事件保留");
        };
        assert_eq!(
            event.kind,
            crate::ui::SemanticKind::Submit,
            "与 SemanticKind::Submit 共存"
        );
        assert!(
            matches!(&event.payload, crate::ui::SemanticPayload::Text(url) if url == "https://uix.dev/route")
        );
        assert_eq!(*calls.borrow(), vec!["https://uix.dev/route".to_string()]);
    }
}
