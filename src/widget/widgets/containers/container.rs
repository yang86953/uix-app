//! Container widget — flexbox layout container with background/border.

use std::cell::Cell;

use crate::api::widget::layout::LayoutEngine;
use crate::define_widget;
use crate::widget::layout::engine::{child_from_tree, BoxModel, FlexLayout, LayoutChild};
use crate::widget::scene::RenderContext;
use crate::widget::style::{BoxShadowDef, DisplayMode, Style};
use crate::widget::{WidgetCore, WidgetId, WidgetTree};
use crate::api::widget::{AlignItems, FlexDirection, JustifyContent};
use crate::render::Color;
use crate::platform::{EdgeInsets, Rect, Size};

define_widget! {
    /// Container — flexbox 布局容器，带背景/边框/圆角/阴影。
    ///
    /// 所有视觉效果统一通过 `style: Style` 配置。布局引擎从 `style.margin`
    /// 读取外边距参与盒模型计算。方向默认 Column（垂直堆叠）。
    ///
    /// 盒模型（与 Web CSS 一致）：
    /// - margin：外边距，布局时占用空间，推开兄弟节点
    /// - padding：内边距，子内容在其内部排列
    /// - border + border_radius：边框与圆角
    /// - background：背景色（支持 hover/active 状态色）
    /// - box_shadow：盒阴影/辉光
    pub struct Container {
        /// 统一样式（所有视觉属性的唯一来源）
        pub style: Style,
        /// 缓存子节点内容尺寸（layout_children 后更新），
        /// 使 preferred_size 在无固定尺寸时能基于子节点内容估算宽度。
        cached_content_size: Cell<Size>,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::render::traits::GraphicsEngine>) -> Size {
        let mh = self.style.margin.horizontal();
        let mv = self.style.margin.vertical();
        let bh = self.style.border_width * 2.0;
        let bv = self.style.border_width * 2.0;
        let cached = self.cached_content_size.get();
        let effective_w = self.style.width
            .unwrap_or_else(|| if cached.w > 0.0 { cached.w + self.style.padding.horizontal() } else { 0.0 });
        Size::new(
            effective_w + mh + bh,
            self.style.height.map(|h| h + mv + bv).unwrap_or(0.0),
        )
    }

    flex_grow => (&self) -> f32 { self.style.flex_grow }

    flex_shrink => (&self) -> f32 { self.style.flex_shrink }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        // Visual area excludes margin (margin is transparent per CSS box model)
        let s = &self.style;
        let visual = Rect::new(
            frame.x + s.margin.left,
            frame.y + s.margin.top,
            (frame.w - s.margin.horizontal()).max(0.0),
            (frame.h - s.margin.vertical()).max(0.0),
        );
        if visual.w <= 0.0 || visual.h <= 0.0 { return; }

        // 统一样式——绘制背景/边框/阴影/透明度
        crate::widget::scene::apply_style(ctx, visual, s);
    }

    layout_children => (&self, frame: Rect, children: &[WidgetId], tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        if children.is_empty() { return Vec::new(); }

        let s = &self.style;

        // 统一的盒模型计算
        let box_model = BoxModel {
            margin: s.margin,
            border_width: s.border_width,
            padding: s.padding,
        };
        let content_rect = box_model.content_rect(frame);
        if content_rect.w <= 0.0 || content_rect.h <= 0.0 { return Vec::new(); }

        // 过滤不可见子节点
        let visible_children: Vec<WidgetId> = children.iter().copied()
            .filter(|&cid| {
                let visible = tree.get(cid)
                    .map(|n| n.visible())
                    .unwrap_or(true);
                if !visible {
                    crate::platform::log::debug_fn(format!("[Container::layout_children] child {} is invisible, skipping", cid));
                }
                visible
            })
            .collect();
        if visible_children.is_empty() { return Vec::new(); }

        // 构建统一子节点信息
        let layout_children: Vec<LayoutChild> = visible_children
            .iter()
            .map(|&cid| child_from_tree(cid, tree))
            .collect();

        // 委托给统一的 FlexLayout 布局引擎
        let engine = FlexLayout {
            direction: convert_flex_direction(s.flex_direction),
            gap: s.gap,
            justify: convert_justify(s.justify_content),
            align: convert_align(s.align_items),
            wrap: s.flex_wrap,
            overflow_content: s.overflow_content,
        };
        let output = engine.layout(content_rect, &layout_children);

        // 缓存子节点内容尺寸
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

/// 将 style::FlexDirection 转换为 layout::FlexDirection
fn convert_flex_direction(d: crate::widget::style::FlexDirection) -> FlexDirection {
    match d {
        crate::widget::style::FlexDirection::Row => FlexDirection::Row,
        crate::widget::style::FlexDirection::Column => FlexDirection::Column,
        crate::widget::style::FlexDirection::RowReverse => FlexDirection::RowReverse,
        crate::widget::style::FlexDirection::ColumnReverse => FlexDirection::ColumnReverse,
    }
}

/// 将 style::JustifyContent 转换为 layout::JustifyContent
fn convert_justify(j: crate::widget::style::JustifyContent) -> JustifyContent {
    match j {
        crate::widget::style::JustifyContent::Start => JustifyContent::Start,
        crate::widget::style::JustifyContent::Center => JustifyContent::Center,
        crate::widget::style::JustifyContent::End => JustifyContent::End,
        crate::widget::style::JustifyContent::SpaceBetween => JustifyContent::SpaceBetween,
        crate::widget::style::JustifyContent::SpaceAround => JustifyContent::SpaceAround,
        crate::widget::style::JustifyContent::SpaceEvenly => JustifyContent::SpaceEvenly,
        crate::widget::style::JustifyContent::Stretch => JustifyContent::Stretch,
    }
}

/// 将 style::AlignItems 转换为 layout::AlignItems
fn convert_align(a: crate::widget::style::AlignItems) -> AlignItems {
    match a {
        crate::widget::style::AlignItems::Start => AlignItems::Start,
        crate::widget::style::AlignItems::Center => AlignItems::Center,
        crate::widget::style::AlignItems::End => AlignItems::End,
        crate::widget::style::AlignItems::Stretch => AlignItems::Stretch,
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
            style: Style::container(),
            cached_content_size: Cell::new(Size::zero()),
        }
    }

    // ═══════════════════════════════════════════════════
    // 统一样式设置
    // ═══════════════════════════════════════════════════

    /// 批量设置 Style（替换所有现有值）。
    pub fn style(mut self, s: Style) -> Self {
        self.style = s;
        self
    }

    /// 应用另一个 Style（非零/非默认值覆盖当前值）。
    pub fn apply_style(mut self, s: Style) -> Self {
        self.style = self.style.apply(s);
        self
    }

    // ═══════════════════════════════════════════════════
    // CSS 风格链式方法
    // ═══════════════════════════════════════════════════

    /// 设置背景色（`bg` 别名）。
    pub fn bg(mut self, c: Color) -> Self {
        self.style.background = Some(c);
        self
    }

    /// 设置外边距。
    pub fn margin(mut self, m: EdgeInsets) -> Self {
        self.style.margin = m;
        self
    }

    /// 设置内边距（`p` 别名）。
    pub fn padding(mut self, p: EdgeInsets) -> Self {
        self.style.padding = p;
        self
    }

    /// 设置内边距（简写）。
    pub fn p(mut self, p: EdgeInsets) -> Self {
        self.style.padding = p;
        self
    }

    /// 设置边框。
    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.style.border_color = Some(color);
        self.style.border_width = width;
        self
    }

    /// 设置圆角。
    pub fn rounded(mut self, r: f32) -> Self {
        self.style.border_radius = r;
        self
    }

    /// 设置固定宽度。
    pub fn w(mut self, v: f32) -> Self {
        self.style.width = Some(v);
        self
    }

    /// 设置固定高度。
    pub fn h(mut self, v: f32) -> Self {
        self.style.height = Some(v);
        self
    }

    /// 同时设置宽高。
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.style.width = Some(w);
        self.style.height = Some(h);
        self
    }

    /// 设置文字颜色。
    pub fn color(mut self, c: Color) -> Self {
        self.style.color = c;
        self
    }

    /// 设置字号。
    pub fn fs(mut self, s: f32) -> Self {
        self.style.font_size = s;
        self
    }

    /// 设置显示模式（Flex / None）。
    pub fn display(mut self, d: DisplayMode) -> Self {
        self.style.display = d;
        self
    }

    /// 设置 flex 方向。
    pub fn direction(mut self, d: crate::widget::style::FlexDirection) -> Self {
        self.style.flex_direction = d;
        self
    }

    /// 设置子项间距。
    pub fn gap(mut self, g: f32) -> Self {
        self.style.gap = g;
        self
    }

    /// 设置主轴对齐。
    pub fn justify(mut self, j: crate::widget::style::JustifyContent) -> Self {
        self.style.justify_content = j;
        self
    }

    /// 设置交叉轴对齐。
    pub fn align(mut self, a: crate::widget::style::AlignItems) -> Self {
        self.style.align_items = a;
        self
    }

    /// 设置 flex-grow。
    pub fn flex_grow(mut self, v: f32) -> Self {
        self.style.flex_grow = v;
        self
    }

    /// 设置 flex-shrink。
    pub fn flex_shrink(mut self, v: f32) -> Self {
        self.style.flex_shrink = v;
        self
    }

    /// 设置透明度。
    pub fn opacity(mut self, o: f32) -> Self {
        self.style.opacity = o;
        self
    }

    /// 设置盒阴影/辉光。
    pub fn shadow(mut self, s: BoxShadowDef) -> Self {
        self.style.box_shadow = Some(s);
        self
    }

    /// 设置可见性。
    pub fn visible(mut self, v: bool) -> Self {
        self.style.visible = v;
        self
    }

    // ═══════════════════════════════════════════════════
    // 兼容旧 API（委托到 style）
    // ═══════════════════════════════════════════════════

    /// 便捷方法：单独设置内边距（旧 API 兼容）。
    pub fn pad(mut self, p: EdgeInsets) -> Self {
        self.style.padding = p;
        self
    }

    /// 便捷方法：设置 flex 方向（旧 API 兼容）。
    pub fn dir(mut self, d: FlexDirection) -> Self {
        self.style.flex_direction = match d {
            FlexDirection::Row => crate::widget::style::FlexDirection::Row,
            FlexDirection::Column => crate::widget::style::FlexDirection::Column,
            FlexDirection::RowReverse => crate::widget::style::FlexDirection::RowReverse,
            FlexDirection::ColumnReverse => crate::widget::style::FlexDirection::ColumnReverse,
        };
        self
    }

    /// 允许内容溢出（跳过 flex-shrink，用于可滚动容器）。
    pub fn overflow_content(mut self) -> Self {
        self.style.overflow_content = true;
        self
    }
}
