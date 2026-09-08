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

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::geometry::spatial::PhysicalUnit;
use crate::draw::{Color, Radius};
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::clipboard;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{
    EventResult, KeyCode, KeyMod, MouseButton, SemanticEvent, SnapshotFields, SystemEvent,
    UserSelect, WidgetId, WidgetTree,
};
use crate::widget;

// ════════════════════════════════════════════════════════════════════════════
// 数据类型
// ════════════════════════════════════════════════════════════════════════════

/// 富文本段类型
///
/// 一个 RichText 由多个段组成，按顺序排列。
/// 支持普通文本（独立样式控制）、内联代码、链接、主题分隔线与换行。
#[derive(Debug, Clone, PartialEq)]
pub enum RichTextSegment {
    /// 普通文本段（带独立样式）
    Text {
        /// 参与布局、选择与复制的文本内容。
        content: String,
        /// 只作用于该文本段的样式覆写。
        style: RichTextStyle,
    },
    /// 内联代码（等宽字体 + 深色背景 + 圆角）
    Code {
        /// 以代码样式显示并可独立复制的文本内容。
        content: String,
    },
    /// 可点击链接（自动带下划线和交互色）
    Link {
        /// 链接在富文本中显示的文字。
        content: String,
        /// 激活链接时通过语义事件提交给应用的目标地址。
        url: String,
    },
    /// 本地图片原子替换对象；仅在 image-codecs capability 开启时可用
    #[cfg(feature = "image-codecs")]
    Image {
        /// 本地文件路径
        src: String,
        /// 选择复制与无障碍使用的替代文本
        alt: String,
        /// 可选逻辑像素宽度覆盖
        width: Option<f32>,
        /// 可选逻辑像素高度覆盖
        height: Option<f32>,
        /// true 保持比例居中，false 拉伸填满
        fit: bool,
        /// 可选逻辑像素圆角半径
        radius: Option<f32>,
    },
    /// 独立、零字符宽度的 Markdown 主题分隔线
    ThematicBreak,
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
// 集中定义布局消费的主题颜色投影。
mod rich_text_palette;
// 向 RichText 组件根和布局实现暴露私有调色板值。
pub(crate) use rich_text_palette::RichTextPalette;
// 集中定义 UIX 静态视觉与主题解析边界。
mod presentation;
use presentation::*;
// 对无资源状态调用提供稳定占位布局兼容入口。
mod rich_text_layout_entry;
// 继续向现有内部测试和公开解析辅助暴露兼容入口。
pub(crate) use rich_text_layout_entry::*;
// 集中保存富文本布局字形、行与代码复制区域类型。
mod layout_types;
// 向富文本组件根暴露内部布局类型。
pub(crate) use layout_types::*;
// 集中保存估算字符宽度、行刷新与完整逻辑源拼接。
mod layout_metrics;
// 提供不拼接正文的跨段 Unicode 字素簇游标。
mod segmented_text;
// 保存 RichText 的段落级 UAX #9 视觉 run 定位。
mod bidi_layout;
mod parse;
mod rich_text_interaction;
// 把公开构建器、reconcile 与测试观测方法放入独立实现文件。
mod rich_text_methods;
mod typography;
// 集中拥有 RichText 内联图片的资源、几何与绘制生命周期。
mod inline_image;
// 集中拥有主题分隔线的解析、布局与绘制策略。
mod thematic_break;
// 将 shaping cluster 到富文本 advance 的映射隔离为小型内部模块。
mod shaped_advance;

pub use self::parse::{layout_rich_text_segments, parse_rich_text};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RichTextPointerAction {
    Link(usize),
    CopyCode(usize),
}

// ════════════════════════════════════════════════════════════════════════════
// Widget
// ════════════════════════════════════════════════════════════════════════════

widget! {
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

        /// 上次真实布局解析出的完整调色板（主题切换时使缓存失效）
        last_layout_palette: Cell<Option<RichTextPalette>>,

        /// 是否需要重新布局
        layout_dirty: Cell<bool>,

        /// 图片段的异步资源和固有尺寸状态
        image_states: RefCell<inline_image::InlineImageStates>,

        // ── 选择状态 ──
        /// 控制普通文字是否允许建立选区；链接与代码复制不受影响。
        selectable: bool,
        /// 由 WidgetTree 结合祖先声明解析出的最终选择策略。
        user_select_policy: UserSelect,
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

        // 全部实例共享的 UIX 静态视觉表。
        #[snapshot(skip)]
        visual: &'static RichTextVisual,
        // 记录调用方是否显式覆盖 UIX 默认字号。
        #[snapshot(skip)]
        font_size_authored: bool,
        #[snapshot(skip)]
        view_style: Option<crate::ui::theme::style::Style>,
        #[snapshot(skip)]
        view_flex_grow: f32,
        #[snapshot(skip)]
        view_flex_shrink: f32,
        #[snapshot(skip)]
        last_font: Cell<Option<crate::draw::FontHandle>>,
        #[snapshot(skip)]
        last_font_size: Cell<f32>,
        #[snapshot(skip)]
        last_line_height: Cell<f32>,
        // 复用每个绘制 run 的 UTF-8 缓冲，避免逐帧分配 String。
        #[snapshot(skip)]
        run_text_scratch: RefCell<String>,
    }

