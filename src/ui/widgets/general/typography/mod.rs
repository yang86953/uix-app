//! Typography 的 Rust 文本选择、排版与复制交互内核。
//!
//! 支持 h1-h5 标题级别、段落文本、disabled/type 等变体。
//! 与 Label 的区别：Typography 提供语义化排版和更多样式选项。

use std::cell::Cell;

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::{Color, Radius, VAlign};
use crate::widget;
// 引入排版快照契约。
use crate::ui::SnapshotFields;
use crate::ui::widget_runtime::clipboard;
use crate::ui::widget_runtime::paint_context::PaintContext;
// 引入共享的单节点文字选区实现。
use crate::ui::text_selection::per_node::PerNodeTextSelection;
// 引入 UI System 拥有的字体族、字重、行高、文本对齐、文本装饰与样式契约。
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{
    ColorValue, FontFamily, FontWeight, LineHeight, PaletteColor, Style, TextAlign, TextDecoration,
};
// 引入主题颜色值与组件运行契约。
use crate::ui::{
    EventResult, KeyCode, KeyMod, MouseButton, SemanticEvent, SystemEvent, ThemeTokens, UserSelect,
    View, ViewNode, WidgetId, WidgetTree,
};

use super::icon::Icon;

#[derive(Debug, Clone, Copy, PartialEq)]
/// 排版组件使用的语义文本层级。
pub enum TypographyType {
    /// 一级标题，使用最大的标题字号与字重。
    Heading1,
    /// 二级标题。
    Heading2,
    /// 三级标题。
    Heading3,
    /// 四级标题。
    Heading4,
    /// 五级标题，使用最小的标题字号。
    Heading5,
    /// 支持折行、行距与首行缩进的段落文本。
    Paragraph,
    /// 不附加段落排版语义的普通文本。
    Text,
}

// 保存单个语义排版层级的字号与默认字重。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TypographyLevelVisual {
    pub(crate) font_size: f32,
    pub(crate) font_weight: FontWeight,
}

// 保存文字行盒、标记背景、复制入口与选择背景的静态几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TypographyGeometryVisual {
    pub(crate) copy_space: f32,
    pub(crate) min_text_width: f32,
    pub(crate) layout_max_height: f32,
    pub(crate) vertical_align: VAlign,
    pub(crate) normal_line_height_factor: f32,
    pub(crate) single_line_height_factor: f32,
    pub(crate) average_advance_factor: f32,
    pub(crate) mark_horizontal_padding: f32,
    pub(crate) mark_vertical_padding: f32,
    pub(crate) code_radius: f32,
    pub(crate) mark_radius: f32,
    pub(crate) selection_alpha: u8,
    pub(crate) copy_width: f32,
    pub(crate) copy_height: f32,
    pub(crate) copy_paint_offset: f32,
    pub(crate) copy_focus_stroke: f32,
    pub(crate) copy_focus_radius: f32,
    pub(crate) copy_icon_size: f32,
    pub(crate) copy_icon: &'static str,
}

// 保存复制入口垂直居中使用的主题字号角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypographyFontRole {
    Body,
}

impl TypographyFontRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Body => tokens.font_size(),
        }
    }
}

// 保存 Typography 使用的全部主题颜色与字号角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TypographyPaletteVisual {
    text: ColorValue,
    disabled_text: ColorValue,
    code_background: ColorValue,
    mark_background: ColorValue,
    primary: ColorValue,
    copy_text: ColorValue,
    copy_center_font: TypographyFontRole,
}

// 全部 Typography 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TypographyVisual {
    pub(crate) levels: [TypographyLevelVisual; 7],
    pub(crate) geometry: TypographyGeometryVisual,
    palette: TypographyPaletteVisual,
}

crate::uix_items!("src/ui/widgets/general/typography/typography.uix");

// 保存每帧一次解析得到的主题视觉值。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedTypographyVisual {
    text: Color,
    disabled_text: Color,
    code_background: Color,
    mark_background: Color,
    primary: Color,
    copy_text: Color,
    copy_center_font_size: f32,
}

