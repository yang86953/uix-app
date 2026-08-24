//! Container widget — flexbox layout container with background/border.

use crate::ui::widget_runtime::widget::WidgetCore;
use std::cell::Cell;

use crate::core::{Constraints, EdgeInsets, Rect, Size};
use crate::draw::scene::PicturePolicy;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::tree_measure::child_from_tree_with_constraints;
use crate::widget;
// 导入共享布局入口与内容外尺寸计算。
use crate::ui::layout::engine::{BoxModel, FlexLayout, LayoutChild, content_size_from_children};

use crate::ui::layout::{AlignItems, FlexDirection, JustifyContent};
use crate::ui::theme::style::{
    BoxShadowDef, ColorValue, DisplayMode, Style, TypographyToken, apply_style,
};
use crate::ui::{SnapshotFields, SnapshotSource};
use crate::ui::{View, ViewNode, WidgetId, WidgetTree};

// 标识 UIX 为 Container 选择的共享主题样式角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContainerStyleRole {
    Default,
}

impl ContainerStyleRole {
    fn resolve(self) -> Style {
        match self {
            Self::Default => Style::container(),
        }
    }
}

// 保存 Container 独有的布局阈值与子项收缩策略。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ContainerLayoutVisual {
    bootstrap_cross_axis_threshold: f32,
    child_flex_shrink: f32,
}

// 全部 Container 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ContainerVisual {
    default_style: ContainerStyleRole,
    layout: ContainerLayoutVisual,
}

crate::uix_items!("src/ui/widgets/containers/container/container.uix");

pub(crate) const fn container_default_style_role() -> ContainerStyleRole {
    ContainerStyleRole::Default
}

