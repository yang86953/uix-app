//! Label 的 Rust 文本选择、布局与绘制内核。
//!
//! 默认不可选中：导航/标题等 UI 文案不应出现拖选高亮。
//! 需要复制选区时调用 `.selectable()`。

use crate::core::{Constraints, Rect, Size};
use crate::draw::geometry::spatial::PhysicalUnit;
use crate::draw::{Color, TextLayoutOptions, VAlign};
use crate::ui::widget_runtime::clipboard;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::widget;
// 引入共享的单节点文字选区实现。
use crate::ui::text_selection::per_node::PerNodeTextSelection;

use crate::ui::theme::style::ColorValue;
use crate::ui::theme::style::Style;
use crate::ui::{EventResult, KeyCode, KeyMod, MouseButton, SystemEvent, UserSelect, WidgetTree};
use crate::ui::{PrimitiveSnapshot, SnapshotSource};
use crate::ui::{ThemeTokens, View, ViewNode};

// 保存 Label 的字号、行盒、度量与选择背景静态视觉。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LabelMetricsVisual {
    pub(crate) default_font_size: f32,
    pub(crate) default_line_height_factor: f32,
    pub(crate) single_line_height_factor: f32,
    pub(crate) measurement_dpi: f32,
    pub(crate) selection_alpha: u8,
    pub(crate) layout_max_height: f32,
    pub(crate) word_wrap: bool,
    pub(crate) vertical_align: VAlign,
}

// 保存 Label 默认文字色与选区强调色的主题角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LabelPaletteVisual {
    text: ColorValue,
    primary: ColorValue,
}

// 全部 Label 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LabelVisual {
    pub(crate) metrics: LabelMetricsVisual,
    palette: LabelPaletteVisual,
}

// 保存每帧从主题解析出的 Label 色值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedLabelVisual {
    text: Color,
    primary: Color,
}

impl LabelVisual {
    fn resolve(self, tokens: &dyn ThemeTokens) -> ResolvedLabelVisual {
        ResolvedLabelVisual {
            text: self.palette.text.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
        }
    }
}

pub(crate) const fn label_vertical_align_top() -> VAlign {
    VAlign::Top
}

pub(crate) const fn label_text_color() -> ColorValue {
    ColorValue::Token("uix.foreground", crate::draw::Color::black())
}

pub(crate) const fn label_primary_color() -> ColorValue {
    ColorValue::Token("uix.selection", crate::draw::Color::from_rgb(64, 128, 192))
}

fn normalized_label_font_size(size: f32) -> f32 {
    if size.is_finite() && size > 0.0 {
        crate::draw::resources::font::text_backend::bounded_font_size(size)
    } else {
        LABEL_VISUAL.metrics.default_font_size
    }
}