impl TypographyVisual {
    fn resolve(self, tokens: &dyn ThemeTokens) -> ResolvedTypographyVisual {
        ResolvedTypographyVisual {
            text: self.palette.text.resolve(tokens),
            disabled_text: self.palette.disabled_text.resolve(tokens),
            code_background: self.palette.code_background.resolve(tokens),
            mark_background: self.palette.mark_background.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            copy_text: self.palette.copy_text.resolve(tokens),
            copy_center_font_size: self.palette.copy_center_font.resolve(tokens),
        }
    }
}

pub(crate) const fn typography_semibold_weight() -> FontWeight {
    FontWeight::SEMIBOLD
}

pub(crate) const fn typography_normal_weight() -> FontWeight {
    FontWeight::NORMAL
}

pub(crate) const fn typography_vertical_align_top() -> VAlign {
    VAlign::Top
}

pub(crate) const fn typography_body_font() -> TypographyFontRole {
    TypographyFontRole::Body
}

pub(crate) const fn typography_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}

pub(crate) const fn typography_disabled_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}

pub(crate) const fn typography_fill_secondary_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillSecondary)
}

pub(crate) const fn typography_warning_background_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::WarningBg)
}

pub(crate) const fn typography_primary_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}

pub(crate) const fn typography_copy_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}