widget! {
    /// Container — flexbox 布局容器，带背景/边框/圆角/阴影。
    ///
    /// 所有视觉效果统一通过 `style: Style` 配置。布局引擎从 `style.margin`
    /// 读取外边距参与盒模型计算。方向默认 Column（垂直堆叠）。
    ///
    /// 盒模型（与 Web CSS 一致）：
    /// - margin：外边距，由**父级** flex/grid 占用空间并偏移本节点 frame
    /// - padding：内边距，子内容在其内部排列（`content_rect` 从 frame 扣除）
    /// - border + border_radius：边框与圆角
    /// - frame = border-box（不含 margin）；勿在 content_rect 中再扣 margin
    /// - background：背景色（支持 hover/active 状态色）
    /// - box_shadow：盒阴影/辉光
    pub struct Container {
        /// 统一样式（所有视觉属性的唯一来源）
        pub style: Style,
        /// 缓存子节点内容尺寸（layout_children 后更新），
        /// 使 measure 在无固定 width/height 时能基于子节点内容估算尺寸。
        pub(crate) cached_content_size: Cell<Size>,
        /// 同目录 UIX 生成的唯一静态视觉表。
        pub(crate) visual: &'static ContainerVisual,
    }

    visible => (&self) -> bool { self.style.visible }

    on_children_changed => (&mut self, child_count: usize) {
        // 最后一个子节点移除后，旧内容尺寸不再是有效的测量下限。
        if child_count == 0 {
            // 立即归零，避免下一轮布局跳过空节点时继续暴露陈旧尺寸。
            self.cached_content_size.set(Size::zero());
        }
    }

    on_child_visibility_changed => (&mut self) {
        // 条件页面切换后，旧可见子树的自然尺寸不再有效。
        self.cached_content_size.set(Size::zero());
    }

    measure => (&self, constraints: Constraints) -> Size {
        let intrinsic = self.intrinsic_size();
        let clamped = constraints.clamp(intrinsic);
        let cached = self.cached_content_size.get();
        let uses_content_floor = self.style.flex_grow <= 0.0;
        // 仅 indefinite（None 或 0）轴用 cached（子项溢出尺寸）撑开 measure；
        // 显式 >0 尺寸为定高/定宽，尊重 clamped，不被 cached 溢出撑大。
        let w = if uses_content_floor
            && cached.w > 0.0
            && self.style.width.is_none_or(|w| w <= 0.0)
        {
            clamped.w.max(cached.w)
        } else {
            clamped.w
        };
        let h = if uses_content_floor
            && cached.h > 0.0
            && self.style.height.is_none_or(|h| h <= 0.0)
        {
            clamped.h.max(cached.h)
        } else {
            clamped.h
        };
        // 内容缓存只在父级对应轴无上界时允许撑出视口；有限约束下必须重新
        // 收口，否则最大化阶段写入的缓存会让还原后的标题栏与内容区保持旧宽度。
        constraints.clamp(Size::new(w, h))
    }

    measure_natural => (&self, constraints: Constraints) -> Size {
        // 自然测量显式绕过 flex-grow 的零 basis 策略。
        constraints.clamp(self.natural_intrinsic_size())
    }

    flex_grow => (&self) -> f32 { self.style.flex_grow }

    flex_shrink => (&self) -> f32 { self.style.flex_shrink }

    align_self => (&self) -> Option<AlignItems> { self.style.align_self }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        // 只有显式 overflow hidden 才裁剪直接子树。
        self.style.clip_content.unwrap_or(false).then_some(frame)
    }

    grid_cell => (&self) -> Option<usize> { self.style.grid_cell }

    grid_column_span => (&self) -> u32 { self.style.grid_column_span }

    grid_row_span => (&self) -> u32 { self.style.grid_row_span }

    layout_margin => (&self) -> EdgeInsets { self.style.margin }

    picture_policy => (&self) -> PicturePolicy { PicturePolicy::Eligible }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        // frame 已是 border-box（父级已处理 margin）；背景画满 frame。
        let s = &self.style;
        let visual = BoxModel {
            margin: s.margin,
            border_width: s.border_width,
            padding: s.padding,
        }
        .visual_rect(frame);
        if visual.w <= 0.0 || visual.h <= 0.0 { return; }

        // 统一样式——绘制背景/边框/阴影/透明度
        apply_style(ctx, visual, s);
    }

    measure_children => (&self, frame: Rect, children: &[WidgetId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        let mut output = Vec::with_capacity(children.len());
        self.measure_children_reusing(frame, children, tree, &mut output);
        output
    }

    measure_children_into => (
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
        output: &mut Vec<LayoutChild>
    ) {
        self.measure_children_reusing(frame, children, tree, output);
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        let mut scratch = crate::ui::LayoutEngineScratch::default();
        let mut output = Vec::with_capacity(children.len());
        self.layout_children_reusing(frame, children, &mut scratch, &mut output);
        output
    }

    layout_children_into => (
        &self,
        frame: Rect,
        children: &[LayoutChild],
        _tree: &WidgetTree,
        scratch: &mut crate::ui::LayoutEngineScratch,
        output: &mut Vec<(WidgetId, Rect)>
    ) {
        self.layout_children_reusing(frame, children, scratch, output);
    }
}

/// 将 style::FlexDirection 转换为 layout::FlexDirection
fn convert_flex_direction(d: crate::ui::theme::style::FlexDirection) -> FlexDirection {
    match d {
        crate::ui::theme::style::FlexDirection::Row => FlexDirection::Row,
        crate::ui::theme::style::FlexDirection::Column => FlexDirection::Column,
        crate::ui::theme::style::FlexDirection::RowReverse => FlexDirection::RowReverse,
        crate::ui::theme::style::FlexDirection::ColumnReverse => FlexDirection::ColumnReverse,
    }
}

/// 将 style::JustifyContent 转换为 layout::JustifyContent
fn convert_justify(j: crate::ui::theme::style::JustifyContent) -> JustifyContent {
    match j {
        crate::ui::theme::style::JustifyContent::Start => JustifyContent::Start,
        crate::ui::theme::style::JustifyContent::Center => JustifyContent::Center,
        crate::ui::theme::style::JustifyContent::End => JustifyContent::End,
        crate::ui::theme::style::JustifyContent::SpaceBetween => JustifyContent::SpaceBetween,
        crate::ui::theme::style::JustifyContent::SpaceAround => JustifyContent::SpaceAround,
        crate::ui::theme::style::JustifyContent::SpaceEvenly => JustifyContent::SpaceEvenly,
        crate::ui::theme::style::JustifyContent::Stretch => JustifyContent::Stretch,
    }
}

