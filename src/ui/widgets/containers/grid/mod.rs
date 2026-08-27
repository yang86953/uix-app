//! Grid widget - CSS Grid-like layout container.

// Grid 在布局轮次间保存由轨道求解得到的固有内容尺寸。
use std::cell::Cell;

use crate::core::{Constraints, EdgeInsets, Rect, Size};
use crate::draw::Color;
use crate::draw::scene::PicturePolicy;
use crate::ui::layout::engine::{BoxModel, GridLayout, LayoutChild};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::tree_measure::child_from_tree_with_constraints;
use crate::widget;

use crate::ui::layout::{AlignItems, GridTrack, JustifyContent};
use crate::ui::theme::style::{ColorValue, DisplayMode, Style, apply_style as paint_style};
use crate::ui::{SnapshotFields, SnapshotSource};
use crate::ui::{View, ViewNode, WidgetId, WidgetTree};

/// 自定义响应式断点无效时返回的 typed 错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakpointError {
    /// 至少一个断点不是有限数值。
    NonFinite,
    /// 至少一个断点为负数。
    Negative,
    /// 断点没有从固定的零宽度起点严格递增。
    NotStrictlyAscending,
}

impl std::fmt::Display for BreakpointError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::NonFinite => "breakpoints must be finite",
            Self::Negative => "breakpoints must be nonnegative",
            Self::NotStrictlyAscending => "breakpoints must be strictly ascending from xs=0",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for BreakpointError {}

/// 响应式 Grid 的 logical 宽度断点。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Breakpoints {
    sm: f32,
    md: f32,
    lg: f32,
    xl: f32,
    xxl: f32,
}

// 标识 UIX 为 Grid 选择的共享主题样式角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GridStyleRole {
    Default,
}

impl GridStyleRole {
    fn resolve(self) -> Style {
        match self {
            Self::Default => Style::default().with_display(DisplayMode::Grid),
        }
    }
}

// 保存 Grid 的响应式单元数、默认单元对齐与等分轨道比例。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GridLayoutVisual {
    responsive_units: usize,
    default_justify_items: JustifyContent,
    equal_track_fraction: f32,
}

// 全部 Grid 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GridVisual {
    default_style: GridStyleRole,
    breakpoints: Breakpoints,
    layout: GridLayoutVisual,
}

crate::uix_items!("src/ui/widgets/containers/grid/grid.uix");

pub(crate) const fn grid_default_style_role() -> GridStyleRole {
    GridStyleRole::Default
}

pub(crate) const fn grid_justify_start() -> JustifyContent {
    JustifyContent::Start
}

pub(crate) const fn grid_breakpoints(sm: f32, md: f32, lg: f32, xl: f32, xxl: f32) -> Breakpoints {
    Breakpoints {
        sm,
        md,
        lg,
        xl,
        xxl,
    }
}

impl Breakpoints {
    /// 构造严格递增的自定义断点；`xs` 固定为 `0`。
    pub fn new(sm: f32, md: f32, lg: f32, xl: f32, xxl: f32) -> Result<Self, BreakpointError> {
        let values = [sm, md, lg, xl, xxl];
        if values.iter().any(|value| !value.is_finite()) {
            return Err(BreakpointError::NonFinite);
        }
        if values.iter().any(|value| *value < 0.0) {
            return Err(BreakpointError::Negative);
        }
        if sm <= 0.0 || values.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(BreakpointError::NotStrictlyAscending);
        }
        Ok(Self {
            sm,
            md,
            lg,
            xl,
            xxl,
        })
    }

    /// Ant Design 的默认断点，单位为 logical px。
    pub const fn antd() -> Self {
        GRID_VISUAL.breakpoints
    }

    /// 返回固定为零的超小断点。
    pub const fn xs(self) -> f32 {
        0.0
    }

    /// 返回小断点。
    pub const fn sm(self) -> f32 {
        self.sm
    }

    /// 返回中等断点。
    pub const fn md(self) -> f32 {
        self.md
    }

    /// 返回大断点。
    pub const fn lg(self) -> f32 {
        self.lg
    }

    /// 返回超大断点。
    pub const fn xl(self) -> f32 {
        self.xl
    }

    /// 返回双倍超大断点。
    pub const fn xxl(self) -> f32 {
        self.xxl
    }
}

impl Default for Breakpoints {
    fn default() -> Self {
        Self::antd()
    }
}

/// 响应式 Grid 中与源顺序子节点一一对应的列配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Col {
    span: u8,
    sm: Option<u8>,
    md: Option<u8>,
    lg: Option<u8>,
    xl: Option<u8>,
    xxl: Option<u8>,
    offset: u8,
    order: i32,
}