widget! {
    /// 按标题、段落或普通文本语义绘制并支持行内样式的排版组件。
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
        /// 由 WidgetTree 结合祖先声明解析出的最终选择策略。
        user_select_policy: UserSelect,
        // 保存由主题模块拥有的可选语义文字颜色。
        semantic_color: Option<ColorValue>,
        color_override: Option<Color>,
        spacing: f32,
        /// 可选的统一样式字体族列表，None 保留当前系统字体。
        font_family: Option<FontFamily>,
        /// 可选的统一样式字体粗细，显式 normal 也覆盖标题或 strong 默认值。
        font_weight: Option<FontWeight>,
        /// 可选的样式行高，优先于段落 spacing 构建器。
        line_height: Option<LineHeight>,
        /// 可选的统一样式文本水平对齐，显式 left 也覆盖继承值。
        text_align: Option<TextAlign>,
        /// 可选的统一样式文本装饰，显式 none 也覆盖局部构建器。
        text_decoration: Option<TextDecoration>,
        indent: f32,
        ellipsis: bool,
        /// 共享的选区状态：布局缓存、选区、拖选锚点与绘制偏移。
        sel: PerNodeTextSelection,
        copy_rect: Cell<Option<Rect>>,
        focused: bool,
        pending_submit: Cell<bool>,
        /// 同目录 UIX 生成的唯一静态视觉表。
        pub(crate) visual: &'static TypographyVisual,
    }

    measure => (&self, constraints: Constraints) -> Size {
        let mut intrinsic = self.intrinsic_size();
        if matches!(self.type_, TypographyType::Paragraph)
            && constraints.max.w.is_finite()
            && constraints.max.w > 0.0
        {
            let (fs, _) = self.compute_font_style();
            let copy_space = if self.copyable {
                self.visual.geometry.copy_space
            } else {
                0.0
            };
            let text_width = (constraints.max.w - copy_space)
                .max(self.visual.geometry.min_text_width);
            let indent = self.paragraph_indent(fs).min(text_width);
            let wrap_width = (text_width - indent).max(self.visual.geometry.min_text_width);
            let estimated = crate::draw::resources::font::text_backend::estimate_text_metrics(
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
                self.resolved_line_height(fs) * estimated.line_count as f32,
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
                // 点击复制图标区域直接复制全文。
                if self.copyable && self.copy_rect.get().is_some_and(|rect| rect.contains(*pos)) {
                    self.copy_content(true);
                    return EventResult::Handled;
                }
                // none 只禁止普通文字选择，不阻断上方已经处理的复制按钮。
                if !self.selection_enabled() {
                    // 把事件交给其余冒泡处理器。
                    return EventResult::NotHandled;
                }
                // all 把当前文本作为原子整体，不启动可收缩的拖选会话。
                if self.user_select_policy == UserSelect::All {
                    // 先选择完整当前文本，树层随后扩展到最近 all 子树。
                    self.sel.select_all(&self.content);
                    // 当前文字节点已经消费按下事件。
                    return EventResult::Handled;
                }
                // 按下即进入拖选：Shift 扩展选区，否则重设锚点。
                self.sel.pointer_down(&self.content, *pos, mods.contains(KeyMod::SHIFT));
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                // 关闭选择时不消费普通指针移动。
                if !self.selection_enabled() {
                    // 允许其他交互组件继续观察移动事件。
                    return EventResult::NotHandled;
                }
                // 拖选中才消费移动事件并扩展选区。
                if !self.sel.pointer_move(&self.content, *pos) {
                    return EventResult::NotHandled;
                }
                EventResult::Handled
            }
            SystemEvent::PointerUp {
                button: MouseButton::Left,
                ..
            } => {
                // 关闭选择时没有需要结束的拖选会话。
                if !self.selection_enabled() {
                    // 允许事件继续冒泡。
                    return EventResult::NotHandled;
                }
                // 结束拖选并清除空选区。
                self.sel.pointer_up();
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                // 失去焦点时清除文字选中。
                self.focused = false;
                self.sel.reset_selection();
                EventResult::Handled
            }
            SystemEvent::FocusIn if self.copyable => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, mods } => {
                let ctrl = mods.contains(KeyMod::CTRL);
                match key {
                    KeyCode::A if ctrl && self.selection_enabled() => {
                        // Ctrl+A 全选当前文本。
                        self.sel.select_all(&self.content);
                        EventResult::Handled
                    }
                    KeyCode::C if ctrl && self.selection_enabled() => {
                        // 有选区复制选区，否则复制全文。
                        if let Some(selected) = self.sel.selected_text(&self.content) {
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

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_submit
            .replace(false)
            .then(|| SemanticEvent::submit(id, "copied"))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let resolved = self.visual.resolve(ctx.tokens());
        let (fs, default_weight) = self.compute_font_style();
        // 显式 Style 字重优先，其次保留 strong 构建器与标题默认语义。
        let font_weight = self.font_weight.unwrap_or_else(|| {
            // strong 是显式兼容构建器，未被 Style 覆盖时使用 bold。
            if self.strong { FontWeight::BOLD } else { default_weight }
        });
        // 为测量、布局与绘制解析唯一最终行高。
        let line_height = self.resolved_line_height(fs);
        // 在绘制阶段通过当前 Provider 主题解析语义颜色。
        let text_c = self.resolved_text_color(&resolved, ctx.tokens());
        let copy_space = if self.copyable {
            self.visual.geometry.copy_space
        } else {
            0.0
        };
        let text_width = (frame.w - copy_space).max(self.visual.geometry.min_text_width);
        let wraps = matches!(self.type_, TypographyType::Paragraph);
        let indent = if wraps {
            self.paragraph_indent(fs).min(text_width)
        } else {
            0.0
        };
        let layout_width = if wraps {
            (text_width - indent).max(self.visual.geometry.min_text_width)
        } else {
            text_width
        };
        // 把 UI 文本对齐语义转换为 draw 中性布局值。
        let h_align = self.text_align.unwrap_or_default().to_draw();

        // 单次布局：同时用于 hit-test 缓存、选中背景和文字绘制
        let opts = crate::draw::TextLayoutOptions {
            max_width: layout_width,
            max_height: self.visual.geometry.layout_max_height,
            line_height,
            word_wrap: wraps,
            // 使用 UI Style 映射后的水平对齐。
            h_align,
            v_align: self.visual.geometry.vertical_align,
            font_size: fs,
        };
        let backend_opts = crate::draw::resources::font::text_backend::TextLayoutOptions::from(opts);
        // 字体族选择与布局、选区度量和绘制共享同一最终句柄。
        let fh = crate::ui::text_family::resolve(ctx, self.font_family.as_ref());
        let mut layout = ctx
            .font_service()
            .layout_text_shared(&fh, &self.content, &backend_opts);
        if indent > 0.0 {
            // 只有段首缩进真正修改几何时才复制共享缓存布局。
            let layout = std::sync::Arc::make_mut(&mut layout);
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
        self.sel.set_draw_pos(draw_pos);
        let abs_pos = crate::core::Point::new(frame.x + draw_pos.x, frame.y + draw_pos.y);

        if !self.content.is_empty() {
            // 缓存字形 x / advance / 字符下标与行信息（选区与命中测试用）。
            self.sel.cache_layout(&layout);

            if self.mark || self.code {
                let background = if self.code {
                    resolved.code_background
                } else {
                    resolved.mark_background
                };
                for line in &layout.lines {
                    if let Some(bounds) = self.line_bounds(&layout, line, abs_pos, fs) {
                        ctx.fill_rect(
                            Rect::new(
                                bounds.x - self.visual.geometry.mark_horizontal_padding,
                                bounds.y - self.visual.geometry.mark_vertical_padding,
                                bounds.w
                                    + self.visual.geometry.mark_horizontal_padding * 2.0,
                                bounds.h + self.visual.geometry.mark_vertical_padding * 2.0,
                            ),
                            background,
                            Some(Radius::uniform(if self.code {
                                self.visual.geometry.code_radius
                            } else {
                                self.visual.geometry.mark_radius
                            })),
                        );
                    }
                }
            }

            if let Some((sel_s, sel_e)) = self.sel.selection() {
                if sel_s < sel_e {
                    let visual_h = ctx.font_service()
                        .horizontal_line_metrics(&fh, fs)
                        .map(|m| m.ascent + m.descent)
                        .unwrap_or(fs * self.visual.geometry.single_line_height_factor);
                    for line in &layout.lines {
                        let gs = line.glyph_start;
                        let ge = (gs + line.glyph_count).min(layout.glyphs.len());
                        // 双向行的逻辑选择区按连续视觉片段分别绘制。
                        for (line_x0, line_x1) in
                            crate::draw::resources::font::text_backend::glyph_selection_x_ranges(
                                // 传入当前视觉行字形。
                                &layout.glyphs[gs..ge],
                                // 传入逻辑选择起点。
                                sel_s,
                                // 传入逻辑选择终点。
                                sel_e,
                            )
                        {
                            // 将片段左缘平移到绝对绘制坐标。
                            let x0 = abs_pos.x + line_x0;
                            // 将片段右缘平移到绝对绘制坐标。
                            let x1 = abs_pos.x + line_x1;
                            // 使用当前视觉行顶部。
                            let y0 = abs_pos.y + line.y;
                            // 绘制当前连续选择片段。
                            ctx.fill_rect(
                                Rect::new(x0, y0, (x1 - x0).max(0.0), visual_h),
                                resolved
                                    .primary
                                    .with_alpha(self.visual.geometry.selection_alpha),
                                None,
                            );
                        }
                    }
                }
            }

            // 通过 UI 私有适配器选择常规或合成粗体面，不向 draw 泄漏样式类型。
            crate::ui::text_weight::paint(ctx, &layout, abs_pos, text_c, fs, font_weight);
            // 按 Style 覆盖优先级或兼容构建器收集最终装饰线段。
            let decoration_segments = self.text_decoration_segments(&layout, abs_pos, fs);
            // 通过 UI 私有适配器提交 draw System 的中性直线命令。
            crate::ui::text_decoration::paint(ctx, &decoration_segments, text_c);
        }

        // copyable 图标
        if self.copyable {
            let copy_x =
                frame.x + (frame.w - self.visual.geometry.copy_width).max(0.0);
            let copy_y = if wraps {
                frame.y
            } else {
                ctx.visual_center_y(frame, resolved.copy_center_font_size)
            };
            self.copy_rect.set(Some(Rect::new(
                copy_x - frame.x,
                copy_y - frame.y,
                self.visual.geometry.copy_width,
                self.visual.geometry.copy_height,
            )));
            let copy_frame = Rect::new(
                copy_x - self.visual.geometry.copy_paint_offset,
                copy_y - self.visual.geometry.copy_paint_offset,
                self.visual.geometry.copy_width,
                self.visual.geometry.copy_height,
            );
            if self.focused && tree.keyboard_focus_visible() {
                ctx.stroke_rect(
                    copy_frame,
                    resolved.primary,
                    self.visual.geometry.copy_focus_stroke,
                    Some(Radius::uniform(self.visual.geometry.copy_focus_radius)),
                );
            }
            Icon::paint_in_frame(
                ctx,
                self.visual.geometry.copy_icon,
                copy_frame,
                resolved.copy_text,
                self.visual.geometry.copy_icon_size,
            );
        } else {
            self.copy_rect.set(None);
        }
    }
}

// 把 Typography Rust 内核与 UIX 静态视觉组合为单一叶节点。
fn build_typography_view(mut kernel: Typography, visual: &'static TypographyVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Typography {
    fn build(self) -> ViewNode {
        build_typography_uix_root(self)
    }
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_typography_uix_root(kernel: Typography) -> ViewNode {
    crate::uix!("src/ui/widgets/general/typography/typography.uix")
}

impl Typography {
    /// 使用文本内容和指定语义层级创建排版组件。
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
            // 没有结构声明时 Typography 保留既有默认可选语义。
            user_select_policy: UserSelect::Auto,
            // 默认沿用当前主题的正文颜色。
            semantic_color: None,
            color_override: None,
            spacing: 0.0,
            // 未声明统一样式时保留绘制上下文当前系统字体。
            font_family: None,
            // 未声明统一样式时保留标题与 strong 的既有字重。
            font_weight: None,
            // 未声明时保持 Typography 既有 normal 或 spacing 语义。
            line_height: None,
            // 未声明统一样式时保持既有左对齐语义。
            text_align: None,
            // 未声明统一样式时保留 underline/delete 构建器语义。
            text_decoration: None,
            indent: 0.0,
            ellipsis: false,
            sel: PerNodeTextSelection::new(),
            copy_rect: Cell::new(None),
            focused: false,
            pending_submit: Cell::new(false),
            visual: TYPOGRAPHY_VISUAL_REF,
        }
    }
    /// 创建标题，并将级别夹取到一至五级。
    pub fn heading(content: &str, level: u8) -> Self {
        Self::new(content, Self::type_for_level(level))
    }

    /// 创建一级标题。
    pub fn title(content: &str) -> Self {
        Self::heading(content, 1)
    }

    /// 将当前组件切换到夹取后一至五级的标题层级。
    pub fn level(mut self, level: u8) -> Self {
        self.type_ = Self::type_for_level(level);
        self
    }
    /// 创建支持折行、行距与缩进的段落文本。
    pub fn paragraph(content: &str) -> Self {
        Self::new(content, TypographyType::Paragraph)
    }
    /// 创建普通行内文本。
    pub fn text(content: &str) -> Self {
        Self::new(content, TypographyType::Text)
    }
    /// 设置组件是否使用禁用态文字样式并拒绝复制交互。
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    /// 为文本启用标记高亮样式。
    pub fn mark(mut self) -> Self {
        self.mark = true;
        self
    }
    /// 为文本启用行内代码样式。
    pub fn code(mut self) -> Self {
        self.code = true;
        self
    }
    /// 为文本启用下划线装饰。
    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }
    /// 为文本启用删除线装饰。
    pub fn delete(mut self) -> Self {
        self.delete = true;
        self
    }
    /// 为文本启用加粗样式。
    pub fn strong(mut self) -> Self {
        self.strong = true;
        self
    }
    /// 为文本启用斜体样式。
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
    /// 设置固定文字颜色，并清除较早配置的语义颜色。
    pub fn color(mut self, c: Color) -> Self {
        // 固定颜色成为最新局部颜色配置。
        self.color_override = Some(c);
        // 清除较早的语义颜色以保持构建器后调用优先。
        self.semantic_color = None;
        self
    }
    /// 设置随主题 Provider 解析的语义文字颜色，并清除固定颜色。
    pub fn semantic_color(
        // 接收主题模块拥有的稳定颜色值契约。
        mut self,
        // 保存待绘制阶段解析的颜色值。
        color: ColorValue,
    ) -> Self {
        // 记录局部语义颜色，并保持固定颜色覆盖入口兼容。
        self.semantic_color = Some(color);
        // 清除较早的固定颜色以保持构建器后调用优先。
        self.color_override = None;
        // 返回构建后的排版组件。
        self
    }
    /// 设置是否显示复制入口并允许键盘复制选中文本。
    pub fn copyable(mut self, v: bool) -> Self {
        self.copyable = v;
        self
    }

    // 接收 WidgetTree 已结合祖先约束解析出的最终选择策略。
    pub(crate) fn set_user_select_policy(&mut self, value: UserSelect) {
        // 保存新策略供事件与跨节点参与资格共同读取。
        self.user_select_policy = value;
        // none 切换必须终止当前选区和拖选生命周期。
        if !self.selection_enabled() {
            // 复用共享选择状态的完整重置入口。
            self.sel.reset_selection();
        }
    }

    // 判断结构策略与 Typography 默认能力组合后的最终选择能力。
    pub(crate) fn selection_enabled(&self) -> bool {
        // Typography 的组件默认值为允许普通文字选择。
        self.user_select_policy.allows_text(true)
    }

    /// 设置段落行高相对字号的倍率；非有限值归零。
    pub fn spacing(mut self, value: f32) -> Self {
        self.spacing = if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        };
        self
    }

    /// 设置段落首行缩进相对字号的倍率；非有限值归零。
    pub fn indent(mut self, value: f32) -> Self {
        self.indent = if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        };
        self
    }

    /// 启用单行省略显示。
    pub fn ellipsis(mut self) -> Self {
        self.ellipsis = true;
        self
    }

    /// 返回当前选区中的文本；没有有效选区时返回空值。
    pub fn selected_text(&self) -> Option<String> {
        self.sel.selected_text(&self.content)
    }

    /// 返回组件当前是否持有用于复制交互的焦点。
    pub fn is_copy_focused(&self) -> bool {
        self.focused
    }

    pub(crate) fn is_cross_text_dragging(&self) -> bool {
        // 禁止策略不允许旧拖选状态继续参与树协调。
        self.selection_enabled() && self.sel.is_dragging()
    }

    pub(crate) fn cross_text_len(&self) -> usize {
        self.sel.cross_text_len(&self.content)
    }

    pub(crate) fn cross_text_anchor(&self) -> usize {
        self.sel.cross_text_anchor()
    }

    pub(crate) fn set_cross_text_range(&self, range: Option<(usize, usize)>) {
        // 禁止策略不能经树级协调重新建立选区。
        if !self.selection_enabled() {
            // 防御性清除可能残留的旧范围。
            self.sel.set_cross_text_range(&self.content, None);
            // 结束当前范围更新。
            return;
        }
        self.sel.set_cross_text_range(&self.content, range);
    }

    pub(crate) fn cross_text_char_at(&self, frame_local: crate::core::Point) -> usize {
        self.sel.cross_text_char_at(&self.content, frame_local)
    }

    fn compute_font_style(&self) -> (f32, FontWeight) {
        let index = match self.type_ {
            TypographyType::Heading1 => 0,
            TypographyType::Heading2 => 1,
            TypographyType::Heading3 => 2,
            TypographyType::Heading4 => 3,
            TypographyType::Heading5 => 4,
            TypographyType::Paragraph => 5,
            TypographyType::Text => 6,
        };
        let level = self.visual.levels[index];
        (level.font_size, level.font_weight)
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

    fn resolved_line_height(&self, font_size: f32) -> f32 {
        // UIX Style 显式行高优先于 Typography 专有 spacing。
        if let Some(line_height) = self.line_height {
            // 倍率与像素值均按最终字号解析。
            return line_height.resolve(font_size);
        }
        // 未声明时段落 spacing 保持现有构建器优先级。
        let factor = if matches!(self.type_, TypographyType::Paragraph) && self.spacing > 0.0 {
            // 只有段落类型消费专有 spacing 构建器。
            self.spacing
        } else {
            // 标题、普通文本与未声明段落保持既有 normal 倍率。
            self.visual.geometry.normal_line_height_factor
        };
        let line_height = font_size * factor;
        if line_height.is_finite() && line_height > 0.0 {
            line_height
        } else {
            font_size * self.visual.geometry.normal_line_height_factor
        }
    }

    fn paragraph_indent(&self, font_size: f32) -> f32 {
        self.indent * font_size
    }

    // 按统一样式优先级生成当前排版组件的文本装饰线段。
    fn text_decoration_segments(
        // 借用与文字绘制相同的布局。
        &self,
        // 借用稳定的文本布局类型。
        layout: &crate::draw::resources::font::text_backend::TextLayout,
        // 接收文字的绝对绘制原点。
        origin: Point,
        // 接收最终语义字号。
        font_size: f32,
    ) -> Vec<crate::ui::text_decoration::DecorationSegment> {
        // 显式统一样式拥有最高优先级，包括显式 none。
        if let Some(decoration) = self.text_decoration {
            // 只生成统一样式指定的一种闭合装饰。
            return crate::ui::text_decoration::segments(layout, origin, font_size, decoration);
        }
        // 未声明 Style 时保留既有两个布尔构建器可同时启用的行为。
        let mut segments = Vec::new();
        // 兼容既有下划线构建器。
        if self.underline {
            // 追加每个视觉行的下划线。
            segments.extend(crate::ui::text_decoration::segments(
                // 复用文字布局。
                layout,
                // 复用文字原点。
                origin,
                // 复用最终字号。
                font_size,
                // 映射既有 underline 标记。
                TextDecoration::Underline,
            ));
        }
        // 兼容既有删除线构建器。
        if self.delete {
            // 追加每个视觉行的删除线。
            segments.extend(crate::ui::text_decoration::segments(
                // 复用文字布局。
                layout,
                // 复用文字原点。
                origin,
                // 复用最终字号。
                font_size,
                // 映射既有 delete 标记。
                TextDecoration::LineThrough,
            ));
        }
        // 返回兼容构建器产生的零至两组线段。
        segments
    }

    fn intrinsic_size(&self) -> Size {
        let (fs, _fw) = self.compute_font_style();
        let copy_space = if self.copyable {
            self.visual.geometry.copy_space
        } else {
            0.0
        };
        let w =
            self.content.chars().count() as f32 * fs * self.visual.geometry.average_advance_factor
                + copy_space;
        // 与 Label 一致：单行用视觉字高，避免光学居中后量高偏大
        let h = self
            // 只有显式样式行高改变单行固有高度。
            .line_height
            // 按语义字号解析行盒。
            .map(|line_height| line_height.resolve(fs))
            // 未声明时保留既有视觉字高。
            .unwrap_or(fs * self.visual.geometry.single_line_height_factor);
        Size::new(w, h)
    }

    fn copy_content(&self, emit_submit: bool) {
        clipboard::copy_to_clipboard(&self.content);
        if emit_submit {
            self.pending_submit.set(true);
        }
    }

    fn line_bounds(
        &self,
        layout: &crate::draw::resources::font::text_backend::TextLayout,
        line: &crate::draw::resources::font::text_backend::LineInfo,
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
            line.height
                .max(font_size * self.visual.geometry.single_line_height_factor),
        ))
    }

    // 测试目标保留复制按钮区域观测入口，供排版交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn copy_rect_for_test(&self) -> Option<Rect> {
        self.copy_rect.get()
    }

    // 测试目标保留渲染行起点观测入口，供排版布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn rendered_line_origins_for_test(&self) -> Vec<Point> {
        self.sel.rendered_line_origins_for_test()
    }

    // 按局部属性优先级解析当前文字颜色。
    fn resolved_text_color(
        // 借用当前排版组件。
        &self,
        // 借用绘制阶段恢复的主题令牌。
        resolved: &ResolvedTypographyVisual,
        tokens: &dyn ThemeTokens,
    ) -> Color {
        // 固定颜色保持最高优先级，兼容既有 color 构建器。
        self.color_override
            // 没有固定覆盖时解析主题感知颜色。
            .or_else(|| self.semantic_color.map(|color| color.resolve(tokens)))
            // 没有局部颜色时按禁用状态读取主题文字令牌。
            .unwrap_or_else(|| {
                // 禁用文字使用四级文本色。
                if self.disabled {
                    // 返回主题禁用色。
                    resolved.disabled_text
                } else {
                    // 返回主题正文色。
                    resolved.text
                }
            })
    }
}