/// 将 style::AlignItems 转换为 layout::AlignItems
fn convert_align(a: crate::ui::theme::style::AlignItems) -> AlignItems {
    match a {
        crate::ui::theme::style::AlignItems::Start => AlignItems::Start,
        crate::ui::theme::style::AlignItems::Center => AlignItems::Center,
        crate::ui::theme::style::AlignItems::End => AlignItems::End,
        crate::ui::theme::style::AlignItems::Stretch => AlignItems::Stretch,
    }
}

impl Default for Container {
    fn default() -> Self {
        Self::new()
    }
}

// 把 Container Rust 内核与 UIX 静态视觉组合为单一叶节点。
fn build_container_view(mut kernel: Container, visual: &'static ContainerVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Container {
    fn build(self) -> ViewNode {
        build_container_uix_root(self)
    }
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_container_uix_root(kernel: Container) -> ViewNode {
    crate::uix!("src/ui/widgets/containers/container/container.uix")
}

impl SnapshotSource for Container {
    fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Container {
            style: self.style.clone(),
        }
    }
}

impl Container {
    /// 创建使用默认容器样式的通用布局容器。
    pub fn new() -> Self {
        Self {
            style: CONTAINER_VISUAL_REF.default_style.resolve(),
            cached_content_size: Cell::new(Size::zero()),
            visual: CONTAINER_VISUAL_REF,
        }
    }

    // 统一拥有型与树级复用入口，避免两套测量语义随优化演进而分叉。
    fn measure_children_reusing(
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
        output: &mut Vec<LayoutChild>,
    ) {
        let content_rect = BoxModel {
            margin: self.style.margin,
            border_width: self.style.border_width,
            padding: self.style.padding,
        }
        .content_rect(frame);
        let child_constraints = self.child_measure_constraints(content_rect);
        output.clear();
        output.extend(children.iter().copied().filter_map(|child_id| {
            let visible = tree
                .get(child_id)
                .map(|node| node.visible())
                .unwrap_or(true);
            if !visible {
                tracing::debug!(
                    "[Container::measure_children] child {} is invisible, skipping",
                    child_id
                );
            }
            visible.then(|| child_from_tree_with_constraints(child_id, tree, child_constraints))
        }));
    }

    // 统一拥有型与树级复用入口；工作区由调用树独占，组件只借用一次求解。
    fn layout_children_reusing(
        &self,
        frame: Rect,
        children: &[LayoutChild],
        scratch: &mut crate::ui::LayoutEngineScratch,
        output: &mut Vec<(WidgetId, Rect)>,
    ) {
        output.clear();
        if children.is_empty() {
            self.cached_content_size.set(Size::zero());
            return;
        }

        let style = &self.style;
        let content_rect = BoxModel {
            margin: style.margin,
            border_width: style.border_width,
            padding: style.padding,
        }
        .content_rect(frame);
        // 零主轴是首次固有尺寸 bootstrap，不得提前返回，否则子节点会堆叠在原点。
        let main_axis_indefinite = matches!(
            style.flex_direction,
            crate::ui::theme::style::FlexDirection::Column
                | crate::ui::theme::style::FlexDirection::ColumnReverse
        ) && style.height.is_none_or(|height| height <= 0.0)
            || matches!(
                style.flex_direction,
                crate::ui::theme::style::FlexDirection::Row
                    | crate::ui::theme::style::FlexDirection::RowReverse
            ) && style.width.is_none_or(|width| width <= 0.0);
        let cross_axis_indefinite = if matches!(
            style.flex_direction,
            crate::ui::theme::style::FlexDirection::Row
                | crate::ui::theme::style::FlexDirection::RowReverse
        ) {
            content_rect.h <= self.visual.layout.bootstrap_cross_axis_threshold
        } else {
            content_rect.w <= self.visual.layout.bootstrap_cross_axis_threshold
        };
        let engine = FlexLayout {
            direction: convert_flex_direction(style.flex_direction),
            gap: style.gap,
            justify: convert_justify(style.justify_content),
            align: convert_align(style.align_items),
            wrap: style.flex_wrap,
            overflow_content: style.overflow_content,
            intrinsic_main: main_axis_indefinite,
            intrinsic_cross: cross_axis_indefinite,
        };

        // 临时取出输入数组，避免同时借用工作区与其中的 Flex 求解缓冲。
        let mut layout_children = std::mem::take(&mut scratch.layout_children);
        layout_children.clear();
        layout_children.extend_from_slice(children);
        for child in &mut layout_children {
            child.flex_shrink = self.visual.layout.child_flex_shrink;
        }
        let _total_size = engine.layout_into(content_rect, &layout_children, scratch);
        let positions = &scratch.flex.child_rects;
        self.cached_content_size.set(content_size_from_children(
            content_rect,
            positions,
            &layout_children,
        ));
        output.reserve(children.len());
        output.extend(
            children
                .iter()
                .zip(positions)
                .map(|(child, rect)| (child.id, *rect)),
        );
        layout_children.clear();
        scratch.layout_children = layout_children;
    }