widget! {
    /// 支持主题样式、固定尺寸与可选文本选择的单样式标签组件。
    pub struct Label {
        /// 标签当前持有并参与布局、绘制与选择的文本。
        pub text: String,
        /// 未设置物理单位字号时使用的逻辑像素字号。
        pub font_size: f32,
        #[snapshot(skip)]
        font_size_authored: bool,
        #[snapshot(skip)]
        /// 在最终内容宽度内折行；默认 Label 保持不折行，UIX Text 启用。
        word_wrap: bool,
        /// 物理单位字号（可选，优先级高于 font_size）。
        pub font_size_unit: Option<PhysicalUnit>,
        /// 可选文字颜色覆写；`None` 使用主题正文色。
        pub color: Option<crate::draw::Color>,
        /// 可选固定布局宽度。
        pub fixed_width: Option<f32>,
        /// 可选固定布局高度。
        pub fixed_height: Option<f32>,
        /// 是否允许拖选 / Ctrl+A / Ctrl+C 选区。默认 false。
        selectable: bool,
        /// 由 WidgetTree 结合祖先声明解析出的最终选择策略。
        user_select_policy: UserSelect,
        /// 共享的选区状态：布局缓存、选区、拖选锚点与绘制偏移。
        sel: PerNodeTextSelection,
        /// 统一样式覆盖（优先于 color/font_size 独立字段）。
        pub(crate) style: Option<Style>,
        /// 同目录 UIX 生成的唯一静态视觉表。
        pub(crate) visual: &'static LabelVisual,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size(constraints))
    }

    flex_grow => (&self) -> f32 {
        self.style.as_ref().map(|s| s.flex_grow).unwrap_or(0.0)
    }

    flex_shrink => (&self) -> f32 {
        self.style.as_ref().map(|s| s.flex_shrink).unwrap_or(1.0)
    }

    layout_margin => (&self) -> crate::core::EdgeInsets {
        self.style.as_ref().map(|style| style.margin).unwrap_or_default()
    }

    align_self => (&self) -> Option<crate::ui::layout::AlignItems> {
        self.style.as_ref().and_then(|style| style.align_self)
    }

    grid_cell => (&self) -> Option<usize> {
        self.style.as_ref().and_then(|style| style.grid_cell)
    }

    grid_column_span => (&self) -> u32 {
        self.style.as_ref().map(|style| style.grid_column_span).unwrap_or(1)
    }

    grid_row_span => (&self) -> u32 {
        self.style.as_ref().map(|style| style.grid_row_span).unwrap_or(1)
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        // 默认不可选中：整个事件处理直接让出。
        if !self.selection_enabled() {
            return EventResult::NotHandled;
        }
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                mods,
            } => {
                // all 把当前文本作为原子整体，不启动可收缩的拖选会话。
                if self.user_select_policy == UserSelect::All {
                    // 先选择完整当前文本，树层随后扩展到最近 all 子树。
                    self.sel.select_all(&self.text);
                    // 当前文字节点已经消费按下事件。
                    return EventResult::Handled;
                }
                // 按下即进入拖选：Shift 扩展选区，否则重设锚点。
                self.sel.pointer_down(&self.text, *pos, mods.contains(KeyMod::SHIFT));
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                // 拖选中才消费移动事件并扩展选区。
                if !self.sel.pointer_move(&self.text, *pos) {
                    return EventResult::NotHandled;
                }
                EventResult::Handled
            }
            SystemEvent::PointerUp {
                button: MouseButton::Left,
                ..
            } => {
                // 结束拖选并清除空选区。
                self.sel.pointer_up();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, mods } => {
                let ctrl = mods.contains(KeyMod::CTRL);
                match key {
                    KeyCode::A if ctrl => {
                        // Ctrl+A 全选当前文本。
                        self.sel.select_all(&self.text);
                        EventResult::Handled
                    }
                    KeyCode::C if ctrl => {
                        // 有选区复制选区，否则复制全文。
                        if let Some(selected) = self.sel.selected_text(&self.text) {
                            clipboard::copy_to_clipboard(&selected);
                        } else {
                            clipboard::copy_to_clipboard(&self.text);
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
        let resolved = self.visual.resolve(ctx.tokens());
        // 统一样式优先：背景/边框（section 色条等）再绘制文字
        if let Some(style) = self.style.as_ref() {
            crate::ui::theme::style::apply_style(ctx, frame, style);
        }

        // 统一样式优先：style.color > self.color > theme default
        let c = if let Some(style) = self.style.as_ref() {
            style.resolve_color(ctx.tokens())
        } else {
            self.color.unwrap_or(resolved.text)
        };
        // 分辨率：物理单位优先 > style > self.font_size
        let fs = self.resolved_font_size(ctx.dpi(), ctx.tokens());
        let fs = normalized_label_font_size(fs);
        // 显式 lineHeight 同时驱动文本布局与固有测量。
        let line_height = self
            // 只在存在统一样式时读取显式值。
            .style
            // 借用样式而不取得其所有权。
            .as_ref()
            // 按最终字号解析倍率或像素值。
            .and_then(|style| style.resolve_line_height(fs))
            // 未声明时保持 Label 既有 normal 行高。
            .unwrap_or(fs * self.visual.metrics.default_line_height_factor);
        // 提前解析内边距，使文本对齐使用真实内容框宽度。
        let pad = self
            // 借用可选统一样式。
            .style
            // 只读取布局已经消费的内边距。
            .as_ref()
            // 提取内容框内边距。
            .map(|s| s.padding)
            // 未声明样式时不增加内边距。
            .unwrap_or_default();
        // 从 Style 解析闭合对齐值并转换为 draw 中性契约。
        let h_align = self
            // 借用可选统一样式。
            .style
            // 只对存在的样式读取有效对齐。
            .as_ref()
            // 显式 left 仍保留覆盖语义。
            .map(Style::effective_text_align)
            // 无样式时保持 Label 既有左对齐。
            .unwrap_or_default()
            // 在 UI 边界完成到 draw 值的单向适配。
            .to_draw();
        // 最终分配的内容框既用于折行，也作为绘制硬边界。
        let content_rect = Rect::new(
            frame.x + pad.left,
            frame.y + pad.top,
            (frame.w - pad.horizontal()).max(0.0),
            (frame.h - pad.vertical()).max(0.0),
        );
        if content_rect.w <= 0.0 || content_rect.h <= 0.0 {
            self.sel.clear_caches();
            return;
        }

        // 单次布局：同时用于 hit-test 缓存、选中背景和文字绘制
        let opts = TextLayoutOptions {
            // 对齐使用最终内容框宽度。
            max_width: content_rect.w,
            max_height: content_rect.h,
            line_height,
            word_wrap: self.word_wrap || self.visual.metrics.word_wrap,
            // 使用 UI Style 映射后的水平对齐。
            h_align,
            v_align: self.visual.metrics.vertical_align,
            font_size: fs,
        };
        let backend_opts = crate::draw::resources::font::text_backend::TextLayoutOptions::from(opts);
        // 字体族选择与布局、选区度量和绘制共享同一最终句柄。
        let fh = crate::ui::text_family::resolve(
            // 传入当前绘制上下文。
            ctx,
            // 从统一样式借用显式有序字体族列表。
            self.style.as_ref().and_then(|style| style.font_family.as_ref()),
        );
        let layout = ctx
            .font_service()
            .layout_text_shared(&fh, &self.text, &backend_opts);

        // 内边距 + 顶对齐绘制。过高 frame 时的垂直居中由父级 AlignItems 负责，
        // 不在此用 visual_center_y 二次修正。
        let draw_pos = crate::core::Point::new(pad.left, pad.top);
        self.sel.set_draw_pos(draw_pos);
        let abs_pos = crate::core::Point::new(frame.x + draw_pos.x, frame.y + draw_pos.y);

        ctx.push_clip(content_rect);
        if !self.text.is_empty() {
            // 缓存字形 x / advance / 字符下标与行信息（选区与命中测试用）。
            self.sel.cache_layout(&layout);

            // 绘制选中背景（按字符下标匹配字形）
            if let Some((sel_s, sel_e)) = self.sel.selection() {
                if sel_s < sel_e {
                    // 选区背景至少覆盖最终行盒，避免大行高留下未选中的垂直空隙。
                    let selection_height = ctx.font_service()
                        .horizontal_line_metrics(&fh, fs)
                        .map(|m| m.ascent + m.descent)
                        // 显式或默认行高与实际字形高度取较大值。
                        .map(|glyph_height| glyph_height.max(line_height))
                        // 缺少字体度量时直接使用最终行高。
                        .unwrap_or(line_height);
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
                                Rect::new(x0, y0, (x1 - x0).max(0.0), selection_height),
                                resolved
                                    .primary
                                    .with_alpha(self.visual.metrics.selection_alpha),
                                None,
                            );
                        }
                    }
                }
            }

            // 在移动拥有型布局前按显式 Style 解析文本装饰。
            let decoration = self
                // 借用可选统一样式。
                .style
                // 只对显式存在的样式解析公共默认语义。
                .as_ref()
                // 从 Style 取得最终闭合装饰值。
                .map(Style::effective_text_decoration)
                // 没有统一样式时 Label 默认不绘制装饰。
                .unwrap_or_default();
            // 预计算线段以允许后续移动布局所有权。
            let decoration_segments = crate::ui::text_decoration::segments(
                // 复用文字与选区使用的同一布局。
                &layout,
                // 使用文字绘制的绝对原点。
                abs_pos,
                // 使用最终解析字号。
                fs,
                // 使用 Style 解析出的装饰值。
                decoration,
            );
            // 从统一样式解析最终字重；无样式时采用文档默认 normal。
            let font_weight = self
                // 借用可选统一样式。
                .style
                // 只对存在的样式读取有效字重。
                .as_ref()
                // 显式 normal 仍保留覆盖语义。
                .map(Style::effective_font_weight)
                // 无样式时使用常规字重。
                .unwrap_or_default();
            // 通过 UI 私有适配器绘制常规或合成粗体文字。
            crate::ui::text_weight::paint(ctx, &layout, abs_pos, c, fs, font_weight);
            // 在文字上方提交装饰直线，确保删除线和上下划线可见。
            crate::ui::text_decoration::paint(ctx, &decoration_segments, c);
        }
        ctx.pop_clip();
    }

    reconcile_sync => (sync_from) {}

    snapshot => (&self) -> crate::ui::WidgetSnapshotFields {
        <Self as crate::ui::SnapshotSource>::snapshot_fields(self)
    }

    reconcile_layout => (&self, next: &dyn crate::ui::Widget) -> Option<bool> {
        let Some(next) = next.as_any().downcast_ref::<Self>() else { return None; };
        self.typography_changed(next).then_some(true)
    }

    reconcile_runtime => (&self, next: &dyn crate::ui::Widget) -> bool {
        let Some(next) = next.as_any().downcast_ref::<Self>() else { return false; };
        self.typography_changed(next)
    }

    declaration_style => (&mut self, style: &crate::ui::Style, declared: &crate::ui::StyleDiff, flex_grow_override: Option<f32>, flex_shrink_override: Option<f32>) {
        let l = self;
        let style_is_default = style == &crate::ui::Style::default();
        let _ = (style_is_default, declared, flex_grow_override, flex_shrink_override);

                let mut merged = l.style.clone().unwrap_or_default().apply(style.clone());
                // 显式零/默认声明在值合并之上恢复，初始非默认样式上的
                // padding:0、width:auto 等覆盖不再被值推断当作未声明。
                if !declared.is_empty() {
                    declared.clone().apply_to(&mut merged);
                }
                if let Some(g) = flex_grow_override {
                    merged.flex_grow = g;
                }
                if let Some(s) = flex_shrink_override {
                    merged.flex_shrink = s;
                }
                // ViewNode width/height → Label 固定尺寸（section 色条等）。
                // 显式 width/height 声明进入固定尺寸，显式 auto 清空固定尺寸
                // （含 builder 预设）；未声明保留组件既有配置。
                if let Some(w) = declared.width {
                    l.fixed_width = w;
                } else if let Some(w) = style.width {
                    l.fixed_width = Some(w);
                }
                if let Some(h) = declared.height {
                    l.fixed_height = h;
                } else if let Some(h) = style.height {
                    l.fixed_height = Some(h);
                }
                l.style = Some(merged);

    }

    text_selection => () {}
}