impl Typography {
    // 从 View 适配边界接收本组件实际消费的样式字段。
    pub(crate) fn apply_view_style(&mut self, style: &Style) {
        // 克隆有序列表以脱离 View Style 生命周期并参与组件协调。
        self.font_family = style.font_family.clone();
        // 显式 Some 包括 normal，能够覆盖标题或 strong 默认字重。
        self.font_weight = style.font_weight;
        // next widget 的默认 None 会在 reconcile 时清除旧值；这里只复制当前显式值。
        self.line_height = style.line_height;
        // 显式 Some 包括 left，能够覆盖继承的其他对齐值。
        self.text_align = style.text_align;
        // 显式 Some 包括 none，能够覆盖局部 underline/delete 构建器。
        self.text_decoration = style.text_decoration;
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        if self.content != next.content {
            self.content = next.content;
            // 文本变更：清空布局缓存并重置选区状态。
            self.sel.clear_caches();
            self.sel.reset_selection();
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
        // 同步声明式语义颜色，确保主题切换仍在绘制阶段解析。
        self.semantic_color = next.semantic_color;
        self.color_override = next.color_override;
        self.spacing = next.spacing;
        // 同步显式字体族列表以触发布局和绘制快照差异。
        self.font_family = next.font_family;
        // 同步显式字体粗细以触发绘制快照差异。
        self.font_weight = next.font_weight;
        // 同步显式行高以触发布局快照差异。
        self.line_height = next.line_height;
        // 同步显式文本对齐以触发绘制快照差异。
        self.text_align = next.text_align;
        // 同步显式文本装饰以触发绘制快照差异。
        self.text_decoration = next.text_decoration;
        self.indent = next.indent;
        self.ellipsis = next.ellipsis;
        self.visual = next.visual;
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
            // 快照保留主题值身份而不是提前固化为某个主题的 RGB。
            semantic_color: self.semantic_color,
            color_override: self.color_override,
            // 快照保留字体族声明顺序与未声明身份。
            font_family: self.font_family.clone(),
            // 快照保留未声明与显式 normal 的差异。
            font_weight: self.font_weight,
            // 快照保留行高单位和值以支持精确布局失效。
            line_height: self.line_height,
            // 快照保留未声明与显式 left 的差异。
            text_align: self.text_align,
            // 快照保留未声明与显式 none 的差异。
            text_decoration: self.text_decoration,
        }
    }
}

// 只在单元测试目标验证主题值与统一文本样式适配契约。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/general/typography/tests.rs"]
mod tests;
