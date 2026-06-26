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
//! - 链接点击：按下时复制 URL 到剪贴板
//! - 代码复制：悬停显示 📋 按钮，点击复制
//!
//! # 与 Label/Typography 的区别
//!
//! Label 和 Typography 面向单样式文本，RichText 面向混合样式段落，
//! 适合渲染 Markdown 风格的聊天消息、文档等富文本内容。

use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex};

use uix_core::{Point, Rect, Size};
use crate::clipboard;
use crate::define_widget;
use uix_graphics::font_service::FontService;
use uix_graphics::{Color, GraphicsEngine, Radius, TextLayoutOptions};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, KeyCode, KeyMod, WidgetEvent, WidgetTree};

// ════════════════════════════════════════════════════════════════════════════
// 数据类型
// ════════════════════════════════════════════════════════════════════════════

/// 富文本段类型
///
/// 一个 RichText 由多个段组成，按顺序排列。
/// 支持普通文本（独立样式控制）、内联代码、链接、换行。
#[derive(Debug, Clone)]
pub enum RichTextSegment {
    /// 普通文本段（带独立样式）
    Text {
        content: String,
        style: RichTextStyle,
    },
    /// 内联代码（等宽字体 + 深色背景 + 圆角）
    Code {
        content: String,
    },
    /// 可点击链接（自动带下划线和交互色）
    Link {
        content: String,
        url: String,
    },
    /// 强制换行
    NewLine,
}

/// 文本样式
///
/// 所有字段可选，未设置的字段会继承默认样式。
#[derive(Debug, Clone)]
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

