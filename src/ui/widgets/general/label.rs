//! Label widget — displays text with optional selection support.
//!
//! 默认不可选中：导航/标题等 UI 文案不应出现拖选高亮。
//! 需要复制选区时调用 `.selectable()`。

use std::cell::Cell;
use std::cell::RefCell;

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::geometry::spatial::PhysicalUnit;
use crate::draw::TextLayoutOptions;
use crate::ui::component::clipboard;
use crate::ui::component::paint_context::PaintContext;
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

#[derive(Debug, Clone, Copy)]
struct TextLineHit {
    y: f32,
    glyph_start: usize,
    glyph_count: usize,
    start_char: usize,
    end_char: usize,
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
        /// 渲染时缓存的字形 x 位置（文本局部坐标）。
        glyph_xs: RefCell<Vec<f32>>,
        /// 与 glyph_xs 平行的字形 advance，用于按中点选择最近光标边界。
        glyph_widths: RefCell<Vec<f32>>,
        /// 与 glyph_xs 平行的字符下标（`chars()` 序）。
        glyph_char_indices: RefCell<Vec<usize>>,
        /// 每行的字符范围与字形范围，用于二维命中测试。
        line_info: RefCell<Vec<TextLineHit>>,
        selection: Cell<Option<(usize, usize)>>,
        sel_anchor: Cell<usize>,
        sel_dragging: Cell<bool>,
        /// 上次渲染时的文本 draw_pos（用于事件命中测试）。
        draw_pos: Cell<crate::core::Point>,
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
        if !self.selectable {
            return EventResult::NotHandled;
        }
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                mods,
            } => {
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
            SystemEvent::KeyDown { key, mods } => {
                let ctrl = mods.contains(KeyMod::CTRL);
                match key {
                    KeyCode::A if ctrl => {
                        let len = self.text.chars().count();
                        self.sel_anchor.set(0);
                        self.set_selection_range(0, len);
                        EventResult::Handled
                    }
                    KeyCode::C if ctrl => {
                        if let Some((s, e)) = self.selection.get() {
                            let selected = self.slice_range(s, e);
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
        self.draw_pos.set(draw_pos);
        let abs_pos = crate::core::Point::new(frame.x + draw_pos.x, frame.y + draw_pos.y);

        if !self.text.is_empty() {
            // 缓存字形 x 与对应字符下标（选区/命中用字符序）
            {
                let mut xs = self.glyph_xs.borrow_mut();
                let mut widths = self.glyph_widths.borrow_mut();
                let mut cis = self.glyph_char_indices.borrow_mut();
                xs.clear();
                widths.clear();
                cis.clear();
                for g in &layout.glyphs {
                    xs.push(g.x);
                    widths.push(g.width.max(0.0));
                    cis.push(g.char_index);
                }
            }

            // 缓存行信息（用于 y 轴命中测试）
            {
                let mut li = self.line_info.borrow_mut();
                li.clear();
                for line in &layout.lines {
                    li.push(TextLineHit {
                        y: line.y,
                        glyph_start: line.glyph_start,
                        glyph_count: line.glyph_count,
                        start_char: line.start_char,
                        end_char: line.end_char,
                    });
                }
            }

            // 绘制选中背景（按字符下标匹配字形）
            if let Some((sel_s, sel_e)) = self.selection.get() {
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
            glyph_xs: RefCell::new(Vec::new()),
            glyph_widths: RefCell::new(Vec::new()),
            glyph_char_indices: RefCell::new(Vec::new()),
            line_info: RefCell::new(Vec::new()),
            selection: Cell::new(None),
            sel_anchor: Cell::new(0),
            sel_dragging: Cell::new(false),
            draw_pos: Cell::new(crate::core::Point::new(0.0, 0.0)),
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
        self.glyph_xs.borrow_mut().clear();
        self.glyph_widths.borrow_mut().clear();
        self.glyph_char_indices.borrow_mut().clear();
        self.line_info.borrow_mut().clear();
        self.selection.set(None);
        self.sel_anchor.set(0);
        self.sel_dragging.set(false);
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
        if !self.selectable {
            self.selection.set(None);
            self.sel_dragging.set(false);
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
        self.selection.get().map(|(s, e)| self.slice_range(s, e))
    }

    pub(crate) fn participates_in_cross_text_selection(&self) -> bool {
        self.selectable
    }

    pub(crate) fn is_cross_text_dragging(&self) -> bool {
        self.selectable && self.sel_dragging.get()
    }

    pub(crate) fn cross_text_len(&self) -> usize {
        self.text.chars().count()
    }

    pub(crate) fn cross_text_anchor(&self) -> usize {
        self.sel_anchor.get()
    }

    pub(crate) fn set_cross_text_range(&self, range: Option<(usize, usize)>) {
        if !self.selectable {
            return;
        }
        match range {
            Some((a, b)) if a != b => self.selection.set(Some((a.min(b), a.max(b)))),
            _ => self.selection.set(None),
        }
    }

    pub(crate) fn cross_text_char_at(&self, frame_local: crate::core::Point) -> usize {
        let dp = self.draw_pos.get();
        self.char_at_xy(frame_local.x - dp.x, frame_local.y - dp.y)
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

    fn char_at_xy(&self, text_x: f32, text_y: f32) -> usize {
        let xs = self.glyph_xs.borrow();
        let widths = self.glyph_widths.borrow();
        let cis = self.glyph_char_indices.borrow();
        let li = self.line_info.borrow();
        if xs.is_empty() {
            return 0;
        }
        if li.is_empty() {
            for i in 0..xs.len() {
                let boundary = xs[i] + widths.get(i).copied().unwrap_or_default() * 0.5;
                if text_x < boundary {
                    return cis.get(i).copied().unwrap_or(i);
                }
            }
            return cis
                .last()
                .map(|c| c + 1)
                .unwrap_or(self.text.chars().count());
        }
        let target_y = text_y.max(li[0].y);
        let Some(line) = li
            .iter()
            .enumerate()
            .find(|(index, line)| {
                let next_y = li.get(index + 1).map_or(f32::MAX, |next| next.y);
                target_y >= line.y && target_y < next_y
            })
            .map(|(_, line)| line)
            .or_else(|| li.last())
        else {
            return 0;
        };
        let start = line.glyph_start.min(xs.len());
        let end = (start + line.glyph_count).min(xs.len());
        for i in start..end {
            let boundary = xs[i] + widths.get(i).copied().unwrap_or_default() * 0.5;
            if text_x < boundary {
                return cis.get(i).copied().unwrap_or(i);
            }
        }
        if end > start {
            line.end_char
        } else {
            line.start_char
        }
    }

    pub(crate) fn set_selection_range(&self, a: usize, b: usize) {
        if a == b {
            self.selection.set(None);
        } else {
            self.selection.set(Some((a.min(b), a.max(b))));
        }
    }

    fn slice_range(&self, start_char: usize, end_char: usize) -> String {
        let chars: Vec<char> = self.text.chars().collect();
        let e = end_char.min(chars.len());
        let s = start_char.min(e);
        chars[s..e].iter().collect()
    }
}
