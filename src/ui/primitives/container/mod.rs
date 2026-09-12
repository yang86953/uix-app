//! Container widget — flexbox layout container with background/border.

use std::cell::Cell;

mod reflow;

use crate::core::{Constraints, EdgeInsets, Rect, Size};
use crate::draw::scene::PicturePolicy;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::tree_measure::child_from_tree_with_flex_constraints;
use crate::widget;
// 导入共享布局入口与内容外尺寸计算。
use crate::ui::layout::engine::{BoxModel, FlexLayout, LayoutChild, content_size_from_children};

use crate::ui::layout::{AlignItems, FlexDirection, JustifyContent};
use crate::ui::theme::style::{
    BoxShadowDef, ColorValue, DisplayMode, Style, StyleDiff, TypographyToken, apply_style,
};
use crate::ui::{PrimitiveSnapshot, SnapshotSource};
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
}

// 全部 Container 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ContainerVisual {
    default_style: ContainerStyleRole,
    layout: ContainerLayoutVisual,
}

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

    on_children_changed => (&mut self, _child_count: usize) {
        // 任意子树拓扑变化都会使旧内容尺寸失效；嵌套 If 保留兄弟节点时也必须清空。
        self.cached_content_size.set(Size::zero());
    }

    on_child_visibility_changed => (&mut self) {
        // 条件页面切换后，旧可见子树的自然尺寸不再有效。
        self.cached_content_size.set(Size::zero());
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    measure_natural => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    flex_basis => (&self, parent_direction: FlexDirection) -> Option<f32> {
        self.box_model().flex_basis(parent_direction, self.style.flex_grow > 0.0, self.style.width, self.style.height)
    }

    minimum_size => (&self) -> Size {
        self.box_model().border_box_size(Size::zero())
    }

    size_constraints => (&self) -> crate::ui::theme::style::SizeConstraints {
        self.style.size_constraints()
    }

    flex_grow => (&self) -> f32 { self.style.flex_grow }

    flex_layout_axes => (&self) -> Option<(FlexDirection, AlignItems)> {
        (self.style.display == DisplayMode::Flex).then(|| (convert_flex_direction(self.style.flex_direction), convert_align(self.style.align_items)))
    }

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

    layout_children => (&self, frame: Rect, children: &[LayoutChild], tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        let mut scratch = crate::ui::LayoutEngineScratch::default();
        let mut output = Vec::with_capacity(children.len());
        self.layout_children_reusing(frame, children, tree, &mut scratch, &mut output);
        output
    }

    layout_children_into => (
        &self,
        frame: Rect,
        children: &[LayoutChild],
        tree: &WidgetTree,
        scratch: &mut crate::ui::LayoutEngineScratch,
        output: &mut Vec<(WidgetId, Rect)>
    ) {
        self.layout_children_reusing(frame, children, tree, scratch, output);
    }

    reconcile_sync => (sync_from) {}

    snapshot => (&self) -> crate::ui::WidgetSnapshotFields {
        <Self as crate::ui::SnapshotSource>::snapshot_fields(self)
    }

    declaration_style => (&mut self, style: &crate::ui::Style, declared: &crate::ui::StyleDiff, flex_grow_override: Option<f32>, flex_shrink_override: Option<f32>) {
        let c = self;
        let style_is_default = style == &crate::ui::Style::default();
        let _ = (style_is_default, declared, flex_grow_override, flex_shrink_override);

                if !style_is_default {
                    c.style = c.style.clone().apply(style.clone());
                }
                // 显式零/none/auto 声明在值合并之上恢复。
                if !declared.is_empty() {
                    declared.clone().apply_to(&mut c.style);
                }
                // View DSL 显式 flex 覆盖（含 0.0），Style::apply 无法表达「设为默认值」
                if let Some(g) = flex_grow_override {
                    c.style.flex_grow = g;
                }
                if let Some(s) = flex_shrink_override {
                    c.style.flex_shrink = s;
                }

    }

    layout_size_locks => (&self) -> (bool, bool) { (self.style.width.is_some(), self.style.height.is_some()) }
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
    build_container_view(kernel, CONTAINER_VISUAL_REF)
}