impl Default for RichTextStyle {
    fn default() -> Self {
        Self {
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            font_size: None,
            color: None,
            bg_color: None,
        }
    }
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
// 布局引擎
// ════════════════════════════════════════════════════════════════════════════

/// 布局后的字形（带样式信息）
#[derive(Debug, Clone)]
struct LayoutGlyph {
    segment_idx: usize,
    global_char_idx: usize,
    x: f32,
    width: f32,
    font_size: f32,
    color: Color,
    bg_color: Option<Color>,
    is_link: bool,
    /// 链接 URL（使用 Arc 共享，避免每个字形都分配新 String）
    link_url: Option<std::sync::Arc<str>>,
}

/// 布局后的行
#[derive(Debug, Clone)]
struct LayoutLine {
    y: f32,
    height: f32,
    glyphs: Vec<LayoutGlyph>,
}

/// 估算字符宽度（px），基于字体大小的比例
fn char_width(fs: f32, ch: char) -> f32 {
    match ch {
        ' ' => fs * 0.35,
        '\t' => fs * 2.0,
        'm' | 'M' | 'W' | 'w' => fs * 0.7,
        'i' | 'I' | 'l' | '1' | '.' | ',' | ':' | ';' | '\'' => fs * 0.3,
        // CJK 汉字、假名、韩文
        c if is_cjk(c) => fs * 1.0,
        // CJK 标点符号
        c if c >= '\u{3000}' && c <= '\u{303f}' => fs * 0.9,
        _ => fs * 0.55,
    }
}

/// 判断字符是否属于 CJK（可用于断行）
fn is_cjk(ch: char) -> bool {
    matches!(ch,
        '\u{4E00}'..='\u{9FFF}'   // CJK 统一表意文字
        | '\u{3400}'..='\u{4DBF}'  // CJK 扩展 A
        | '\u{20000}'..='\u{2A6DF}'// CJK 扩展 B
        | '\u{3040}'..='\u{309F}'  // 平假名
        | '\u{30A0}'..='\u{30FF}'  // 片假名
        | '\u{FF66}'..='\u{FF9F}'  // 半角片假名
        | '\u{AC00}'..='\u{D7AF}'  // 韩文音节
        | '\u{1100}'..='\u{11FF}'  // 韩文辅音
        | '\u{1200}'..='\u{137F}'  // 韩文元音
    )
}

/// 估算文本宽度
fn text_width(text: &str, fs: f32) -> f32 {
    text.chars().map(|c| char_width(fs, c)).sum()
}

/// 刷新当前行到行列表
fn flush_line(
    lines: &mut Vec<LayoutLine>,
    glyphs: &mut Vec<LayoutGlyph>,
    line_height: f32,
) {
    let y = lines.last().map(|l| l.y + l.height).unwrap_or(0.0);
    let g = std::mem::take(glyphs);
    lines.push(LayoutLine { y, height: line_height.max(1.0), glyphs: g });
}

/// 执行富文本布局
///
/// 对每段文本，逐个分隔处理：
/// 1. 空格分词（拉丁文本）
/// 2. 若"词"超过 max_width，则按字符级断行（处理 CJK 和超长英文单词）
/// 3. CJK 字符之间可任意断行
///
/// 返回 (行列表, 总高度, 最大行宽)。
fn layout_rich_text(
    segments: &[RichTextSegment],
    max_width: f32,
    default_font_size: f32,
    default_color: Color,
) -> (Vec<LayoutLine>, f32, f32) {
    let mut lines: Vec<LayoutLine> = Vec::new();
    let mut current_line_glyphs: Vec<LayoutGlyph> = Vec::new();
    let mut current_x: f32 = 0.0;
    let line_height_factor: f32 = 1.5;
    let default_line_h = default_font_size * line_height_factor;
    let mut current_line_h: f32 = default_line_h;
    let mut max_line_w: f32 = 0.0;

    for (seg_idx, segment) in segments.iter().enumerate() {
        match segment {
            RichTextSegment::NewLine => {
                max_line_w = max_line_w.max(current_x);
                flush_line(&mut lines, &mut current_line_glyphs, current_line_h);
                current_x = 0.0;
                current_line_h = default_line_h;
            }
            RichTextSegment::Text { content, style } => {
                let fs = style.resolved_font_size(default_font_size);
                let color = style.resolved_color(default_color);
                let bg = style.bg_color;
                let seg_line_h = fs * line_height_factor;
                current_line_h = current_line_h.max(seg_line_h);

                layout_text_content(
                    content, fs, color, bg, false, None,
                    seg_idx, max_width, seg_line_h,
                    &mut lines, &mut current_line_glyphs,
                    &mut current_x, &mut max_line_w,
                );
            }
            RichTextSegment::Code { content } => {
                let fs = default_font_size * 0.9;
                let color = Color::from_rgb(230, 180, 100);
                let bg = Color::from_rgb(40, 40, 45);
                let seg_line_h = fs * line_height_factor;
                current_line_h = current_line_h.max(seg_line_h);

                layout_text_content(
                    content, fs, color, Some(bg), false, None,
                    seg_idx, max_width, seg_line_h,
                    &mut lines, &mut current_line_glyphs,
                    &mut current_x, &mut max_line_w,
                );
            }
            RichTextSegment::Link { content, url } => {
                let fs = default_font_size;
                let color = Color::from_rgb(55, 110, 255);
                let seg_line_h = fs * line_height_factor;
                current_line_h = current_line_h.max(seg_line_h);

                layout_text_content(
                    content, fs, color, None, true, Some(url.as_str()),
                    seg_idx, max_width, seg_line_h,
                    &mut lines, &mut current_line_glyphs,
                    &mut current_x, &mut max_line_w,
                );
            }
        }
    }

    max_line_w = max_line_w.max(current_x);
    if !current_line_glyphs.is_empty() || lines.is_empty() {
        flush_line(&mut lines, &mut current_line_glyphs, current_line_h);
    }

    let total_height = lines.last().map(|l| l.y + l.height).unwrap_or(default_line_h);
    (lines, total_height, max_line_w)
}

/// 布局一段文本内容（处理分词、断行和字形生成）
///
/// 策略：
/// 1. 按空格分词（保留英文自然断词）
/// 2. 若单个词超过 max_width，则按字符级断行（处理 CJK 和超长单词）
/// 3. CJK 字符之间任意位置可断
#[allow(clippy::too_many_arguments)]
fn layout_text_content(
    content: &str,
    fs: f32,
    color: Color,
    bg_color: Option<Color>,
    is_link: bool,
    link_url: Option<&str>,
    seg_idx: usize,
    max_width: f32,
    seg_line_h: f32,
    lines: &mut Vec<LayoutLine>,
    glyphs: &mut Vec<LayoutGlyph>,
    current_x: &mut f32,
    max_line_w: &mut f32,
) {
    // 预计算共享的链接 URL（Arc<str>），避免每个字形都分配新 String
    let shared_url: Option<std::sync::Arc<str>> = link_url.map(|u| std::sync::Arc::from(u));

    // 步骤 1：按空格分词
    let tokens: Vec<&str> = content.split_inclusive(' ').collect();

    for token in &tokens {
        let token_w = text_width(token, fs);

        // 若当前行放不下这个词，先换行
        if *current_x + token_w > max_width && !glyphs.is_empty() {
            *max_line_w = (*max_line_w).max(*current_x);
            flush_line(lines, glyphs, seg_line_h);
            *current_x = 0.0;
        }

        // 步骤 2：若单个词仍超过行宽，按字符级断行
        if *current_x + token_w > max_width && glyphs.is_empty() {
            // 当前行是空的但这个词仍放不下 → 强制字符级断行
            let mut word_chars_x = *current_x;
            for ch in token.chars() {
                let cw = char_width(fs, ch);
                if word_chars_x + cw > max_width && word_chars_x > 0.0 && !glyphs.is_empty() {
                    *max_line_w = (*max_line_w).max(word_chars_x);
                    flush_line(lines, glyphs, seg_line_h);
                    word_chars_x = 0.0;
                }
                glyphs.push(LayoutGlyph {
                    segment_idx: seg_idx,
                    global_char_idx: 0,
                    x: word_chars_x,
                    width: cw,
                    font_size: fs,
                    color,
                    bg_color,
                    is_link,
                    link_url: shared_url.clone(),
                });
                word_chars_x += cw;
            }
            *current_x = word_chars_x;
        } else {
            // 正常追加（词不超宽或行已空但超宽已在上面处理）
            for ch in token.chars() {
                let cw = char_width(fs, ch);
                glyphs.push(LayoutGlyph {
                    segment_idx: seg_idx,
                    global_char_idx: 0,
                    x: *current_x,
                    width: cw,
                    font_size: fs,
                    color,
                    bg_color,
                    is_link,
                    link_url: shared_url.clone(),
                });
                *current_x += cw;
            }
        }
    }
}

/// 为布局中的每个字形分配全局字符索引
fn assign_global_indices(lines: &mut [LayoutLine]) {
    let mut idx: usize = 0;
    for line in lines.iter_mut() {
        for glyph in line.glyphs.iter_mut() {
            glyph.global_char_idx = idx;
            idx += 1;
        }
    }
}

/// 在布局行中查找某个段的第一个字形位置
fn find_first_glyph(lines: &[LayoutLine], segment_idx: usize) -> Option<(usize, usize)> {
    for (li, line) in lines.iter().enumerate() {
        for (gi, glyph) in line.glyphs.iter().enumerate() {
            if glyph.segment_idx == segment_idx {
                return Some((li, gi));
            }
        }
    }
    None
}

// ════════════════════════════════════════════════════════════════════════════
// 基于真实字体度量的布局（用于 render 阶段）
// ════════════════════════════════════════════════════════════════════════════

/// 获取文本中每个字符的真实 advance 宽度（使用 FontService 字体度量）
///
/// 调用 `font_service.layout_text` 以获取每个字符的真实宽度，
/// 替换 `char_width` 的虚构系数。对于复杂文本（连字/emoji），
/// 如果 glyph 数量不足则回退到估算值。
fn real_char_advances(
    font_service: &FontService,
    font: &uix_graphics::FontHandle,
    text: &str,
    fs: f32,
) -> Vec<f32> {
    if text.is_empty() {
        return Vec::new();
    }
    let opts = TextLayoutOptions {
        font_size: fs,
        max_width: f32::MAX,
        max_height: 0.0,
        line_height: 0.0,
        word_wrap: false,
        h_align: uix_graphics::HAlign::Left,
        v_align: uix_graphics::VAlign::Top,
    };
    let layout = font_service.layout_text(font, text, &uix_graphics::text_backend::TextLayoutOptions::from(opts));
    let chars: Vec<char> = text.chars().collect();
    let mut advances = Vec::with_capacity(chars.len());
    for (i, _) in chars.iter().enumerate() {
        if i < layout.glyphs.len() {
            advances.push(layout.glyphs[i].width);
        } else {
            // 字形不足时回退（连字、emoji、字体不支持等）
            advances.push(fs * 0.55);
        }
    }
    advances
}

/// 基于真实字体度量执行富文本布局
///
/// 与 `layout_rich_text` 逻辑相同，但使用 `real_char_advances` 获取真实字符宽度，
/// 替换 `char_width` 的虚构系数。仅在 render 阶段调用（有 FontService 可用）。
fn layout_rich_text_real(
    segments: &[RichTextSegment],
    max_width: f32,
    default_font_size: f32,
    default_color: Color,
    font_service: &FontService,
    font: &uix_graphics::FontHandle,
) -> (Vec<LayoutLine>, f32, f32) {
    let mut lines: Vec<LayoutLine> = Vec::new();
    let mut current_line_glyphs: Vec<LayoutGlyph> = Vec::new();
    let mut current_x: f32 = 0.0;
    let line_height_factor: f32 = 1.5;
    let default_line_h = default_font_size * line_height_factor;
    let mut current_line_h: f32 = default_line_h;
    let mut max_line_w: f32 = 0.0;

    for (seg_idx, segment) in segments.iter().enumerate() {
        match segment {
            RichTextSegment::NewLine => {
                max_line_w = max_line_w.max(current_x);
                flush_line(&mut lines, &mut current_line_glyphs, current_line_h);
                current_x = 0.0;
                current_line_h = default_line_h;
            }
            RichTextSegment::Text { content, style } => {
                let fs = style.resolved_font_size(default_font_size);
                let color = style.resolved_color(default_color);
                let bg = style.bg_color;
                let seg_line_h = fs * line_height_factor;
                current_line_h = current_line_h.max(seg_line_h);

                layout_text_content_real(
                    content, fs, color, bg, false, None,
                    seg_idx, max_width, seg_line_h,
                    &mut lines, &mut current_line_glyphs,
                    &mut current_x, &mut max_line_w,
                    font_service, font,
                );
            }
            RichTextSegment::Code { content } => {
                let fs = default_font_size * 0.9;
                let color = Color::from_rgb(230, 180, 100);
                let bg = Color::from_rgb(40, 40, 45);
                let seg_line_h = fs * line_height_factor;
                current_line_h = current_line_h.max(seg_line_h);

                layout_text_content_real(
                    content, fs, color, Some(bg), false, None,
                    seg_idx, max_width, seg_line_h,
                    &mut lines, &mut current_line_glyphs,
                    &mut current_x, &mut max_line_w,
                    font_service, font,
                );
            }
            RichTextSegment::Link { content, url } => {
                let fs = default_font_size;
                let color = Color::from_rgb(55, 110, 255);
                let seg_line_h = fs * line_height_factor;
                current_line_h = current_line_h.max(seg_line_h);

                layout_text_content_real(
                    content, fs, color, None, true, Some(url.as_str()),
                    seg_idx, max_width, seg_line_h,
                    &mut lines, &mut current_line_glyphs,
                    &mut current_x, &mut max_line_w,
                    font_service, font,
                );
            }
        }
    }

    max_line_w = max_line_w.max(current_x);
    if !current_line_glyphs.is_empty() || lines.is_empty() {
        flush_line(&mut lines, &mut current_line_glyphs, current_line_h);
    }

    let total_height = lines.last().map(|l| l.y + l.height).unwrap_or(default_line_h);
    (lines, total_height, max_line_w)
}

/// 基于真实字体度量布局一段文本
#[allow(clippy::too_many_arguments)]
fn layout_text_content_real(
    content: &str,
    fs: f32,
    color: Color,
    bg_color: Option<Color>,
    is_link: bool,
    link_url: Option<&str>,
    seg_idx: usize,
    max_width: f32,
    seg_line_h: f32,
    lines: &mut Vec<LayoutLine>,
    glyphs: &mut Vec<LayoutGlyph>,
    current_x: &mut f32,
    max_line_w: &mut f32,
    font_service: &FontService,
    font: &uix_graphics::FontHandle,
) {
    // 获取每个字符的真实 advance 宽度
    let advances = real_char_advances(font_service, font, content, fs);
    let chars: Vec<char> = content.chars().collect();
    // 预计算共享的链接 URL（Arc<str>），避免每个字形都分配新 String
    let shared_url: Option<std::sync::Arc<str>> = link_url.map(|u| std::sync::Arc::from(u));

    // 按空格分词
    let mut start = 0usize;
    let total = chars.len();

    while start < total {
        // 找一个词的结束位置（空格或末尾）
        let mut end = start;
        while end < total && chars[end] != ' ' {
            end += 1;
        }
        // 包含结尾空格
        if end < total && chars[end] == ' ' {
            end += 1;
        }
        if end == start {
            start = end;
            continue;
        }

        // 计算这个词的总宽度
        let word_advances = &advances[start..end.min(advances.len())];
        let word_w: f32 = word_advances.iter().sum();

        // 若当前行放不下这个词，先换行
        if *current_x + word_w > max_width && !glyphs.is_empty() {
            *max_line_w = (*max_line_w).max(*current_x);
            flush_line(lines, glyphs, seg_line_h);
            *current_x = 0.0;
        }

        // 若单个词超宽且行是空的，按字符级断行
        if *current_x + word_w > max_width && glyphs.is_empty() {
            let mut word_x = *current_x;
            for i in start..end.min(advances.len()) {
                let cw = advances[i];
                if word_x + cw > max_width && word_x > 0.0 && !glyphs.is_empty() {
                    *max_line_w = (*max_line_w).max(word_x);
                    flush_line(lines, glyphs, seg_line_h);
                    word_x = 0.0;
                }
                let _ch = if i < chars.len() { chars[i] } else { ' ' };
                glyphs.push(LayoutGlyph {
                    segment_idx: seg_idx,
                    global_char_idx: 0,
                    x: word_x,
                    width: cw,
                    font_size: fs,
                    color,
                    bg_color,
                    is_link,
                    link_url: shared_url.clone(),
                });
                word_x += cw;
            }
            *current_x = word_x;
        } else {
            // 正常追加
            for i in start..end.min(advances.len()) {
                let cw = advances[i];
                let _ch = if i < chars.len() { chars[i] } else { ' ' };
                glyphs.push(LayoutGlyph {
                    segment_idx: seg_idx,
                    global_char_idx: 0,
                    x: *current_x,
                    width: cw,
                    font_size: fs,
                    color,
                    bg_color,
                    is_link,
                    link_url: shared_url.clone(),
                });
                *current_x += cw;
            }
        }
        start = end;
    }
}

/// 代码块复制按钮区域
#[derive(Debug, Clone)]
struct CodeCopyRegion {
    rect: Rect,
    content: String,
}

// ════════════════════════════════════════════════════════════════════════════
// Widget
// ════════════════════════════════════════════════════════════════════════════

define_widget! {
    /// 富文本显示组件
    ///
    /// 支持带样式的分段文本、内联代码、可点击链接、自动换行和文字选择。
    pub struct RichText {
        /// 富文本段列表
        pub segments: Vec<RichTextSegment>,

        /// 默认字体大小
        pub default_font_size: f32,

        /// 默认文字颜色
        pub default_color: Color,

        /// 布局行缓存
        layout_lines: RefCell<Vec<LayoutLine>>,

        /// 布局总高度缓存
        layout_height: Cell<f32>,

        /// 内容最大行宽（用于 preferred_size 返回合理宽度）
        content_width: Cell<f32>,

        /// 上次布局使用的宽度（用于 preferred_size 复用估计值）
        last_layout_width: Cell<f32>,

        /// 是否需要重新布局
        layout_dirty: Cell<bool>,

        // ── 选择状态 ──
        selection: Cell<Option<(usize, usize)>>,
        sel_anchor: Cell<usize>,
        sel_dragging: Cell<bool>,

        // ── 链接悬停 ──
        _hovered_link: Cell<Option<usize>>,

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
            default_color: Color::from_rgb(200, 200, 200),
            layout_lines: RefCell::new(Vec::new()),
            layout_height: Cell::new(0.0),
            content_width: Cell::new(0.0),
            last_layout_width: Cell::new(0.0),
            layout_dirty: Cell::new(true),
            selection: Cell::new(None),
            sel_anchor: Cell::new(0),
            sel_dragging: Cell::new(false),
            _hovered_link: Cell::new(None),
            code_regions: RefCell::new(Vec::new()),
            hovered_code: Cell::new(None),
            pending_copy: Arc::new(Mutex::new(None)),
        }
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        // 使用上次渲染的实际宽度作为布局估计宽度，
        // 避免 preferred_size 和 render 使用不同宽度导致高度估计偏差。
        // 首帧无缓存时使用 400.0 作为合理默认值。
        let est_width = if self.last_layout_width.get() > 0.0 {
            self.last_layout_width.get()
        } else {
            400.0_f32
        };

        if self.layout_dirty.get() || self.layout_height.get() <= 0.0 {
            let (_, total_h, max_w) = layout_rich_text(
                &self.segments, est_width, self.default_font_size, self.default_color,
            );
            self.layout_height.set(total_h);
            self.content_width.set(max_w);
            self.layout_dirty.set(false);
        }
        Size::new(self.content_width.get(), self.layout_height.get())
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
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
                let lines = self.layout_lines.borrow();
                for line in lines.iter() {
                    if pos.y < line.y || pos.y >= line.y + line.height { continue; }
                    for glyph in &line.glyphs {
                        if glyph.is_link && pos.x >= glyph.x && pos.x < glyph.x + glyph.width {
                            if let Some(url) = &glyph.link_url {
                                clipboard::copy_to_clipboard(url);
                                return EventResult::Handled;
                            }
                        }
                    }
                }

                // 文字选择
                let char_idx = self.char_at_pos(*pos, &lines);
                self.selection.set(None);
                self.sel_anchor.set(char_idx);
                self.sel_dragging.set(true);
                EventResult::Handled
            }

