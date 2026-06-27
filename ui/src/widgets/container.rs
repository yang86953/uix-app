//! Container widget — flexbox layout container with background/border.

use std::cell::Cell;

use crate::define_widget;
use uix_graphics::{Color, Radius};
use crate::{AlignItems, FlexDirection, JustifyContent};
use crate::layout::engine::{
    BoxModel, FlexLayout, LayoutChild, LayoutEngine, child_from_tree,
};
use crate::style::Style;
use uix_platform::{EdgeInsets, Rect, Size};
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

        // 统一的盒模型计算（使用底层 BoxModel）
        let box_model = BoxModel {
            margin: self.margin,
            border_width: self.border_width,
            padding: self.padding,
        };
        let content_rect = box_model.content_rect(frame);
        if content_rect.w <= 0.0 || content_rect.h <= 0.0 { return Vec::new(); }

        // 过滤不可见子节点：不可见的 widget 不参与布局，不占空间
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
        if visible_children.is_empty() { return Vec::new(); }

        // 构建统一子节点信息（使用底层 child_from_tree）
        let layout_children: Vec<LayoutChild> = visible_children
            .iter()
            .map(|&cid| child_from_tree(cid, tree))
            .collect();

        // 委托给统一的 FlexLayout 布局引擎
        let engine = FlexLayout {
            direction: self.direction,
            gap: self.gap,
            justify: self.justify,
            align: self.align,
            wrap: false,
            overflow_content: self.overflow_content,
        };
        let output = engine.layout(content_rect, &layout_children);

        // 缓存子节点内容尺寸，供 preferred_size 在无 fixed_width 时使用
        self.cached_content_size.set(Size::new(
            output.total_size.w.max(0.0),
            output.total_size.h.max(0.0),
        ));

        visible_children
            .iter()
            .zip(output.positions)
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

}

