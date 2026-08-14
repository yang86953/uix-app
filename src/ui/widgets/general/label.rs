//! Label widget — displays text with optional selection support.
//!
//! 默认不可选中：导航/标题等 UI 文案不应出现拖选高亮。
//! 需要复制选区时调用 `.selectable()`。

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::TextLayoutOptions;
use crate::draw::geometry::spatial::PhysicalUnit;
use crate::ui::component::clipboard;
use crate::ui::component::paint_context::PaintContext;
// 引入共享的单节点文字选区实现。
use crate::ui::text_selection::per_node::PerNodeTextSelection;
use crate::ui::theme::style::Style;
use crate::ui::{EventResult, KeyCode, KeyMod, MouseButton, SystemEvent, WidgetTree};
use crate::ui::{SnapshotFields, SnapshotSource};

const DEFAULT_LABEL_FONT_SIZE: f32 = 12.0;

fn normalized_label_font_size(size: f32) -> f32 {
    if size.is_finite() && size > 0.0 {
        size
    } else {
        DEFAULT_LABEL_FONT_SIZE
    }
}

component! {
    pub struct Label {
        pub text: String,
        pub font_size: f32,
        /// 物理单位字号（可选，优先级高于 font_size）。
        pub font_size_unit: Option<PhysicalUnit>,
        pub color: Option<crate::draw::Color>,
        pub fixed_width: Option<f32>,
        pub fixed_height: Option<f32>,
        /// 是否允许拖选 / Ctrl+A / Ctrl+C 选区。默认 false。
        selectable: bool,
        /// 共享的选区状态：布局缓存、选区、拖选锚点与绘制偏移。
        sel: PerNodeTextSelection,
        /// 统一样式覆盖（优先于 color/font_size 独立字段）。
        pub(crate) style: Option<Style>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
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
        if !self.selectable {
            return EventResult::NotHandled;
        }
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                mods,
            } => {
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
        // 统一样式优先：背景/边框（section 色条等）再绘制文字
        if let Some(style) = self.style.as_ref() {
            crate::ui::theme::style::apply_style(ctx, frame, style);
        }

        // 统一样式优先：style.color > self.color > theme default
        let c = if let Some(style) = self.style.as_ref() {
            style.resolve_color(ctx.tokens())
        } else {
            self.color.unwrap_or_else(|| ctx.tokens().color_text())
        };
        // 分辨率：物理单位优先 > style > self.font_size
        let fs = if let Some(unit) = self.font_size_unit {
            unit.to_dip(ctx.dpi())
        } else if let Some(style) = self.style.as_ref() {
            style.resolve_font_size(ctx.tokens())
        } else {
            self.font_size
        };
        let fs = normalized_label_font_size(fs);

        // 单次布局：同时用于 hit-test 缓存、选中背景和文字绘制
        let opts = TextLayoutOptions {
            max_width: f32::MAX,
            max_height: 0.0,
            line_height: fs * 1.5,
            word_wrap: false,
            h_align: crate::draw::HAlign::Left,
            v_align: crate::draw::VAlign::Top,
            font_size: fs,
        };
        let backend_opts = crate::draw::resources::font::text_backend::TextLayoutOptions::from(opts);
        let fh = *ctx.font();
        let layout = ctx.font_service().layout_text(&fh, &self.text, &backend_opts);

        // 内边距 + 顶对齐绘制。过高 frame 时的垂直居中由父级 AlignItems 负责，
        // 不在此用 visual_center_y 二次修正。
        let pad = self
            .style
            .as_ref()
            .map(|s| s.padding)
            .unwrap_or_default();
        let draw_pos = crate::core::Point::new(pad.left, pad.top);
        self.sel.set_draw_pos(draw_pos);
        let abs_pos = crate::core::Point::new(frame.x + draw_pos.x, frame.y + draw_pos.y);

        if !self.text.is_empty() {
            // 缓存字形 x / advance / 字符下标与行信息（选区与命中测试用）。
            self.sel.cache_layout(&layout);

            // 绘制选中背景（按字符下标匹配字形）
            if let Some((sel_s, sel_e)) = self.sel.selection() {
                if sel_s < sel_e {
                    let visual_h = ctx.font_service()
                        .horizontal_line_metrics(&fh, fs)
                        .map(|m| m.ascent + m.descent)
                        .unwrap_or(fs * 1.2);
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
                                ctx.tokens().color_primary().with_alpha(64),
                                None,
                            );
                        }
                    }
                }
            }

            // 绘制文本（使用同一布局）
            ctx.blit_owned_glyph_layout(layout, abs_pos, c, fs);
        }
    }
}