            WidgetEvent::MouseMove { pos } => {
                let lines = self.layout_lines.borrow();

                // 代码块悬停
                let old_code = self.hovered_code.get();
                let mut new_code: Option<usize> = None;
                for (i, region) in self.code_regions.borrow().iter().enumerate() {
                    if region.rect.contains(*pos) { new_code = Some(i); break; }
                }
                self.hovered_code.set(new_code);
                if new_code != old_code { return EventResult::Handled; }

                // 选择拖拽
                if !self.sel_dragging.get() { return EventResult::NotHandled; }
                let char_idx = self.char_at_pos(*pos, &lines);
                let anchor = self.sel_anchor.get();
                self.set_selection_range(anchor, char_idx);
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
                    _ => EventResult::NotHandled,
                }
            }

            _ => EventResult::NotHandled,
        }
    }

    flex_grow => (&self) -> f32 { 1.0 }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let max_w = frame.w.max(1.0);

        // 布局缓存：仅在内容或宽度变化时重新布局，否则复用上次结果
        let need_relayout = self.layout_dirty.get()
            || (self.last_layout_width.get() - max_w).abs() > 0.5;

        let (layout_lines, total_h, max_line_w) = if need_relayout {
            let font = *ctx.font();
            let (lines, h, w) = layout_rich_text_real(
                &self.segments, max_w, self.default_font_size, self.default_color,
                ctx.font_service(), &font,
            );
            let mut lines = lines;
            assign_global_indices(&mut lines);
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

                // 链接下划线
                if glyph.is_link {
                    ctx.fill_rect(
                        Rect::new(gx, gy + glyph.font_size * 0.95, glyph.width, 1.0),
                        glyph.color.with_alpha(180),
                        None,
                    );
                }
            }
        }

        // ── 按段绘制文字 ──
        for (seg_idx, segment) in self.segments.iter().enumerate() {
            match segment {
                RichTextSegment::NewLine => {}
                RichTextSegment::Text { content, style } => {
                    if content.is_empty() { continue; }
                    let fs = style.resolved_font_size(self.default_font_size);
                    let color = style.resolved_color(self.default_color);
                    // 文本只在首个出现行绘制一次，由 ctx.draw_text 内部处理 \n 换行，
                    // 避免折行后每行都画整段内容导致视觉重复。
                    if let Some((first_li, _gi)) = find_first_glyph(&layout_lines, seg_idx) {
                        let gx = frame.x + layout_lines[first_li].glyphs.iter()
                            .find(|g| g.segment_idx == seg_idx).map(|g| g.x).unwrap_or(0.0);
                        let gy = layout_lines[first_li].y
                            + (layout_lines[first_li].height - fs) * 0.5;
                        ctx.draw_text(content, Point::new(gx, gy), color, fs);
                    }
                }
                RichTextSegment::Code { content } => {
                    if content.is_empty() { continue; }
                    let fs = self.default_font_size * 0.9;
                    let color = Color::from_rgb(230, 180, 100);
                    if let Some((li, gi)) = find_first_glyph(&layout_lines, seg_idx) {
                        let gx = frame.x + layout_lines[li].glyphs[gi].x;
                        let gy = layout_lines[li].y + (layout_lines[li].height - fs) * 0.5 + 2.0;
                        ctx.draw_text(content, Point::new(gx, gy), color, fs);

                        // 记录代码块复制按钮
                        let code_w: f32 = layout_lines[li].glyphs.iter()
                            .filter(|g| g.segment_idx == seg_idx)
                            .map(|g| g.width)
                            .sum();
                        let btn = Rect::new(gx + code_w - 4.0, gy, 24.0, 16.0);
                        let cidx = code_regions.len();
                        if self.hovered_code.get() == Some(cidx) {
                            ctx.fill_rect(btn, Color::from_rgb(55, 55, 62), Some(Radius::uniform(3.0)));
                            ctx.draw_text("📋", Point::new(btn.x + 5.0, btn.y + 1.0),
                                Color::from_rgb(200, 200, 200), 10.0);
                        }
                        code_regions.push(CodeCopyRegion { rect: btn, content: content.clone() });
                    }
                }
                RichTextSegment::Link { content, .. } => {
                    if content.is_empty() { continue; }
                    let fs = self.default_font_size;
                    let color = Color::from_rgb(55, 110, 255);
                    if let Some((li, gi)) = find_first_glyph(&layout_lines, seg_idx) {
                        let gx = frame.x + layout_lines[li].glyphs[gi].x;
                        let gy = layout_lines[li].y + (layout_lines[li].height - fs) * 0.5;
                        ctx.draw_text(content, Point::new(gx, gy), color, fs);
                    }
                }
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 公有方法
// ════════════════════════════════════════════════════════════════════════════

impl RichText {
    /// 设置富文本内容
    pub fn content(mut self, segments: Vec<RichTextSegment>) -> Self {
        self.segments = segments;
        self.layout_dirty.set(true);
        self
    }

    /// 设置默认字体大小
    pub fn font_size(mut self, size: f32) -> Self {
        self.default_font_size = size;
        self.layout_dirty.set(true);
        self
    }

    /// 设置默认文字颜色
    pub fn color(mut self, c: Color) -> Self {
        self.default_color = c;
        self.layout_dirty.set(true);
        self
    }

    /// 获取选中的文本
    pub fn selected_text(&self) -> Option<String> {
        self.selection.get().map(|(s, e)| self.extract_text_range(s, e))
    }

    /// 获取待复制的代码内容（由主循环调用）
    pub fn take_pending_copy(&self) -> Option<String> {
        self.pending_copy.lock().ok().and_then(|mut pc| pc.take())
    }

    // ── 内部 ──

    fn char_at_pos(&self, pos: Point, lines: &[LayoutLine]) -> usize {
        for line in lines {
            if pos.y < line.y || pos.y >= line.y + line.height { continue; }
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
        lines.last()
            .and_then(|l| l.glyphs.last())
            .map(|g| g.global_char_idx + 1)
            .unwrap_or(0)
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
                let local_start = if start > seg_start { start - seg_start } else { 0 };
                let local_end = if end < seg_end { end - seg_start } else { seg_len };
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
    let (_, total_h, max_w) = layout_rich_text(segments, max_width, default_font_size, default_color);
    let char_count: usize = segments.iter().map(|s| match s {
        RichTextSegment::NewLine => 1,
        RichTextSegment::Text { content, .. } => content.chars().count(),
        RichTextSegment::Code { content } => content.chars().count(),
        RichTextSegment::Link { content, .. } => content.chars().count(),
    }).sum();
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
