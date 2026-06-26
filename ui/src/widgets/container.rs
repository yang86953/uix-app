//! Container widget — flexbox layout container with background/border.

use std::cell::Cell;

use crate::define_widget;
use uix_graphics::{Color, Radius};
use crate::{
    compute_flex_layout, AlignItems, FlexChild, FlexDirection, FlexInput, JustifyContent,
};
use crate::style::Style;
use uix_core::{EdgeInsets, Rect, Size};
use crate::render_context::RenderContext;
use crate::widget::{WidgetCore, WidgetId, WidgetTree};

define_widget! {
    /// Container — flexbox 布局容器，带背景/边框/圆角。
    ///
    /// 盒模型（与 Web CSS 一致）：
    /// - margin：外边距，布局时占用空间
    /// - border：边框，影响布局尺寸
    /// - padding：内边距，子内容在其内部排列
    /// - 默认方向为 Column（垂直堆叠，类似 Web block 流式布局）
    pub struct Container {
        pub bg_color: Option<Color>,
        pub border_color: Option<Color>,
        pub border_width: f32,
        pub border_radius: f32,
        pub padding: EdgeInsets,
        pub margin: EdgeInsets,
        pub gap: f32,
        pub direction: FlexDirection,
        pub justify: JustifyContent,
        pub align: AlignItems,
        pub fixed_width: Option<f32>,
        pub fixed_height: Option<f32>,
        pub flex_grow: f32,
        pub flex_shrink: f32,
        /// 允许内容溢出容器主轴方向（跳过 flex-shrink，总尺寸反映实际内容）
        pub overflow_content: bool,
        /// 盒阴影 / 辉光（霓虹科幻效果）
        pub box_shadow_color: Option<Color>,
        /// 模糊半径（越大越散）
        pub box_shadow_blur: f32,
        /// 水平偏移
        pub box_shadow_offset_x: f32,
        /// 垂直偏移
        pub box_shadow_offset_y: f32,
        /// 统一样式覆盖（优先于 bg_color/border_color/box_shadow 等独立字段）
        pub style: Option<Style>,
        /// 缓存子节点内容尺寸（layout_children 后更新），
        /// 使 preferred_size 在无 fixed_width 时能基于子节点内容估算宽度。
        /// 使用 Cell 实现内部可变性，preferred_size(&self) 可直接读取。
        cached_content_size: Cell<Size>,
    }

    // preferred_size 包含 margin + border（Web 盒模型中 margin/border 占用空间）。
    // 当 fixed_width 为 None 时，使用 layout_children 缓存的子节点内容宽度，
    // 使父容器 flex 布局能基于实际内容分配空间（修复 Container 包裹单子时的宽度错误）。
    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        let mh = self.margin.horizontal();
        let mv = self.margin.vertical();
        let bh = self.border_width * 2.0;
        let bv = self.border_width * 2.0;
        let cached = self.cached_content_size.get();
        let effective_w = self.fixed_width
            .unwrap_or_else(|| if cached.w > 0.0 { cached.w + self.padding.horizontal() } else { 0.0 });
        Size::new(
            effective_w + mh + bh,
            self.fixed_height.map(|h| h + mv + bv).unwrap_or(0.0),
        )
    }

    flex_grow => (&self) -> f32 { self.flex_grow }

    flex_shrink => (&self) -> f32 { self.flex_shrink }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        // Visual area excludes margin (margin is transparent per CSS box model)
        let visual = Rect::new(
            frame.x + self.margin.left,
            frame.y + self.margin.top,
            (frame.w - self.margin.horizontal()).max(0.0),
            (frame.h - self.margin.vertical()).max(0.0),
        );
        if visual.w <= 0.0 || visual.h <= 0.0 { return; }

        // 统一样式优先（使用 ctx.apply_style 统一渲染背景/边框/阴影）
        if let Some(ref s) = self.style {
            ctx.apply_style(visual, s);
        } else {
            // 回退：独立字段渲染（保持向后兼容）
            if let Some(sc) = self.box_shadow_color {
                let r = if self.border_radius > 0.0 { Some(Radius::uniform(self.border_radius)) } else { None };
                ctx.draw_box_shadow(visual, self.box_shadow_blur, self.box_shadow_offset_x, self.box_shadow_offset_y, sc, r);
            }
            if let Some(c) = self.bg_color {
                let r = if self.border_radius > 0.0 { Some(Radius::uniform(self.border_radius)) } else { None };
                ctx.fill_rect(visual, c, r);
            }
            if let Some(c) = self.border_color {
                let r = if self.border_radius > 0.0 { Some(Radius::uniform(self.border_radius)) } else { None };
                ctx.stroke_rect(visual, c, self.border_width, r);
            }
        }
    }
    layout_children => (&self, frame: Rect, children: &[WidgetId], tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        if children.is_empty() { return Vec::new(); }

        // 内容区域 = frame - margin - border - padding（Web 盒模型）
        let mh = self.margin.horizontal();
        let mv = self.margin.vertical();
        let bh = self.border_width * 2.0;
        let bv = self.border_width * 2.0;
        let inner = Rect::new(
            frame.x + self.margin.left + self.border_width,
            frame.y + self.margin.top + self.border_width,
            (frame.w - mh - bh - self.padding.horizontal()).max(0.0),
            (frame.h - mv - bv - self.padding.vertical()).max(0.0),
        );

        // 传递给 flex 布局的内容容器（不含 border/padding）
        let flex_container = Rect::new(
            inner.x + self.padding.left,
            inner.y + self.padding.top,
            inner.w,
            inner.h,
        );

        // 过滤不可见子节点：不可见的 widget 不参与布局，不占空间
        // 注意：tree.get(cid) 返回 None 时（节点尚未完全就绪），
        // 使用 unwrap_or(true) 假设可见（安全侧），避免静默过滤掉正在构建中的子节点。
        let visible_children: Vec<WidgetId> = children.iter().copied()
            .filter(|&cid| {
                let visible = tree.get(cid)
                    .map(|n| n.visible())
                    .unwrap_or(true);
                if !visible {
                    log::debug!("[Container::layout_children] child {} is invisible, skipping", cid);
                }
                visible
            })
            .collect();
        if visible_children.is_empty() {
            return Vec::new();
        }

        // overflow_content 模式：使用简单流式堆叠，不压缩，不使用 flex 分配
        // 类似 Web CSS block 流式布局 — 子节点按自然尺寸依次排列
        if self.overflow_content {
            return self.layout_overflow(&flex_container, &visible_children, tree);
        }

        // 合并为单次遍历：同时构建 child_sizes 和 flex_children
        let (child_sizes, flex_children): (Vec<Size>, Vec<FlexChild>) = visible_children
            .iter()
            .map(|&cid| {
                let node = tree.get(cid);
                let pref = node.map(|c| c.preferred_size(None)).unwrap_or_default();
                let h = if pref.h > 0.0 { pref.h } else { node.map(|c| c.frame().h).unwrap_or(0.0) };
                let size = Size::new(pref.w, h);
                let flex = FlexChild {
                    flex_grow: node.map(|c| c.inner().flex_grow()).unwrap_or(0.0),
                    flex_shrink: node.map(|c| c.inner().flex_shrink()).unwrap_or(1.0),
                    ..FlexChild::default()
                };
                (size, flex)
            })
            .unzip();

        let input = FlexInput {
            direction: self.direction,
            gap: self.gap,
            padding: EdgeInsets::zero(),  // padding 已算入 flex_container
            container: flex_container,
            children: flex_children,
            child_sizes,
            justify_content: self.justify,
            align_items: self.align,
            ..FlexInput::default()
        };

        let output = compute_flex_layout(&input);

        // 缓存子节点内容尺寸（不含 padding/margin/border），
        // 供 preferred_size 在无 fixed_width 时使用。
        let content_w = output.total_size.w.max(0.0);
        let content_h = output.total_size.h.max(0.0);
        self.cached_content_size.set(Size::new(content_w, content_h));

        visible_children
            .iter()
            .zip(output.child_rects)
            .map(|(&cid, rect)| (cid, rect))
            .collect()
    }
}