impl SnapshotSource for Label {
    fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Label {
            text: self.text.clone(),
            font_size: self.font_size,
            font_size_unit: self.font_size_unit,
            color: self.color,
            fixed_width: self.fixed_width,
            fixed_height: self.fixed_height,
            style: self.style.clone(),
        }
    }
}

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        let t = text.into();
        Self {
            text: t,
            font_size: DEFAULT_LABEL_FONT_SIZE,
            font_size_unit: None,
            color: None,
            fixed_width: None,
            fixed_height: None,
            selectable: false,
            sel: PerNodeTextSelection::new(),
            style: None,
        }
    }

    /// 允许拖选与快捷键复制选区（导航/标题等默认关闭）。
    pub fn selectable(mut self) -> Self {
        self.selectable = true;
        self
    }

    /// 设置统一样式（覆盖文字颜色/字号等视觉属性）。
    pub fn style(mut self, s: Style) -> Self {
        self.style = Some(s);
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

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
        self.font_size_unit = next.font_size_unit;
        self.color = next.color;
        self.fixed_width = next.fixed_width;
        self.fixed_height = next.fixed_height;
        self.selectable = next.selectable;
        // 关闭可选中时同步清除残留选区。
        if !self.selectable {
            self.sel.reset_selection();
        }
        self.style = next.style;
    }

    pub fn color(mut self, c: crate::draw::Color) -> Self {
        self.color = Some(c);
        self
    }

    pub fn font_size(mut self, s: f32) -> Self {
        self.font_size = normalized_label_font_size(s);
        self.font_size_unit = None;
        self
    }

    /// 设置物理单位字号（优先级高于 `font_size()`）。
    /// 自动适配 DPI：`12.pt()` 在不同屏幕上物理尺寸一致。
    pub fn font_size_unit(mut self, unit: PhysicalUnit) -> Self {
        self.font_size_unit = Some(unit);
        self
    }

    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.fixed_width = Some(w);
        self.fixed_height = Some(h);
        self
    }

    pub fn selected_text(&self) -> Option<String> {
        self.sel.selected_text(&self.text)
    }

    pub(crate) fn participates_in_cross_text_selection(&self) -> bool {
        // 仅显式可选的 Label 参与跨节点拖选。
        self.selectable
    }

    pub(crate) fn is_cross_text_dragging(&self) -> bool {
        // 拖选状态同样以可选中为前提。
        self.selectable && self.sel.is_dragging()
    }

    pub(crate) fn cross_text_len(&self) -> usize {
        self.sel.cross_text_len(&self.text)
    }

    pub(crate) fn cross_text_anchor(&self) -> usize {
        self.sel.cross_text_anchor()
    }

    pub(crate) fn set_cross_text_range(&self, range: Option<(usize, usize)>) {
        // 不可选中的 Label 不接收跨节点选区。
        if !self.selectable {
            return;
        }
        self.sel.set_cross_text_range(&self.text, range);
    }

    pub(crate) fn cross_text_char_at(&self, frame_local: crate::core::Point) -> usize {
        self.sel.cross_text_char_at(&self.text, frame_local)
    }

    fn intrinsic_size(&self) -> Size {
        let style_w = self.style.as_ref().and_then(|s| s.width);
        let style_h = self.style.as_ref().and_then(|s| s.height);
        let pad = self.style.as_ref().map(|s| s.padding).unwrap_or_default();
        let w = self.fixed_width.or(style_w);
        let h = self.fixed_height.or(style_h);
        if let (Some(w), Some(h)) = (w, h) {
            Size::new(w, h)
        } else {
            let raw_font_size = self
                .font_size_unit
                .map(|unit| unit.to_dip(96.0))
                .or_else(|| self.style.as_ref().map(|s| s.font_size.default_size()))
                .unwrap_or(self.font_size);
            let fs = normalized_label_font_size(raw_font_size);
            let estimated = crate::draw::resources::font::text_backend::estimate_text_metrics(
                &self.text,
                f32::INFINITY,
                fs,
            );
            // 单行固有高度 = 行盒（≈ ascent+descent）；与顶对齐绘制一致。
            // 与 Icon 同行时由父级 AlignItems::Center 对齐，勿在 paint 里二次居中。
            // 显式多行保留完整 line box，禁止后继节点压到实际字形上。
            let text_height = if estimated.line_count == 1 {
                fs * 1.2
            } else {
                fs * 1.5 * estimated.line_count as f32
            };
            Size::new(
                w.unwrap_or(estimated.max_line_width + pad.horizontal()),
                h.unwrap_or(text_height + pad.vertical()),
            )
        }
    }
}