    @new -> Self {
        Self {
            segments: Vec::new(),
            default_font_size: RICH_TEXT_VISUAL.defaults.font_size,
            default_font_size_unit: None,
            // 默认启用主题正文色，因此字段只保存显式 color() 覆写的占位值。
            default_color: Color::default(),
            use_theme_color: true,
            layout_lines: RefCell::new(Vec::new()),
            layout_height: Cell::new(0.0),
            content_width: Cell::new(0.0),
            last_layout_width: Cell::new(0.0),
            last_layout_palette: Cell::new(None),
            layout_dirty: Cell::new(true),
            // 图片状态初始为空，首次可见绘制才启动后台请求。
            image_states: RefCell::new(inline_image::InlineImageStates::new()),
            // UIX 文档契约要求文字选择默认关闭。
            selectable: false,
            // 未挂载或没有结构声明时保留 RichText 默认不可选语义。
            user_select_policy: UserSelect::Auto,
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
            visual: RICH_TEXT_VISUAL_REF,
            font_size_authored: false,
            view_style: None,
            view_flex_grow: 1.0,
            view_flex_shrink: 1.0,
            last_font: Cell::new(None),
            last_font_size: Cell::new(0.0),
            last_line_height: Cell::new(0.0),
            run_text_scratch: RefCell::new(String::new()),
        }
    }