// 把 Label Rust 内核与 UIX 静态视觉组合为单一叶节点。
fn build_label_view(mut kernel: Label, visual: &'static LabelVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Label {
    fn build(self) -> ViewNode {
        build_label_uix_root(self)
    }
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_label_uix_root(kernel: Label) -> ViewNode {
    build_label_view(kernel, LABEL_VISUAL_REF)
}

impl SnapshotSource for Label {
    fn snapshot_fields(&self) -> crate::ui::WidgetSnapshotFields {
        let fields = {
            PrimitiveSnapshot::Label {
                text: self.text.clone(),
                font_size: self.font_size,
                font_size_unit: self.font_size_unit,
                color: self.color,
                fixed_width: self.fixed_width,
                fixed_height: self.fixed_height,
                style: self.style.clone(),
            }
        };
        fields.into()
    }
}

impl Label {
    // Builder flags preserve explicit default-sized text under a custom theme;
    // direct public-field overrides retain compatibility when non-default.
    pub(crate) fn typography_changed(&self, next: &Self) -> bool {
        self.font_size_authored != next.font_size_authored || self.word_wrap != next.word_wrap
    }

    fn resolved_font_size(&self, dpi: f32, tokens: &dyn crate::ui::ThemeTokens) -> f32 {
        if let Some(unit) = self.font_size_unit {
            unit.to_dip(dpi)
        } else if let Some(style) = self.style.as_ref().filter(|style| {
            style.font_size != crate::ui::theme::style::TypographyToken::default()
                || (!self.font_size_authored
                    && self.font_size == self.visual.metrics.default_font_size)
        }) {
            style.resolve_font_size(tokens)
        } else if self.font_size_authored || self.font_size != self.visual.metrics.default_font_size
        {
            self.font_size
        } else {
            tokens.number("uix.font-size", 14.0)
        }
    }

    /// 创建使用默认字号、主题颜色且不可选中的文本标签。
    pub fn new(text: impl Into<String>) -> Self {
        let t = text.into();
        Self {
            text: t,
            font_size: LABEL_VISUAL.metrics.default_font_size,
            font_size_authored: false,
            word_wrap: false,
            font_size_unit: None,
            color: None,
            fixed_width: None,
            fixed_height: None,
            selectable: false,
            // 未挂载或没有结构声明时保留 Label 默认不可选语义。
            user_select_policy: UserSelect::Auto,
            sel: PerNodeTextSelection::new(),
            style: None,
            visual: LABEL_VISUAL_REF,
        }
    }

    /// 在父布局分配的内容宽度内折行，并按实际行数测量高度。
    pub fn word_wrap(mut self, enabled: bool) -> Self {
        self.word_wrap = enabled;
        self
    }

    /// 允许拖选与快捷键复制选区（导航/标题等默认关闭）。
    pub fn selectable(mut self) -> Self {
        self.selectable = true;
        self
    }

    // 接收 WidgetTree 已结合祖先约束解析出的最终选择策略。
    pub(crate) fn set_user_select_policy(&mut self, value: UserSelect) {
        // 保存新策略供事件与跨节点参与资格共同读取。
        self.user_select_policy = value;
        // 策略关闭当前组件选择能力时立即清理旧选区和拖选。
        if !self.selection_enabled() {
            // 复用共享选择状态的完整重置入口。
            self.sel.reset_selection();
        }
    }

    // 判断结构策略与组件显式构建器组合后的最终选择能力。
    fn selection_enabled(&self) -> bool {
        // auto 保留 selectable 构建器，其余值由公开策略决定。
        self.user_select_policy.allows_text(self.selectable)
    }

    /// 设置统一样式（覆盖文字颜色/字号等视觉属性）。
    pub fn style(mut self, s: Style) -> Self {
        self.style = Some(s);
        self
    }

    /// 返回标签当前持有的文本。
    pub fn text(&self) -> &str {
        &self.text
    }

    /// 替换标签文本，并清除旧布局缓存与文本选区。
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        // 文本变更：清空布局缓存并重置选区状态。
        self.sel.clear_caches();
        self.sel.reset_selection();
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        if self.text != next.text {
            self.set_text(next.text);
        }
        self.font_size = next.font_size;
        self.font_size_authored = next.font_size_authored;
        self.word_wrap = next.word_wrap;
        self.font_size_unit = next.font_size_unit;
        self.color = next.color;
        self.fixed_width = next.fixed_width;
        self.fixed_height = next.fixed_height;
        self.selectable = next.selectable;
        // 关闭可选中时同步清除残留选区。
        if !self.selection_enabled() {
            self.sel.reset_selection();
        }
        self.style = next.style;
        self.visual = next.visual;
    }

    /// 设置标签文字颜色。
    pub fn color(mut self, c: crate::draw::Color) -> Self {
        self.color = Some(c);
        self
    }

    /// 设置逻辑像素字号并清除物理单位字号；非法值回退默认字号。
    pub fn font_size(mut self, s: f32) -> Self {
        self.font_size = normalized_label_font_size(s);
        self.font_size_authored = true;
        self.font_size_unit = None;
        self
    }

    /// 设置物理单位字号（优先级高于 `font_size()`）。
    /// 自动适配 DPI：`12.pt()` 在不同屏幕上物理尺寸一致。
    pub fn font_size_unit(mut self, unit: PhysicalUnit) -> Self {
        self.font_size_unit = Some(unit);
        self
    }

    /// 设置标签请求的固定宽度与高度。
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.fixed_width = Some(w);
        self.fixed_height = Some(h);
        self
    }

    /// 返回当前非空文本选区的拥有型内容；没有选区时返回 `None`。
    pub fn selected_text(&self) -> Option<String> {
        self.sel.selected_text(&self.text)
    }

    pub(crate) fn participates_in_cross_text_selection(&self) -> bool {
        // 使用结构策略与显式构建器组合后的最终能力。
        self.selection_enabled()
    }

    pub(crate) fn is_cross_text_dragging(&self) -> bool {
        // 拖选状态同样以可选中为前提。
        self.selection_enabled() && self.sel.is_dragging()
    }

    pub(crate) fn cross_text_len(&self) -> usize {
        self.sel.cross_text_len(&self.text)
    }

    pub(crate) fn cross_text_anchor(&self) -> usize {
        self.sel.cross_text_anchor()
    }

    pub(crate) fn set_cross_text_range(&self, range: Option<(usize, usize)>) {
        // 不可选中的 Label 不接收跨节点选区。
        if !self.selection_enabled() {
            return;
        }
        self.sel.set_cross_text_range(&self.text, range);
    }

    pub(crate) fn cross_text_char_at(&self, frame_local: crate::core::Point) -> usize {
        self.sel.cross_text_char_at(&self.text, frame_local)
    }

    fn intrinsic_size(&self, constraints: Constraints) -> Size {
        let style_w = self.style.as_ref().and_then(|s| s.width);
        let style_h = self.style.as_ref().and_then(|s| s.height);
        let pad = self.style.as_ref().map(|s| s.padding).unwrap_or_default();
        let w = self.fixed_width.or(style_w);
        let h = self.fixed_height.or(style_h);
        if let (Some(w), Some(h)) = (w, h) {
            Size::new(w, h)
        } else {
            let raw_font_size =
                crate::ui::widget_runtime::measurement::with_measurement_tokens::<Self, _>(
                    |tokens| self.resolved_font_size(self.visual.metrics.measurement_dpi, tokens),
                );
            let fs = normalized_label_font_size(raw_font_size);
            // 显式行高按最终字号解析，未声明时继续使用既有测量策略。
            let explicit_line_height = self
                // 读取可选统一样式。
                .style
                // 借用样式以保留组件所有权。
                .as_ref()
                // 只解析显式 lineHeight。
                .and_then(|style| style.resolve_line_height(fs));
            let available_width = w.unwrap_or(constraints.max.w).min(constraints.max.w);
            let estimated = crate::ui::widget_runtime::measurement::text_metrics(
                &self.text,
                (available_width - pad.horizontal()).max(0.0),
                fs,
                explicit_line_height.unwrap_or(fs * self.visual.metrics.default_line_height_factor),
                self.style.as_ref().and_then(|s| s.font_family.as_ref()),
                self.word_wrap || self.visual.metrics.word_wrap,
            );
            // 单行和多行使用同一 normal 行盒，与顶对齐绘制一致。
            // 与 Icon 同行时由父级 AlignItems::Center 对齐，勿在 paint 里二次居中。
            // 显式多行保留完整 line box，禁止后继节点压到实际字形上。
            let text_height = if let Some(line_height) = explicit_line_height {
                // 显式行高对单行与多行统一生效。
                line_height * estimated.line_count as f32
            } else if estimated.line_count == 1 {
                // 未声明时使用与动态文字一致的 normal 行高。
                fs * self.visual.metrics.single_line_height_factor
            } else {
                // 未声明时保留现有多行 normal 行高。
                fs * self.visual.metrics.default_line_height_factor * estimated.line_count as f32
            };
            Size::new(
                w.unwrap_or(if estimated.width_wrapped {
                    available_width
                } else {
                    (estimated.max_line_width + pad.horizontal()).min(available_width)
                }),
                h.unwrap_or(text_height + pad.vertical()),
            )
        }
    }
}