impl Col {
    /// 创建默认跨越全部二十四单元的列配置。
    pub const fn new() -> Self {
        Self {
            span: GRID_VISUAL.layout.responsive_units as u8,
            sm: None,
            md: None,
            lg: None,
            xl: None,
            xxl: None,
            offset: 0,
            order: 0,
        }
    }

    /// 设置一到二十四之间的基础跨列数。
    pub fn span(mut self, span: u32) -> Self {
        self.span = Self::normalize_span(span);
        self
    }

    /// 设置小断点及以上的跨列数。
    pub fn sm(mut self, span: u32) -> Self {
        self.sm = Some(Self::normalize_span(span));
        self
    }

    /// 设置中等断点及以上的跨列数。
    pub fn md(mut self, span: u32) -> Self {
        self.md = Some(Self::normalize_span(span));
        self
    }

    /// 设置大断点及以上的跨列数。
    pub fn lg(mut self, span: u32) -> Self {
        self.lg = Some(Self::normalize_span(span));
        self
    }

    /// 设置超大断点及以上的跨列数。
    pub fn xl(mut self, span: u32) -> Self {
        self.xl = Some(Self::normalize_span(span));
        self
    }

    /// 设置双倍超大断点及以上的跨列数。
    pub fn xxl(mut self, span: u32) -> Self {
        self.xxl = Some(Self::normalize_span(span));
        self
    }

    /// 设置当前列之前保留的栅格单元数。
    pub fn offset(mut self, offset: u32) -> Self {
        self.offset = offset.min((GRID_VISUAL.layout.responsive_units - 1) as u32) as u8;
        self
    }

    /// 设置不改变源节点身份的视觉排序值。
    pub fn order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    /// 返回基础跨列数。
    pub const fn base_span(self) -> u8 {
        self.span
    }

    /// 返回基础前置偏移单元数。
    pub const fn base_offset(self) -> u8 {
        self.offset
    }

    /// 返回视觉排序值。
    pub const fn visual_order(self) -> i32 {
        self.order
    }

    fn span_at(self, width: f32, breakpoints: Breakpoints) -> usize {
        let mut span = self.span;
        for (threshold, override_span) in [
            (breakpoints.sm, self.sm),
            (breakpoints.md, self.md),
            (breakpoints.lg, self.lg),
            (breakpoints.xl, self.xl),
            (breakpoints.xxl, self.xxl),
        ] {
            if width >= threshold {
                if let Some(value) = override_span {
                    span = value;
                }
            }
        }
        span as usize
    }

    fn effective_offset(self, span: usize) -> usize {
        (self.offset as usize).min(GRID_VISUAL.layout.responsive_units - span)
    }

    fn normalize_span(span: u32) -> u8 {
        span.clamp(1, GRID_VISUAL.layout.responsive_units as u32) as u8
    }
}

impl Default for Col {
    fn default() -> Self {
        Self::new()
    }
}