impl Default for Container {
    fn default() -> Self {
        Self::new()
    }
}

impl Container {
    pub fn new() -> Self {
        Self {
            bg_color: None,
            border_color: None,
            border_width: 0.0,
            border_radius: 0.0,
            padding: EdgeInsets::zero(),
            margin: EdgeInsets::zero(),
            gap: 0.0,
            direction: FlexDirection::Column,
            justify: JustifyContent::Start,
            align: AlignItems::Stretch,
            fixed_width: None,
            fixed_height: None,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            overflow_content: false,
            box_shadow_color: None,
            box_shadow_blur: 0.0,
            box_shadow_offset_x: 0.0,
            box_shadow_offset_y: 0.0,
            style: None,
            cached_content_size: Cell::new(Size::zero()),
        }
    }

    /// 设置统一样式（覆盖背景/边框/阴影/文字颜色等所有视觉属性）。
    /// 设置后，bg/border/box_shadow 等独立字段不再生效。
    pub fn style(mut self, s: Style) -> Self {
        self.style = Some(s);
        self
    }

    pub fn bg(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }
    /// 设置外边距（Web 盒模型，布局时占用空间）。
    pub fn margin(mut self, m: EdgeInsets) -> Self {
        self.margin = m;
        self
    }
    pub fn border(mut self, c: Color, w: f32) -> Self {
        self.border_color = Some(c);
        self.border_width = w;
        self
    }
    pub fn rounded(mut self, r: f32) -> Self {
        self.border_radius = r;
        self
    }
    pub fn pad(mut self, p: EdgeInsets) -> Self {
        self.padding = p;
        self
    }
    pub fn gap(mut self, g: f32) -> Self {
        self.gap = g;
        self
    }
    pub fn dir(mut self, d: FlexDirection) -> Self {
        self.direction = d;
        self
    }
    pub fn align(mut self, a: AlignItems) -> Self {
        self.align = a;
        self
    }
    pub fn justify(mut self, j: JustifyContent) -> Self {
        self.justify = j;
        self
    }
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.fixed_width = Some(w);
        self.fixed_height = Some(h);
        self
    }
    /// 便捷方法：单独设置宽度（用于 `ui!` 宏）。
    pub fn w(mut self, v: f32) -> Self {
        self.fixed_width = Some(v);
        self
    }
    /// 便捷方法：单独设置高度（用于 `ui!` 宏）。
    pub fn h(mut self, v: f32) -> Self {
        self.fixed_height = Some(v);
        self
    }
    pub fn flex_grow(mut self, v: f32) -> Self {
        self.flex_grow = v;
        self
    }
    pub fn flex_shrink(mut self, v: f32) -> Self {
        self.flex_shrink = v;
        self
    }
    /// 允许内容溢出（跳过 flex-shrink，用于可滚动容器）。
    pub fn overflow_content(mut self) -> Self {
        self.overflow_content = true;
        self
    }

    /// 设置盒阴影/辉光效果（用于霓虹科幻视觉风格）。
    pub fn box_shadow(mut self, color: Color, blur: f32) -> Self {
        self.box_shadow_color = Some(color);
        self.box_shadow_blur = blur;
        self
    }

    /// 设置盒阴影偏移量（默认无偏移，配合 box_shadow 使用）。
    pub fn box_shadow_offset(mut self, x: f32, y: f32) -> Self {
        self.box_shadow_offset_x = x;
        self.box_shadow_offset_y = y;
        self
    }

    /// 简单流式堆叠布局（用于 overflow_content 模式）。
    /// 类似 Web CSS block 流式布局，子节点按自然尺寸依次排列，不压缩、
    /// 不分配剩余空间。仅支持 Column/Row 方向 + Start/Center/End/Stretch
    /// 交叉轴对齐。
    fn layout_overflow(
        &self,
        container: &Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
    ) -> Vec<(WidgetId, Rect)> {
        let is_row = matches!(self.direction, FlexDirection::Row | FlexDirection::RowReverse);
        let is_reverse = matches!(self.direction, FlexDirection::RowReverse | FlexDirection::ColumnReverse);
        let count = children.len();
        let mut result = Vec::with_capacity(count);

        // 收集子节点尺寸：取 preferred_size 与当前实际 frame 的最大值，
        // 确保 Phase 2 扩展后的尺寸被正确使用，避免 siblings 重叠。
        let child_heights: Vec<f32> = children
            .iter()
            .map(|&cid| {
                let node = tree.get(cid);
                let pref = node.map(|c| c.preferred_size(None)).unwrap_or_default();
                let current = node.map(|c| c.frame().h).unwrap_or(0.0);
                if pref.h > 0.0 { pref.h.max(current) } else { current }
            })
            .collect();

        let child_widths: Vec<f32> = children
            .iter()
            .map(|&cid| {
                let node = tree.get(cid);
                let pref = node.map(|c| c.preferred_size(None)).unwrap_or_default();
                let current = node.map(|c| c.frame().w).unwrap_or(0.0);
                if pref.w > 0.0 { pref.w.max(current) } else { current.max(container.w) }
            })
            .collect();

        let container_main = if is_row { container.w } else { container.h };
        let container_cross = if is_row { container.h } else { container.w };
        let gap = self.gap;
        let total_gaps = gap * (count as f32 - 1.0).max(0.0);

        // 计算主轴方向子节点尺寸总和
        let main_sizes: Vec<f32> = if is_row {
            child_widths.clone()
        } else {
            child_heights.clone()
        };

        // 流式堆叠：沿主轴依次排列
        let mut cursor = 0.0f32;
        if is_reverse {
            let total_main: f32 = main_sizes.iter().sum::<f32>() + total_gaps;
            cursor = container_main - total_main;
        }

        for i in 0..count {
            let main_size = main_sizes[i];
            let cross_size = if is_row { child_heights[i] } else { child_widths[i] };

            let child_cross_size = if self.align == AlignItems::Stretch {
                container_cross
            } else {
                cross_size
            };

            let cross_offset = match self.align {
                AlignItems::Start => 0.0,
                AlignItems::Center => (container_cross - child_cross_size) / 2.0,
                AlignItems::End => container_cross - child_cross_size,
                AlignItems::Stretch => 0.0,
            };

            let (cx, cy) = if is_row {
                (container.x + cursor, container.y + cross_offset)
            } else {
                (container.x + cross_offset, container.y + cursor)
            };
            let (cw, ch) = if is_row {
                (main_size, child_cross_size)
            } else {
                (child_cross_size, main_size)
            };

            result.push((children[i], Rect::new(cx, cy, cw, ch)));

            if is_reverse {
                cursor -= main_size + gap;
            } else {
                cursor += main_size + gap;
            }
        }

        result
    }
}