    // ═══════════════════════════════════════════════════
    // 统一样式设置
    // ═══════════════════════════════════════════════════

    /// 批量设置 Style（替换所有现有值）。
    pub(crate) fn sync_from(&mut self, next: Self) {
        self.style = next.style;
        self.visual = next.visual;
    }

    /// 用指定样式完整替换容器的当前样式。
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
    pub fn bg(mut self, c: impl Into<ColorValue>) -> Self {
        self.style.background = Some(c.into());
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
    pub fn border(mut self, color: impl Into<ColorValue>, width: f32) -> Self {
        self.style.border_color = Some(color.into());
        self.style.border_width = EdgeInsets::uniform(width);
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
    pub fn color(mut self, c: impl Into<ColorValue>) -> Self {
        self.style.color = c.into();
        self
    }

    /// 设置字号。
    pub fn fs(mut self, s: impl Into<TypographyToken>) -> Self {
        self.style.font_size = s.into();
        self
    }

    /// 设置显示模式（Flex / None）。
    pub fn display(mut self, d: DisplayMode) -> Self {
        self.style.display = d;
        self
    }

    /// 设置 flex 方向。
    pub fn direction(mut self, d: crate::ui::theme::style::FlexDirection) -> Self {
        self.style.flex_direction = d;
        self
    }

    /// 设置子项间距。
    pub fn gap(mut self, g: f32) -> Self {
        self.style.gap = g;
        self
    }

    /// 设置主轴对齐。
    pub fn justify(mut self, j: crate::ui::theme::style::JustifyContent) -> Self {
        self.style.justify_content = j;
        self
    }

    /// 设置交叉轴对齐。
    pub fn align(mut self, a: crate::ui::theme::style::AlignItems) -> Self {
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

    /// 便捷方法：单独设置内边距。
    pub fn pad(mut self, p: EdgeInsets) -> Self {
        self.style.padding = p;
        self
    }

    /// 便捷方法：设置 flex 方向。
    pub fn dir(mut self, d: FlexDirection) -> Self {
        self.style.flex_direction = match d {
            FlexDirection::Row => crate::ui::theme::style::FlexDirection::Row,
            FlexDirection::Column => crate::ui::theme::style::FlexDirection::Column,
            FlexDirection::RowReverse => crate::ui::theme::style::FlexDirection::RowReverse,
            FlexDirection::ColumnReverse => crate::ui::theme::style::FlexDirection::ColumnReverse,
        };
        self
    }

    /// 允许内容溢出（跳过 flex-shrink，用于可滚动容器）。
    pub fn overflow_content(mut self) -> Self {
        self.style.overflow_content = true;
        self
    }

    fn child_measure_constraints(&self, content_rect: Rect) -> Constraints {
        let is_row = matches!(
            self.style.flex_direction,
            crate::ui::theme::style::FlexDirection::Row
                | crate::ui::theme::style::FlexDirection::RowReverse
        );
        // 与 Space 对齐：主轴始终 MAX。定高 Column 若用 content_rect.h 钳子项，
        // measure 会把 wrap 内容从 121 压回 106，Phase 1 写回后与 Phase 2 振荡；
        // ScrollView 内又无 parent_cap，会打满 converge。交叉轴在有明确尺寸时
        // 仍约束，供 wrap 计算行宽。
        let max_w = if is_row || self.style.width.is_none() {
            f32::MAX
        } else {
            content_rect.w
        };
        let max_h = if !is_row || self.style.height.is_none() {
            f32::MAX
        } else {
            content_rect.h
        };
        Constraints::loose(Size::new(max_w, max_h))
    }

    fn intrinsic_size(&self) -> Size {
        let bh = self.style.border_width.horizontal();
        let bv = self.style.border_width.vertical();
        let cached = self.cached_content_size.get();
        // flex_grow 子项必须以 0 为 basis，让父级分配确定空间；
        // 否则窗口缩小后仍用上一轮 cached 内容高/宽，ScrollView 视口被撑满 → 无滚动条。
        let grow = self.style.flex_grow > 0.0;
        let content_w = if cached.w > 0.0 {
            cached.w + self.style.padding.horizontal()
        } else {
            0.0
        };
        let content_h = if cached.h > 0.0 {
            cached.h + self.style.padding.vertical()
        } else {
            0.0
        };
        // 显式 >0 是定高/定宽，尊重之，不被内容溢出撑大（内容溢出走 overflow）；
        // None 或 0 视为 indefinite，用内容尺寸撑开（ScrollView 内 size(_,0) 撑开用）。
        let effective_w = match self.style.width {
            Some(w) if w > 0.0 => w,
            _ => {
                if grow {
                    0.0
                } else {
                    content_w
                }
            }
        };
        let effective_h = match self.style.height {
            Some(h) if h > 0.0 => h,
            _ => {
                if grow {
                    0.0
                } else {
                    content_h
                }
            }
        };
        Size::new(effective_w + bh, effective_h + bv)
    }

    // 返回不受 flex-grow basis 归零影响的内容固有尺寸。
    fn natural_intrinsic_size(&self) -> Size {
        // 边框属于 Container 的 border-box 自然宽度。
        let border_width = self.style.border_width.horizontal();
        // 边框属于 Container 的 border-box 自然高度。
        let border_height = self.style.border_width.vertical();
        // 子布局缓存是当前组件拥有的内容尺寸事实。
        let cached = self.cached_content_size.get();
        // 有内容时把水平内边距计入自然 border-box。
        let content_width = if cached.w > 0.0 {
            // 缓存不含 Container 自己的内边距。
            cached.w + self.style.padding.horizontal()
        } else {
            // 尚无内容缓存时保持稳定零宽 bootstrap。
            0.0
        };
        // 有内容时把垂直内边距计入自然 border-box。
        let content_height = if cached.h > 0.0 {
            // 缓存不含 Container 自己的内边距。
            cached.h + self.style.padding.vertical()
        } else {
            // 尚无内容缓存时保持稳定零高 bootstrap。
            0.0
        };
        // 显式正宽仍优先于子树自然宽度。
        let width = self
            // 读取可选的声明宽度。
            .style
            // 选择严格为正的有效宽度。
            .width
            // 零值仍代表未指定轴。
            .filter(|width| *width > 0.0)
            // 未指定宽度时采用真实内容宽度。
            .unwrap_or(content_width);
        // 显式正高仍优先于子树自然高度。
        let height = self
            // 读取可选的声明高度。
            .style
            // 选择严格为正的有效高度。
            .height
            // 零值仍代表未指定轴。
            .filter(|height| *height > 0.0)
            // 未指定高度时采用真实内容高度。
            .unwrap_or(content_height);
        // 返回包含边框的完整自然 border-box 尺寸。
        Size::new(width + border_width, height + border_height)
    }
}

// 容器布局缓存的内部回归测试。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/containers/container__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