impl SnapshotSource for Container {
    fn snapshot_fields(&self) -> crate::ui::WidgetSnapshotFields {
        let fields = {
            PrimitiveSnapshot::Container {
                style: self.style.clone(),
            }
        };
        fields.into()
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
        if self.style.display == DisplayMode::Grid {
            output.clear();
            let explicit = |value: Option<f32>| value.is_some_and(|v| v.is_finite() && v > 0.0);
            let constraints = Constraints::loose(Size::new(
                if explicit(self.style.width) {
                    content_rect.w
                } else {
                    f32::MAX
                },
                if explicit(self.style.height) {
                    content_rect.h
                } else {
                    f32::MAX
                },
            ));
            let reference = self.percent_reference(children.first().copied(), tree, content_rect);
            output.extend(children.iter().copied().filter(|id| tree.get(*id).is_some_and(|node| node.visible())).map(|id| {
                crate::ui::widget_runtime::tree_measure::child_from_tree_with_constraints_in(id, tree, constraints, reference)
            }));
            return;
        }
        let child_constraints = self.child_measure_constraints(content_rect);
        let reference = self.percent_reference(children.first().copied(), tree, content_rect);
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
            visible.then(|| {
                child_from_tree_with_flex_constraints(
                    child_id,
                    tree,
                    child_constraints,
                    convert_flex_direction(self.style.flex_direction),
                    reference,
                )
            })
        }));
    }

    // 只有容器自身确定的轴才作为子项百分比参照，避免内容撑开轴形成自引用；
    // 参照值取父级本轮实际分配 frame 的内容盒，不用声明字面量。
    fn percent_reference(
        &self,
        first_child: Option<WidgetId>,
        tree: &WidgetTree,
        content_rect: Rect,
    ) -> crate::ui::theme::style::PercentReference {
        // 零仍沿用 UIX 既有 auto 哨兵语义，只作“该轴确定”的依据。
        let explicit = |value: Option<f32>| value.is_some_and(|v| v.is_finite() && v > 0.0);
        crate::ui::widget_runtime::tree_measure::percent_reference_for_children(
            first_child,
            tree,
            content_rect,
            (explicit(self.style.width), explicit(self.style.height)),
        )
    }

    // 统一拥有型与树级复用入口；工作区由调用树独占，组件只借用一次求解。
    fn layout_children_reusing(
        &self,
        frame: Rect,
        children: &[LayoutChild],
        tree: &WidgetTree,
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
        if style.display == DisplayMode::Grid {
            let engine = crate::ui::GridLayout {
                columns: vec![],
                rows: vec![],
                col_gap: style.grid_column_gap,
                row_gap: style.grid_row_gap,
                align_items: style.align_items,
                justify_items: crate::ui::layout::JustifyContent::Start,
                justify_content: style.justify_content,
            };
            let grid = scratch.grid();
            let intrinsic = engine.layout_with_tracks_into(
                Rect::new(content_rect.x, content_rect.y, 0.0, 0.0),
                &style.grid_template_columns,
                &style.grid_template_rows,
                children,
                &mut grid.children,
                &mut grid.compute,
            );
            self.cached_content_size.set(intrinsic);
            engine.layout_with_tracks_into(
                content_rect,
                &style.grid_template_columns,
                &style.grid_template_rows,
                children,
                &mut grid.children,
                &mut grid.compute,
            );
            output.extend(
                children
                    .iter()
                    .zip(&grid.compute.child_rects)
                    .map(|(child, rect)| (child.id, *rect)),
            );
            return;
        }
        // 无显式主轴尺寸且不参与 grow 的容器始终由内容撑开；这也是 column_fit
        // 进入 ScrollView 后建立自然滚动范围的前提。flexGrow 链只在 bootstrap
        // 阶段保持自然尺寸，取得实际 frame 后必须分配剩余空间。
        let (main_axis_extent, explicit_main_axis) = if matches!(
            style.flex_direction,
            crate::ui::theme::style::FlexDirection::Column
                | crate::ui::theme::style::FlexDirection::ColumnReverse
        ) {
            (
                content_rect.h,
                style
                    .height
                    .is_some_and(|height| height.is_finite() && height > 0.0),
            )
        } else {
            (
                content_rect.w,
                style
                    .width
                    .is_some_and(|width| width.is_finite() && width > 0.0),
            )
        };
        let is_column = matches!(
            style.flex_direction,
            crate::ui::theme::style::FlexDirection::Column
                | crate::ui::theme::style::FlexDirection::ColumnReverse
        );
        // 已继承有限宽度的 Row 不能继续按无界主轴排布；Column 的自动高度仍由内容撑开。
        let main_axis_indefinite = !explicit_main_axis
            && ((is_column && (style.flex_grow <= 0.0
                || crate::ui::widget_runtime::tree_measure::container_height_is_flex_cross_content(children, tree)))
                || main_axis_extent <= self.visual.layout.bootstrap_cross_axis_threshold);
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

        // 子项自身拥有 grow/shrink 声明，容器不得用视觉默认值覆盖其公开契约。
        let _total_size = engine.layout_into(content_rect, children, scratch);
        // 横排先分配主轴宽度，再按各槽位重测高度；否则折行仍沿用无限宽的单行高度。
        let reflowed = if matches!(
            engine.direction,
            FlexDirection::Row | FlexDirection::RowReverse
        ) {
            reflow::row_children_at_allocated_width(
                children,
                &scratch.flex.child_rects,
                tree,
                self.child_measure_constraints(content_rect),
                self.percent_reference(children.first().map(|child| child.id), tree, content_rect),
            )
        } else {
            std::borrow::Cow::Borrowed(children)
        };
        if matches!(reflowed, std::borrow::Cow::Owned(_)) {
            engine.layout_into(content_rect, &reflowed, scratch);
        }
        let children = reflowed.as_ref();
        let positions = &scratch.flex.child_rects;
        let mut content_size = content_size_from_children(content_rect, positions, children);
        if !main_axis_indefinite && !explicit_main_axis && style.flex_grow <= 0.0 {
            // 内容尺寸容器（无显式主轴尺寸且不 grow）被继承的小宽度挤压后，
            // positions 的主轴末端只是挤压结果，不能当作自然尺寸回写缓存；
            // 否则下一轮 measure 永远报告挤压值，父级再也分配不回自然宽度
            // （顶栏窗口控制按钮塌成细条即此反馈环）。主轴自然下限取子项
            // 测量基线加间距，与 overflow 布局的内容语义保持一致。
            let gap = crate::ui::layout::engine::finite_or_zero(style.gap);
            let basis_sum: f32 = children
                .iter()
                .map(|child| {
                    crate::ui::layout::engine::finite_non_negative(child.measured_size.w)
                        + crate::ui::layout::engine::finite_or_zero(child.margin.horizontal())
                })
                .sum::<f32>()
                + gap * children.len().saturating_sub(1) as f32;
            content_size.w = content_size.w.max(basis_sum);
        }
        self.cached_content_size.set(content_size);
        output.reserve(children.len());
        output.extend(
            children
                .iter()
                .zip(positions)
                .map(|(child, rect)| (child.id, *rect)),
        );
    }

    // ═══════════════════════════════════════════════════
    // 统一样式设置
    // ═══════════════════════════════════════════════════

    /// Replaces declaration fields while retaining runtime layout state.
    pub fn sync_from(&mut self, next: Self) {
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

    /// 在当前样式上应用类型化差异覆盖；未声明字段保留当前值。
    ///
    /// 与 [`Self::apply_style`] 的值推断不同，显式零、false、none、auto
    /// 都可以作为恢复声明（如把继承的 padding 恢复为 0）。
    pub fn style_diff(mut self, diff: StyleDiff) -> Self {
        diff.apply_to(&mut self.style);
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
        let max_w = if is_row || content_rect.w <= self.visual.layout.bootstrap_cross_axis_threshold
        {
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

    fn box_model(&self) -> BoxModel {
        BoxModel {
            margin: self.style.margin,
            border_width: self.style.border_width,
            padding: self.style.padding,
        }
    }

    fn intrinsic_size(&self) -> Size {
        self.box_model().preferred_size(
            self.cached_content_size.get(),
            self.style.width,
            self.style.height,
        )
    }
}

const CONTAINER_VISUAL: ContainerVisual = ContainerVisual {
    default_style: ContainerStyleRole::Default,
    layout: ContainerLayoutVisual {
        bootstrap_cross_axis_threshold: 1.0,
    },
};
const CONTAINER_VISUAL_REF: &ContainerVisual = &CONTAINER_VISUAL;