    measure => (&self, constraints: Constraints) -> Size { self.measure_content(constraints) }

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
                // 未显式开启时把普通文字保持为非交互内容。
                if !self.selection_enabled() { return EventResult::NotHandled; }
                if !self.local_frame().contains(*pos) {
                    return EventResult::NotHandled;
                }
                // all 把当前富文本作为原子整体，不启动可收缩的拖选会话。
                if self.user_select_policy == UserSelect::All {
                    // 计算全部富文本逻辑字符范围。
                    let total = self.cross_text_len();
                    // 保存完整范围并沿用字素簇归一规则。
                    self.set_selection_range(0, total);
                    // all 不保留指针拖选状态。
                    self.sel_dragging.set(false);
                    // 当前文字节点已经消费按下事件。
                    return EventResult::Handled;
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
                    // 全选只属于显式开启的文字选择契约。
                    KeyCode::A if ctrl && self.selection_enabled() => {
                        let total: usize = self.segments.iter()
                            .map(|s| match s {
                                RichTextSegment::Text { content, .. } => content.chars().count(),
                                RichTextSegment::Code { content } => content.chars().count(),
                                RichTextSegment::Link { content, .. } => content.chars().count(),
                                // 图片以 alt 文本参与全选逻辑范围。
                                #[cfg(feature = "image-codecs")]
                                RichTextSegment::Image { alt, .. } => alt.chars().count(),
                                RichTextSegment::ThematicBreak => 0,
                                RichTextSegment::NewLine => 1,
                            }).sum();
                        self.sel_anchor.set(0);
                        self.set_selection_range(0, total);
                        EventResult::Handled
                    }
                    // 复制选区只在选择能力开启时消费快捷键。
                    KeyCode::C if ctrl && self.selection_enabled() => {
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

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_submit.borrow_mut().take().map(|url| {
            // E-07：on_link 便捷回调与既有 SemanticKind::Submit 语义事件共存。
            if let Some(callback) = &self.on_link {
                callback(&url);
            }
            SemanticEvent::submit(id, url)
        })
    }

    flex_grow => (&self) -> f32 { self.view_flex_grow }

    flex_shrink => (&self) -> f32 { self.view_flex_shrink }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        // 图片能力关闭时组件树不参与 RichText 绘制。
        #[cfg(not(feature = "image-codecs"))]
        let _ = tree;
        let frame = Self::normalized_frame(frame);
        self.last_frame.set(Some(frame));
        self.code_regions.borrow_mut().clear();
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let previous_font = *ctx.font();
        let font = crate::ui::text_family::resolve(ctx,
            self.view_style.as_ref().and_then(|style| style.font_family.as_ref()));
        ctx.set_font(font);
        let fs = self.font_size_with_tokens(ctx.dpi(), ctx.tokens());
        let metrics = self.resolved_metrics(fs);
        let padding = self.content_padding();
        let frame = Rect::new(frame.x + padding.left, frame.y + padding.top,
            (frame.w - padding.horizontal()).max(0.0), (frame.h - padding.vertical()).max(0.0));
        let max_w = frame.w;
        // 每帧只解析一次 UIX 声明的主题角色。
        let explicit_color = self.view_style.as_ref().map(|style| style.color.resolve(ctx.tokens()));
        let resolved = self.visual.resolve(explicit_color.unwrap_or(self.default_color),
            explicit_color.is_none() && self.use_theme_color, ctx.tokens());
        let resolved_palette = resolved.layout_palette;

        // 图片能力开启时先非阻塞轮询资源事实，尺寸就绪后使布局缓存失效。
        #[cfg(feature = "image-codecs")]
        if inline_image::prepare(
            // 传递公开段列表。
            &self.segments,
            // 传递组件独占图片状态。
            &mut self.image_states.borrow_mut(),
            // 传递当前绘制和资源上下文。
            ctx,
            // 传递组件树失效队列。
            tree,
            // 传递当前组件绘制范围。
            frame,
        ) {
            // 固有尺寸或失败状态变化要求重建真实布局。
            self.layout_dirty.set(true);
        }

        // 布局缓存：仅在内容或宽度变化时重新布局，否则复用上次结果
        let need_relayout = self.layout_dirty.get()
            || (self.last_layout_width.get() - max_w).abs()
                > self.visual.defaults.relayout_epsilon
            || self.last_layout_palette.get() != Some(resolved_palette)
            || self.last_font.get() != Some(font)
            || self.last_font_size.get() != fs
            || self.last_line_height.get() != metrics.line_height_factor;

        if need_relayout {
            let (lines, h, w) = layout_rich_text_real_with_images_visual(
                &self.segments,
                max_w,
                fs,
                resolved_palette,
                ctx.font_service(),
                &font,
                &self.image_states.borrow(),
                metrics,
            );
            self.last_font.set(Some(font));
            self.last_font_size.set(fs);
            self.last_line_height.set(metrics.line_height_factor);
            // 直接转移相对坐标布局所有权，避免复制全部行与字形。
            self.layout_lines.replace(lines);
            self.layout_height.set(h);
            self.content_width.set(w);
            self.last_layout_width.set(max_w);
            self.last_layout_palette.set(Some(resolved_palette));
            self.layout_dirty.set(false);
        }
        // 绘制全程借用相对坐标缓存，不再为每帧克隆行与字形。
        let layout_lines = self.layout_lines.borrow();

        let mut code_regions = self.code_regions.borrow_mut();
        ctx.push_clip(frame);

        // 先绘制零字形的主题分隔线行，再绘制普通字形内容。
        thematic_break::draw(
            ctx,
            &layout_lines,
            frame,
            self.visual.thematic_break,
            resolved.text_quaternary,
        );

        // 图片能力开启时按共享布局几何绘制全部原子替换对象。
        #[cfg(feature = "image-codecs")]
        inline_image::draw_visual(
            // 传递绘制上下文。
            ctx,
            // 传递公开图片样式与 alt。
            &self.segments,
            // 传递已轮询资源状态。
            &self.image_states.borrow(),
            // 传递真实布局行。
            &layout_lines,
            // 传递组件 frame。
            frame,
            // 传递 UIX 图片占位视觉。
            self.visual.image,
            // 传递本帧一次解析完成的颜色。
            resolved,
        );

        // 同帧交互状态只读取一次，焦点序号也只映射一次段索引。
        let selection = self.selection.get();
        let hovered_link = self.hovered_link.get();
        let pressed_action = self.pressed_action;
        let focused_link_segment = self
            .focused
            .then(|| self.link_segment_at_ordinal(self.focused_link))
            .flatten();

        // ── 逐行绘制 ──
        for line in layout_lines.iter() {
            let line_y = frame.y + line.y;
            for glyph in &line.glyphs {
                let gx = frame.x + glyph.x;
                let gy = line_y;

                // 背景色（代码段背景）
                if let Some(bg) = glyph.bg_color {
                    let pad = self.visual.decoration.background_padding;
                    ctx.fill_rect(
                        Rect::new(gx - pad, gy - pad, glyph.width + pad * 2.0, line.height + pad * 2.0),
                        bg,
                        Some(Radius::uniform(self.visual.decoration.background_radius)),
                    );
                }

                // 选中背景
                if let Some((sel_s, sel_e)) = selection {
                    if glyph.global_char_idx < sel_e
                        && glyph.global_char_idx + glyph.source_char_len > sel_s
                    {
                        ctx.fill_rect(
                            Rect::new(gx, gy, glyph.width, line.height),
                            resolved
                                .primary
                                .with_alpha(self.visual.decoration.selection_alpha),
                            None,
                        );
                    }
                }

                // 图片已经由原子绘制路径处理，不进入文本样式与装饰分支。
                if glyph.kind == LayoutGlyphKind::InlineImage {
                    // 选择高亮已在上方覆盖，继续下一个原子。
                    continue;
                }

                let style = match self.segments.get(glyph.segment_idx) {
                    Some(RichTextSegment::Text { style, .. }) => Some(style),
                    _ => None,
                };
                let focused_link = focused_link_segment == Some(glyph.segment_idx);
                let pressed_link =
                    pressed_action == Some(RichTextPointerAction::Link(glyph.segment_idx));
                let active_link = hovered_link == Some(glyph.segment_idx)
                    || focused_link
                    || pressed_link;
                if active_link {
                    ctx.fill_rect(
                        Rect::new(gx, gy, glyph.width, line.height),
                        resolved.primary.with_alpha(if pressed_link {
                            self.visual.decoration.link_pressed_alpha
                        } else {
                            self.visual.decoration.link_active_alpha
                        }),
                        None,
                    );
                }

                // 链接或显式下划线
                if glyph.is_link || style.is_some_and(|style| style.underline) {
                    ctx.fill_rect(
                        Rect::new(
                            gx,
                            gy + glyph.font_size * self.visual.decoration.underline_ratio,
                            glyph.width,
                            self.visual.decoration.stroke,
                        ),
                        if active_link {
                            resolved.primary
                        } else {
                            glyph
                                .color
                                .with_alpha(self.visual.decoration.inactive_decoration_alpha)
                        },
                        None,
                    );
                }
                if style.is_some_and(|style| style.strikethrough) {
                    ctx.fill_rect(
                        Rect::new(
                            gx,
                            gy + glyph.font_size * self.visual.decoration.strikethrough_ratio,
                            glyph.width,
                            self.visual.decoration.stroke,
                        ),
                        glyph
                            .color
                            .with_alpha(self.visual.decoration.inactive_decoration_alpha),
                        None,
                    );
                }
            }
        }

        // 按实际折行结果绘制连续 run，保证命中、选择与像素使用同一布局。
        let mut run_text_scratch = self.run_text_scratch.borrow_mut();
        for line in layout_lines.iter() {
            let line_y = frame.y + line.y;
            let mut start = 0;
            while start < line.glyphs.len() {
                // 图片原子不进入 draw_text 字符 run。
                if line.glyphs[start].kind == LayoutGlyphKind::InlineImage {
                    // 跳过已经由图片绘制路径处理的单一原子。
                    start += 1;
                    // 继续寻找下一个文本 run。
                    continue;
                }
                let segment_idx = line.glyphs[start].segment_idx;
                // 同一绘制 run 还必须共享 UAX #9 行级方向。
                let bidi_level = line.glyphs[start].bidi_level;
                let mut end = start + 1;
                while end < line.glyphs.len()
                    && line.glyphs[end].segment_idx == segment_idx
                    && line.glyphs[end].bidi_level == bidi_level
                {
                    end += 1;
                }
                let run = &line.glyphs[start..end];
                // 复用组件私有 UTF-8 缓冲，避免每个 run、每帧创建 String。
                run_text_scratch.clear();
                run_text_scratch.extend(run.iter().map(|glyph| glyph.ch));
                let first = &run[0];
                let fs = first.font_size;
                // RTL run 内保留逻辑文本顺序，因此绘制原点取全部字符视觉左缘。
                let gx = frame.x
                    + run
                        // 遍历当前单向 run 字符。
                        .iter()
                        // 提取字符视觉左缘。
                        .map(|glyph| glyph.x)
                        // 聚合 run 左缘。
                        .fold(f32::INFINITY, f32::min);
                let gy = self.run_draw_y(ctx, line, line_y, fs);
                let focused_link = focused_link_segment == Some(segment_idx);
                let color = if first.is_link
                    && (hovered_link == Some(segment_idx)
                        || focused_link
                        || pressed_action == Some(RichTextPointerAction::Link(segment_idx)))
                {
                    resolved.primary
                } else {
                    first.color
                };
                // 通过统一 run 绘制入口应用粗体与斜体，保持 DisplayList 可回放。
                draw_rich_text_run(
                    ctx,
                    run_text_scratch.as_str(),
                    Point::new(gx, gy),
                    color,
                    fs,
                    self.segments.get(segment_idx),
                    self.visual.metrics,
                );
                start = end;
            }
        }
        drop(run_text_scratch);

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
                let gy = frame.y
                    + line.y
                    + (line.height - glyph.font_size) * 0.5
                    + self.visual.metrics.code_baseline_offset;
                let btn_width = self.visual.copy.width.min(frame.w);
                let btn = Rect::new(
                    (gx - self.visual.copy.trailing_gap)
                        .min(frame.x + frame.w - btn_width)
                        .max(frame.x),
                    gy,
                    btn_width,
                    self.visual.copy.height,
                );
                if let Some(visible_btn) = btn.intersect(&frame) {
                    let hovered = self.hovered_code.get() == Some(segment_idx);
                    let pressed =
                        pressed_action == Some(RichTextPointerAction::CopyCode(segment_idx));
                    if hovered || pressed {
                        // 复制按钮底从当前主题交互填充 token 解析。
                        ctx.fill_rect(
                            visible_btn,
                            if pressed {
                                // 按压态使用更强的次级填充。
                                resolved.fill_secondary
                            } else {
                                // 悬停态使用较弱的三级填充。
                                resolved.fill_tertiary
                            },
                            Some(Radius::uniform(self.visual.copy.radius)),
                        );
                        crate::ui::widgets::icon::Icon::paint_in_frame(
                            ctx,
                            self.visual.copy.icon,
                            visible_btn,
                            // 复制图标使用当前主题次级文字色。
                            resolved.text_secondary,
                            self.visual
                                .copy
                                .icon_size
                                .min(visible_btn.h * self.visual.copy.icon_height_ratio),
                        );
                    }
                    code_regions.push(CodeCopyRegion {
                        rect: Rect::new(
                            visible_btn.x - frame.x + padding.left,
                            visible_btn.y - frame.y + padding.top,
                            visible_btn.w,
                            visible_btn.h,
                        ),
                        segment_idx,
                    });
                }
            }
        }
        ctx.pop_clip();
        ctx.set_font(previous_font);
    }
}

// 把 Markdown/Unicode/交互内核与 UIX 静态视觉融合为单一 RichText 根节点。
fn build_rich_text_view(mut kernel: RichText, visual: &'static RichTextVisual) -> ViewNode {
    if !kernel.font_size_authored {
        kernel.default_font_size = visual.defaults.font_size;
    }
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

// 让声明式 View 构建统一进入同目录 UIX 根。
fn build_rich_text_uix_root(kernel: RichText) -> ViewNode {
    crate::uix!("src/ui/widgets/display/rich_text/rich_text.uix")
}

impl View for RichText {
    fn build(self) -> ViewNode {
        build_rich_text_uix_root(self)
    }
}