widget! {
    /// Grid container widget.
    pub struct Grid {
        /// 控制网格容器尺寸、间距、对齐与伸缩行为的统一样式。
        pub style: Style,
        breakpoints: Option<Breakpoints>,
        cols: Vec<Col>,
        /// 保存零轴启动后由轨道与子内容求得的自然内容尺寸。
        cached_content_size: Cell<Size>,
        /// 同目录 UIX 生成的唯一静态视觉表。
        pub(crate) visual: &'static GridVisual,
    }

    measure => (&self, constraints: Constraints) -> Size {
        // 显式尺寸优先，无显式尺寸时由上一轮轨道内容撑开。
        constraints.clamp(self.intrinsic_size())
    }

    on_children_changed => (&mut self, child_count: usize) {
        // 移除最后一个子节点后旧轨道内容不再构成有效测量下限。
        if child_count == 0 {
            // 立即清除缓存，避免空 Grid 继续保留旧尺寸。
            self.cached_content_size.set(Size::zero());
        }
    }

    on_child_visibility_changed => (&mut self) {
        // 网格的固有范围由当前可见单元共同决定。
        self.cached_content_size.set(Size::zero());
    }

    layout_margin => (&self) -> EdgeInsets { self.style.margin }

    flex_grow => (&self) -> f32 { self.style.flex_grow }

    flex_shrink => (&self) -> f32 { self.style.flex_shrink }

    align_self => (&self) -> Option<AlignItems> { self.style.align_self }

    grid_cell => (&self) -> Option<usize> { self.style.grid_cell }

    grid_column_span => (&self) -> u32 { self.style.grid_column_span }

    grid_row_span => (&self) -> u32 { self.style.grid_row_span }

    picture_policy => (&self) -> PicturePolicy { PicturePolicy::Eligible }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        // 父布局已在 frame 外消费 margin，Grid 绘制只使用 border-box。
        let visual = self.visual_rect(frame);
        if visual.w <= 0.0 || visual.h <= 0.0 { return; }
        paint_style(ctx, visual, &self.style);
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

impl Default for Grid {
    fn default() -> Self {
        Self::new()
    }
}

// 把 Grid Rust 内核与 UIX 静态视觉组合为单一叶节点。
fn build_grid_view(mut kernel: Grid, visual: &'static GridVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Grid {
    fn build(self) -> ViewNode {
        build_grid_uix_root(self)
    }
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_grid_uix_root(kernel: Grid) -> ViewNode {
    crate::uix!("src/ui/widgets/containers/grid/grid.uix")
}

impl SnapshotSource for Grid {
    fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Grid {
            style: self.style.clone(),
            breakpoints: self.breakpoints,
            cols: self.cols.clone(),
        }
    }
}

impl Grid {
    /// 创建使用 Grid 显示模式的空容器。
    pub fn new() -> Self {
        Self {
            style: GRID_VISUAL_REF.default_style.resolve(),
            breakpoints: None,
            cols: Vec::new(),
            // 新 Grid 尚无已求解的轨道内容。
            cached_content_size: Cell::new(Size::zero()),
            visual: GRID_VISUAL_REF,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.style = next.style;
        self.breakpoints = next.breakpoints;
        self.cols = next.cols;
        self.visual = next.visual;
        // 保留 cached_content_size，避免无关声明协调丢失布局固有尺寸。
    }

    // 结合显式尺寸、内容缓存与盒模型计算 Grid 的自然 border-box 尺寸。
    fn intrinsic_size(&self) -> Size {
        // 读取上一轮由零空间轨道求解得到的内容尺寸。
        let cached = self.cached_content_size.get();
        // 有内容时把横向 padding 纳入自然 border-box。
        let content_width = if cached.w > 0.0 {
            // padding 位于内容与 border 之间。
            cached.w + self.style.padding.horizontal()
        } else {
            // 空内容不单独物化 padding 尺寸，保持既有空 Grid 语义。
            0.0
        };
        // 有内容时把纵向 padding 纳入自然 border-box。
        let content_height = if cached.h > 0.0 {
            // padding 位于内容与 border 之间。
            cached.h + self.style.padding.vertical()
        } else {
            // 空内容不单独物化 padding 尺寸。
            0.0
        };
        // flex-grow Grid 继续以零为未分配主尺寸，避免缓存阻止父级收缩。
        let grows = self.style.flex_grow > 0.0;
        // 显式正宽度优先，否则采用可用的内容缓存。
        let width = match self.style.width {
            // 正宽度保持作者声明。
            Some(width) if width > 0.0 => width,
            // grow 子项把剩余空间分配交给父级。
            _ if grows => 0.0,
            // 普通 Auto Grid 使用自然内容宽度。
            _ => content_width,
        };
        // 显式正高度优先，否则采用可用的内容缓存。
        let height = match self.style.height {
            // 正高度保持作者声明。
            Some(height) if height > 0.0 => height,
            // grow 子项把剩余空间分配交给父级。
            _ if grows => 0.0,
            // 普通 Auto Grid 使用自然内容高度。
            _ => content_height,
        };
        // border 始终属于最终 border-box 尺寸。
        Size::new(
            // 横向尺寸包含左右 border。
            width + self.style.border_width.horizontal(),
            // 纵向尺寸包含上下 border。
            height + self.style.border_width.vertical(),
        )
    }

    // 返回 Grid 应绘制的 border-box，margin 只由父布局在 frame 外消费。
    fn visual_rect(&self, frame: Rect) -> Rect {
        // 复用共享盒模型入口保证与 Container 一致。
        BoxModel {
            // margin 保留在模型中，但 visual_rect 不会重复扣除它。
            margin: self.style.margin,
            // 边框厚度属于视觉 border-box。
            border_width: self.style.border_width,
            // padding 只影响内容区。
            padding: self.style.padding,
        }
        // 同时归一化最终实际视觉矩形。
        .visual_rect(frame)
    }

    /// 替换样式并强制保持 Grid 显示模式。
    pub fn style(mut self, style: Style) -> Self {
        self.style = style.with_display(DisplayMode::Grid);
        self.ensure_responsive_tracks();
        self
    }

    /// 设置显式列轨并关闭响应式二十四单元模式。
    pub fn columns(mut self, cols: Vec<GridTrack>) -> Self {
        self.style.grid_template_columns = cols;
        self.breakpoints = None;
        self.cols.clear();
        self
    }

    /// 创建使用 24 单元栅格和 Ant Design 默认断点的响应式 Grid。
    pub fn responsive() -> Self {
        let mut grid = Self::new();
        grid.breakpoints = Some(Breakpoints::antd());
        grid.ensure_responsive_tracks();
        grid
    }

    /// 设置断点并启用响应式布局。
    pub fn breakpoints(mut self, breakpoints: Breakpoints) -> Self {
        self.breakpoints = Some(breakpoints);
        self.ensure_responsive_tracks();
        self
    }

    /// 设置与源顺序子节点对应的列配置并启用响应式布局。
    pub fn cols(mut self, cols: Vec<Col>) -> Self {
        self.cols = cols;
        self.breakpoints.get_or_insert_with(Breakpoints::antd);
        self.ensure_responsive_tracks();
        self
    }

    /// 判断是否启用了响应式二十四单元模式。
    pub fn is_responsive(&self) -> bool {
        self.breakpoints.is_some()
    }

    /// 设置显式行轨。
    pub fn rows(mut self, rows: Vec<GridTrack>) -> Self {
        self.style.grid_template_rows = rows;
        self
    }

    /// 设置列轨间距。
    pub fn col_gap(mut self, gap: f32) -> Self {
        self.style.grid_column_gap = gap;
        self
    }

    /// 设置行轨间距。
    pub fn row_gap(mut self, gap: f32) -> Self {
        self.style.grid_row_gap = gap;
        self
    }

    /// 同时设置通用、列轨和行轨间距。
    pub fn gap(mut self, gap: f32) -> Self {
        self.style.gap = gap;
        self.style.grid_column_gap = gap;
        self.style.grid_row_gap = gap;
        self
    }

    /// 设置内容内边距。
    pub fn pad(mut self, padding: EdgeInsets) -> Self {
        self.style.padding = padding;
        self
    }

    /// 设置自定义背景色。
    pub fn bg(mut self, color: Color) -> Self {
        self.style.background = Some(ColorValue::Custom(color));
        self
    }

    /// 设置统一颜色和宽度的边框。
    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.style.border_color = Some(ColorValue::Custom(color));
        self.style.border_width = EdgeInsets::uniform(width);
        self
    }

    /// 设置圆角半径。
    pub fn rounded(mut self, radius: f32) -> Self {
        self.style.border_radius = radius;
        self
    }

    /// 同时设置显式宽度和高度。
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.style.width = Some(width);
        self.style.height = Some(height);
        self
    }

    /// 设置单元格内子项的交叉轴对齐方式。
    pub fn align(mut self, align: AlignItems) -> Self {
        self.style.align_items = align;
        self
    }

    /// 设置整组列轨在可用空间内的水平分布。
    pub fn justify(mut self, justify: JustifyContent) -> Self {
        self.style.justify_content = justify;
        self
    }

    /// 便捷：包裹多个子节点（等价 `tree! { Grid::responsive() => [...] }`）。
    pub fn children<I>(self, nodes: Vec<I>) -> crate::ui::widget_runtime::widget::WidgetNode
    where
        I: crate::ui::IntoWidgetNode,
    {
        crate::ui::widget_runtime::widget::WidgetNode::new(
            Box::new(self),
            nodes.into_iter().map(|node| node.into_node()).collect(),
        )
    }

    /// 合并非默认样式字段并保持 Grid 显示模式。
    pub fn apply_style(&mut self, style: &Style) {
        self.style = self.style.clone().apply(style.clone());
        self.style.display = DisplayMode::Grid;
        self.ensure_responsive_tracks();
    }

    /// 创建两个等宽弹性列的 Grid。
    pub fn two_columns() -> Self {
        let fraction = GRID_VISUAL_REF.layout.equal_track_fraction;
        Self::new().columns(vec![GridTrack::Fr(fraction), GridTrack::Fr(fraction)])
    }

    /// 创建三个等宽弹性列的 Grid。
    pub fn three_columns() -> Self {
        let fraction = GRID_VISUAL_REF.layout.equal_track_fraction;
        Self::new().columns(vec![
            GridTrack::Fr(fraction),
            GridTrack::Fr(fraction),
            GridTrack::Fr(fraction),
        ])
    }

    // 统一拥有型与树级复用入口，保持 Grid 子项约束只有一套语义。
    fn measure_children_reusing(
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
        output: &mut Vec<LayoutChild>,
    ) {
        output.clear();
        if self.style.grid_template_columns.is_empty() || children.is_empty() {
            return;
        }
        let content_rect = BoxModel {
            margin: self.style.margin,
            border_width: self.style.border_width,
            padding: self.style.padding,
        }
        .content_rect(frame);
        let max_width = if self.style.width.is_some_and(|width| width > 0.0) {
            content_rect.w
        } else {
            f32::MAX
        };
        let max_height = if self.style.height.is_some_and(|height| height > 0.0) {
            content_rect.h
        } else {
            f32::MAX
        };
        let constraints = Constraints::loose(Size::new(max_width, max_height));
        output.extend(
            children
                .iter()
                .copied()
                .map(|id| child_from_tree_with_constraints(id, tree, constraints)),
        );
    }

    // Grid 借用布局树唯一工作区，先求自然尺寸，再以实际 frame 复写最终位置。
    fn layout_children_reusing(
        &self,
        frame: Rect,
        children: &[LayoutChild],
        scratch: &mut crate::ui::LayoutEngineScratch,
        output: &mut Vec<(WidgetId, Rect)>,
    ) {
        output.clear();
        if self.style.grid_template_columns.is_empty() || children.is_empty() {
            self.cached_content_size.set(Size::zero());
            return;
        }
        let content_rect = BoxModel {
            margin: self.style.margin,
            border_width: self.style.border_width,
            padding: self.style.padding,
        }
        .content_rect(frame);
        let grid = scratch.grid();
        let layout_children = if self.breakpoints.is_some() {
            self.responsive_children_into(
                content_rect.w,
                children,
                &mut grid.layout_children,
                &mut grid.visual_order,
            );
            grid.layout_children.as_slice()
        } else {
            children
        };
        let engine = GridLayout {
            columns: Vec::new(),
            rows: Vec::new(),
            col_gap: self.effective_col_gap(),
            row_gap: self.effective_row_gap(),
            align_items: self.style.align_items,
            justify_items: self.visual.layout.default_justify_items,
            justify_content: self.style.justify_content,
        };
        let intrinsic_size = engine.layout_with_tracks_into(
            Rect::new(content_rect.x, content_rect.y, 0.0, 0.0),
            &self.style.grid_template_columns,
            &self.style.grid_template_rows,
            layout_children,
            &mut grid.children,
            &mut grid.compute,
        );
        self.cached_content_size.set(intrinsic_size);
        let _ = engine.layout_with_tracks_into(
            content_rect,
            &self.style.grid_template_columns,
            &self.style.grid_template_rows,
            layout_children,
            &mut grid.children,
            &mut grid.compute,
        );
        output.reserve(layout_children.len());
        output.extend(
            layout_children
                .iter()
                .zip(grid.compute.child_rects.iter())
                .map(|(child, rect)| (child.id, *rect)),
        );
    }

    fn effective_col_gap(&self) -> f32 {
        if self.style.grid_column_gap != 0.0 {
            self.style.grid_column_gap
        } else {
            self.style.gap
        }
    }

    fn effective_row_gap(&self) -> f32 {
        if self.style.grid_row_gap != 0.0 {
            self.style.grid_row_gap
        } else {
            self.style.gap
        }
    }

    fn ensure_responsive_tracks(&mut self) {
        if self.breakpoints.is_some() {
            self.style.grid_template_columns =
                vec![
                    GridTrack::Fr(self.visual.layout.equal_track_fraction);
                    self.visual.layout.responsive_units
                ];
        }
    }

    fn responsive_children_into(
        &self,
        available_width: f32,
        children: &[LayoutChild],
        configured: &mut Vec<LayoutChild>,
        visual_order: &mut Vec<usize>,
    ) {
        configured.clear();
        configured.extend_from_slice(children);
        let Some(breakpoints) = self.breakpoints else {
            return;
        };

        visual_order.clear();
        visual_order.extend(0..configured.len());
        visual_order.sort_by_key(|index| {
            (
                self.cols.get(*index).copied().unwrap_or_default().order,
                *index,
            )
        });

        let mut next_cell = 0usize;
        for &index in visual_order.iter() {
            let col = self.cols.get(index).copied().unwrap_or_default();
            let span = col.span_at(available_width, breakpoints);
            let offset = col.effective_offset(span);
            let responsive_units = self.visual.layout.responsive_units;
            let current_column = next_cell % responsive_units;
            if current_column + offset + span > responsive_units {
                next_cell = next_cell.div_ceil(responsive_units) * responsive_units;
            }
            let cell = next_cell + offset;
            configured[index].grid_cell = Some(cell);
            configured[index].grid_column_span = span as u32;
            next_cell = cell + span;
        }
    }
}