impl crate::ui::WidgetTextSelection for Label {
    fn selection_enabled(&self) -> bool {
        self.participates_in_cross_text_selection()
    }
    fn selection_dragging(&self) -> bool {
        self.is_cross_text_dragging()
    }
    fn selection_len(&self) -> usize {
        self.cross_text_len()
    }
    fn selection_anchor(&self) -> usize {
        self.cross_text_anchor()
    }
    fn set_selection_range(&self, range: Option<(usize, usize)>) {
        self.set_cross_text_range(range);
    }
    fn selection_char_at(&self, point: crate::core::Point) -> usize {
        self.cross_text_char_at(point)
    }
    fn selection_text(&self) -> Option<String> {
        self.selected_text()
    }
    fn set_selection_policy(&mut self, policy: crate::ui::UserSelect) {
        self.set_user_select_policy(policy);
    }
}

const LABEL_VISUAL: LabelVisual = LabelVisual {
    metrics: LabelMetricsVisual {
        default_font_size: 14.0,
        default_line_height_factor: 1.5,
        single_line_height_factor: 1.5,
        measurement_dpi: 96.0,
        selection_alpha: 64,
        layout_max_height: 0.0,
        word_wrap: false,
        vertical_align: VAlign::Top,
    },
    palette: LabelPaletteVisual {
        text: label_text_color(),
        primary: label_primary_color(),
    },
};
const LABEL_VISUAL_REF: &LabelVisual = &LABEL_VISUAL;
