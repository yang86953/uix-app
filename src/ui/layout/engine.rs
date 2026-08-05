//! 统一布局引擎 — FlexBox 和 Grid 的组件化封装。
//!
//! 将原本的纯函数 compute_flex_layout / compute_grid_layout 重新设计为
//! 可被组合使用的布局组件，提供统一的 LayoutEngine trait 接口。
//!
//! 盒模型计算也统一在此层，所有容器组件通过 BoxModel 获得一致的内框计算。

use super::flex::compute_flex_layout;
use super::grid::compute_grid_layout;
use super::{AlignItems, FlexChild, FlexDirection, FlexInput, JustifyContent};
use super::{GridChild, GridInput, GridTrack};
use crate::core::{ComponentId, EdgeInsets, Rect, Size};
use crate::draw::geometry::spatial::AABB3D;

// ── 统一盒模型 ────────────────────────────────────────────────────

/// 统一盒模型 — 所有容器共享的 margin/border/padding 计算。
///
/// 父布局先消费 margin；本层从 border-box frame 扣除 border 与 padding 得到内容区域。
#[derive(Debug, Clone, Copy)]
pub struct BoxModel {
    pub margin: EdgeInsets,
    pub border_width: EdgeInsets,
    pub padding: EdgeInsets,
}

impl BoxModel {
    pub const ZERO: Self = Self {
        margin: EdgeInsets::zero(),
        border_width: EdgeInsets::zero(),
        padding: EdgeInsets::zero(),
    };

    /// 从子项 frame 计算内容区（扣除 border + padding）。
    ///
    /// `frame` 是父级 flex/grid 分配的 border-box：**不含 margin**
    ///（margin 已由父级在放置时计入间距）。此处再扣 margin 会双重缩进。
    pub fn content_rect(&self, frame: Rect) -> Rect {
        // 先把测量哨兵和非法 frame 收敛为可写入布局树的实际几何。
        let frame = normalize_layout_rect(frame);
        // border 只能是有限非负厚度。
        let border = normalize_non_negative_insets(self.border_width);
        // padding 只能是有限非负厚度。
        let padding = normalize_non_negative_insets(self.padding);
        // 横向边框总量也要防止有限大数相加溢出。
        let border_horizontal = finite_non_negative(border.horizontal());
        // 纵向边框总量也要防止有限大数相加溢出。
        let border_vertical = finite_non_negative(border.vertical());
        // 横向内边距总量也要防止有限大数相加溢出。
        let padding_horizontal = finite_non_negative(padding.horizontal());
        // 纵向内边距总量也要防止有限大数相加溢出。
        let padding_vertical = finite_non_negative(padding.vertical());
        // 计算后再次归一，阻止坐标加法或尺寸减法溢出到最终 frame。
        normalize_layout_rect(Rect::new(
            frame.x + border.left + padding.left,
            frame.y + border.top + padding.top,
            (frame.w - border_horizontal - padding_horizontal).max(0.0),
            (frame.h - border_vertical - padding_vertical).max(0.0),
        ))
    }

    /// 视觉区域 = border-box（与 frame 同；margin 在 frame 外由父级留白）。
    pub fn visual_rect(&self, frame: Rect) -> Rect {
        // 视觉入口同样不得重新物化无界哨兵或非有限 frame。
        normalize_layout_rect(frame)
    }
}

// ── 统一子节点信息 ─────────────────────────────────────────────────

/// 统一子节点布局信息，Flex 和 Grid 引擎共用。
///
/// grid_* 字段仅在 Grid 引擎中使用（Flex 引擎忽略）。
///
/// # Margin 参与布局
///
/// `margin` 占用了主轴和交叉轴的空间。布局引擎计算子节点位置时：
/// - 主轴方向：子节点的占位 = measured_size.main + margin.main_axis_sum
/// - 分配 frame 时：子节点起始位置 = cursor + margin.start (主轴方向)
/// - 分配 frame 时：交叉轴起始位置 = cross_offset + margin.cross_start
#[derive(Debug, Clone)]
pub struct LayoutChild {
    pub id: ComponentId,
    pub measured_size: Size,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    /// 外边距——参与布局计算，推开兄弟节点
    pub margin: crate::core::EdgeInsets,
    /// 子项交叉轴对齐覆盖；None 时继承父容器 align_items。
    pub align_self: Option<AlignItems>,
    /// Grid: 起始单元格索引
    pub grid_cell: Option<usize>,
    /// Grid: 列跨度
    pub grid_column_span: u32,
    /// Grid: 行跨度
    pub grid_row_span: u32,
}

impl LayoutChild {
    pub fn new(id: ComponentId, measured_size: Size) -> Self {
        Self {
            id,
            measured_size,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            margin: crate::core::EdgeInsets::zero(),
            align_self: None,
            grid_cell: None,
            grid_column_span: 1,
            grid_row_span: 1,
        }
    }

    pub fn with_flex(mut self, grow: f32, shrink: f32) -> Self {
        self.flex_grow = grow;
        self.flex_shrink = shrink;
        self
    }

    pub fn with_grid_cell(mut self, cell: usize) -> Self {
        self.grid_cell = Some(cell);
        self
    }

    pub fn with_grid_span(mut self, columns: u32, rows: u32) -> Self {
        self.grid_column_span = columns.max(1);
        self.grid_row_span = rows.max(1);
        self
    }

    /// 主轴方向的外边距总和（用于布局占位计算）。
    pub fn margin_main(&self, direction: FlexDirection) -> f32 {
        match direction {
            FlexDirection::Row | FlexDirection::RowReverse => self.margin.left + self.margin.right,
            FlexDirection::Column | FlexDirection::ColumnReverse => {
                self.margin.top + self.margin.bottom
            }
        }
    }

    /// 起点方向的外边距（用于布局时设置子节点起始位置）。
    pub fn margin_start(&self, direction: FlexDirection) -> f32 {
        match direction {
            FlexDirection::Row => self.margin.left,
            FlexDirection::RowReverse => self.margin.right,
            FlexDirection::Column => self.margin.top,
            FlexDirection::ColumnReverse => self.margin.bottom,
        }
    }

    /// 交叉轴起始方向的外边距。
    pub fn margin_cross_start(&self, direction: FlexDirection) -> f32 {
        match direction {
            FlexDirection::Row | FlexDirection::RowReverse => self.margin.top,
            FlexDirection::Column | FlexDirection::ColumnReverse => self.margin.left,
        }
    }
}

// ── 布局引擎 trait ─────────────────────────────────────────────────

/// 统一布局引擎 trait — Flex 和 Grid 的公共抽象。
pub trait LayoutEngine {
    /// 在内容区域内计算子节点位置。
    fn layout(&self, content_rect: Rect, children: &[LayoutChild]) -> LayoutOutput;
}

/// 布局引擎的输出：子节点位置 + 内容总尺寸。
#[derive(Debug, Clone)]
pub struct LayoutOutput {
    pub positions: Vec<Rect>,
    pub total_size: Size,
}

impl LayoutOutput {
    // 在共享引擎边界统一收敛最终 frame 与总尺寸。
    fn normalized(mut self) -> Self {
        // 逐项清除算法累加产生的非有限值或测量哨兵。
        for frame in &mut self.positions {
            // 保留有效几何，仅替换不能写入布局树的分量。
            *frame = normalize_layout_rect(*frame);
        }
        // 总尺寸也必须是有限非负的实际值。
        self.total_size = normalize_layout_size(self.total_size);
        // 返回保持子项数量和顺序不变的输出。
        self
    }
}

// 把坐标值限制为有限且不是 f32::MAX 测量哨兵的实际值。
fn finite_or_zero(value: f32) -> f32 {
    // f32::MAX 及其负值不能进入实际 frame，非有限值同样回退为零。
    if value.is_finite() && value.abs() < f32::MAX {
        // 有效坐标保留原值，允许布局溢出产生有限负位置。
        value
    } else {
        // 非法坐标采用稳定零值。
        0.0
    }
}

// 把尺寸值限制为有限、非负且不是无界哨兵的实际值。
fn finite_non_negative(value: f32) -> f32 {
    // 先清除非有限值和测量哨兵，再钳制负尺寸。
    finite_or_zero(value).max(0.0)
}

// 归一最终或待求解的矩形。
fn normalize_layout_rect(rect: Rect) -> Rect {
    // Rect::new 继续承担 NaN 安全构造，传入值已满足布局层更强契约。
    Rect::new(
        finite_or_zero(rect.x),
        finite_or_zero(rect.y),
        finite_non_negative(rect.w),
        finite_non_negative(rect.h),
    )
}

// 归一最终或待求解的尺寸。
fn normalize_layout_size(size: Size) -> Size {
    // 两个轴都不得把测量阶段的无界哨兵写入实际输出。
    Size::new(finite_non_negative(size.w), finite_non_negative(size.h))
}

// 归一允许负值语义的外边距。
fn normalize_margin(insets: EdgeInsets) -> EdgeInsets {
    // 负外边距保持既有语义，只清除非有限值和无界哨兵。
    EdgeInsets::new(
        finite_or_zero(insets.left),
        finite_or_zero(insets.top),
        finite_or_zero(insets.right),
        finite_or_zero(insets.bottom),
    )
}

// 归一不允许负值语义的 border/padding。
fn normalize_non_negative_insets(insets: EdgeInsets) -> EdgeInsets {
    // 各边独立钳制，避免一条非法边污染另一轴的内容尺寸。
    EdgeInsets::new(
        finite_non_negative(insets.left),
        finite_non_negative(insets.top),
        finite_non_negative(insets.right),
        finite_non_negative(insets.bottom),
    )
}

/// 3D 感知的布局输出：子节点位置（含 z 深度） + 内容总尺寸。
///
/// `z` 用于 3D 空间中的深度排序和透视变换。
/// 默认 z=0。
// 原公开面遗留（SMC-04 模块收口后无内部消费方；保留供外部集成）。
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct LayoutOutput3D {
    pub positions: Vec<Rect>,
    pub z_values: Vec<f32>,
    pub total_size: Size,
}

// 原公开面遗留（SMC-04 模块收口后无内部消费方；保留供外部集成）。
#[allow(dead_code)]
impl LayoutOutput3D {
    /// 从 LayoutOutput + z 值创建。
    pub fn from_2d(output: &LayoutOutput, z: f32) -> Self {
        let count = output.positions.len();
        Self {
            positions: output.positions.clone(),
            z_values: vec![z; count],
            total_size: output.total_size,
        }
    }

    /// 每个子节点独立 z 值。
    pub fn from_2d_zipped(output: &LayoutOutput, z_values: Vec<f32>) -> Self {
        Self {
            positions: output.positions.clone(),
            z_values,
            total_size: output.total_size,
        }
    }

    /// 转换为 AABB3D 列表（所有盒子 d=0）。
    pub fn to_aabbs(&self) -> Vec<AABB3D> {
        self.positions
            .iter()
            .zip(self.z_values.iter())
            .map(|(r, z)| AABB3D::from_rect_z(r.x, r.y, r.w, r.h, *z, 0.0))
            .collect()
    }
}

// ── FlexBox 布局引擎 ───────────────────────────────────────────────

/// FlexBox 布局引擎组件。
///
/// 封装 FlexInput 构建和 compute_flex_layout 调用，
/// 可被任何容器 widget 组合使用。
#[derive(Debug, Clone)]
pub struct FlexLayout {
    pub direction: FlexDirection,
    pub gap: f32,
    pub justify: JustifyContent,
    pub align: AlignItems,
    pub wrap: bool,
    /// 允许内容溢出（跳过 flex-shrink，子节点按自然尺寸流式堆叠）
    pub overflow_content: bool,
    /// 主轴无显式尺寸时由子项撑开，仍走标准 flex-grow 路径。
    pub intrinsic_main: bool,
}

impl FlexLayout {
    pub fn new() -> Self {
        Self {
            direction: FlexDirection::Column,
            gap: 0.0,
            justify: JustifyContent::Start,
            align: AlignItems::Stretch,
            wrap: false,
            overflow_content: false,
            intrinsic_main: false,
        }
    }

    /// 便捷构造：垂直堆叠。
    pub fn column() -> Self {
        Self::new()
    }

    /// 便捷构造：水平排列。
    pub fn row() -> Self {
        Self {
            direction: FlexDirection::Row,
            ..Self::new()
        }
    }

    pub fn with_direction(mut self, d: FlexDirection) -> Self {
        self.direction = d;
        self
    }
    pub fn with_gap(mut self, g: f32) -> Self {
        self.gap = g;
        self
    }
    pub fn with_justify(mut self, j: JustifyContent) -> Self {
        self.justify = j;
        self
    }
    pub fn with_align(mut self, a: AlignItems) -> Self {
        self.align = a;
        self
    }
}

impl Default for FlexLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutEngine for FlexLayout {
    fn layout(&self, content_rect: Rect, children: &[LayoutChild]) -> LayoutOutput {
        // 布局求解前先把测量阶段的无界矩形转为实际有限输入。
        let content_rect = normalize_layout_rect(content_rect);
        if children.is_empty() {
            // 空子集也必须经过统一输出收敛，不能直传父级哨兵。
            return LayoutOutput {
                positions: Vec::new(),
                total_size: Size::new(content_rect.w, content_rect.h),
            }
            .normalized();
        }

        // 溢出模式：使用简单流式堆叠
        if self.overflow_content {
            // 流式路径与标准路径共享同一最终几何契约。
            return overflow_layout(self, content_rect, children).normalized();
        }

        // 标准 FlexBox 模式
        let flex_children: Vec<FlexChild> = children
            .iter()
            .map(|c| FlexChild {
                // 弹性因子只接受有限非负实际值。
                flex_grow: finite_non_negative(c.flex_grow),
                // 压缩因子只接受有限非负实际值。
                flex_shrink: finite_non_negative(c.flex_shrink),
                align_self: c.align_self,
                // 子项测量哨兵不能进入最终求解算术。
                measured_size: normalize_layout_size(c.measured_size),
                // 外边距保留有限负值语义并清除非法分量。
                margin: normalize_margin(c.margin),
                ..FlexChild::default()
            })
            .collect();

        let input = FlexInput {
            direction: self.direction,
            wrap: self.wrap,
            // gap 保留既有有限负值语义，但清除非有限值和哨兵。
            gap: finite_or_zero(self.gap),
            padding: crate::core::EdgeInsets::zero(),
            container: content_rect,
            children: &flex_children,
            justify_content: self.justify,
            align_items: self.align,
            intrinsic_main: self.intrinsic_main,
        };

        let output = compute_flex_layout(&input);

        // 标准求解结果在公开边界执行最终有限化。
        LayoutOutput {
            positions: output.child_rects,
            total_size: output.total_size,
        }
        .normalized()
    }
}

/// 溢出模式：流式堆叠（不压缩，不分配剩余空间）。
fn overflow_layout(
    engine: &FlexLayout,
    content_rect: Rect,
    children: &[LayoutChild],
) -> LayoutOutput {
    let is_row = matches!(
        engine.direction,
        FlexDirection::Row | FlexDirection::RowReverse
    );
    let is_reverse = matches!(
        engine.direction,
        FlexDirection::RowReverse | FlexDirection::ColumnReverse
    );
    let count = children.len();
    let gap = finite_or_zero(engine.gap);

    let container_main = if is_row {
        content_rect.w
    } else {
        content_rect.h
    };
    let container_cross = if is_row {
        content_rect.h
    } else {
        content_rect.w
    };

    let total_margin_main: f32 = children
        .iter()
        .map(|child| finite_or_zero(child.margin_main(engine.direction)))
        .sum();
    let total_main: f32 = children
        .iter()
        .map(|child| {
            finite_non_negative(if is_row {
                child.measured_size.w
            } else {
                child.measured_size.h
            })
        })
        .sum::<f32>()
        + total_margin_main
        + gap * (count as f32 - 1.0).max(0.0);
    let total_size = if is_row {
        Size::new(total_main, container_cross)
    } else {
        Size::new(container_cross, total_main)
    };

    let mut cursor = if is_reverse {
        container_main - total_main
    } else {
        0.0
    };
    let mut positions = Vec::with_capacity(count);

    for child in children {
        let main = finite_non_negative(if is_row {
            child.measured_size.w
        } else {
            child.measured_size.h
        });
        let cross_size = if is_row {
            finite_non_negative(child.measured_size.h)
        } else {
            finite_non_negative(child.measured_size.w)
        };
        let margin_cross = if is_row {
            finite_or_zero(child.margin.vertical())
        } else {
            finite_or_zero(child.margin.horizontal())
        };

        let cross_align = child.align_self.unwrap_or(engine.align);
        let child_cross = if cross_align == AlignItems::Stretch {
            (container_cross - margin_cross).max(0.0)
        } else {
            cross_size
        };

        let cross_offset = match cross_align {
            AlignItems::Start => 0.0,
            AlignItems::Center => (container_cross - child_cross) / 2.0,
            AlignItems::End => container_cross - child_cross,
            AlignItems::Stretch => 0.0,
        };

        let (x, y, w, h) = if is_row {
            (
                content_rect.x + cursor + finite_or_zero(child.margin_start(engine.direction)),
                content_rect.y
                    + cross_offset
                    + finite_or_zero(child.margin_cross_start(engine.direction)),
                main,
                child_cross,
            )
        } else {
            (
                content_rect.x
                    + cross_offset
                    + finite_or_zero(child.margin_cross_start(engine.direction)),
                content_rect.y + cursor + finite_or_zero(child.margin_start(engine.direction)),
                child_cross,
                main,
            )
        };

        positions.push(Rect::new(x, y, w, h));

        let occupied_main = main + finite_or_zero(child.margin_main(engine.direction));
        if is_reverse {
            cursor -= occupied_main + gap;
        } else {
            cursor += occupied_main + gap;
        }
    }

    LayoutOutput {
        positions,
        total_size,
    }
}

// ── Grid 布局引擎 ─────────────────────────────────────────────────

/// Grid 布局引擎组件。
///
/// 封装 GridInput 构建和 compute_grid_layout 调用，
/// 可被任何容器 widget 组合使用。
#[derive(Debug, Clone)]
pub struct GridLayout {
    pub columns: Vec<GridTrack>,
    pub rows: Vec<GridTrack>,
    pub col_gap: f32,
    pub row_gap: f32,
    pub align_items: AlignItems,
    pub justify_items: JustifyContent,
}

impl GridLayout {
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            col_gap: 0.0,
            row_gap: 0.0,
            align_items: AlignItems::Stretch,
            justify_items: JustifyContent::Start,
        }
    }

    pub fn with_columns(mut self, cols: Vec<GridTrack>) -> Self {
        self.columns = cols;
        self
    }
    pub fn with_rows(mut self, rows: Vec<GridTrack>) -> Self {
        self.rows = rows;
        self
    }
    pub fn with_gap(mut self, col_gap: f32, row_gap: f32) -> Self {
        self.col_gap = col_gap;
        self.row_gap = row_gap;
        self
    }
    pub fn with_align(mut self, a: AlignItems) -> Self {
        self.align_items = a;
        self
    }
    pub fn with_justify(mut self, j: JustifyContent) -> Self {
        self.justify_items = j;
        self
    }

    pub(crate) fn layout_with_tracks(
        &self,
        content_rect: Rect,
        columns: &[GridTrack],
        rows: &[GridTrack],
        children: &[LayoutChild],
    ) -> LayoutOutput {
        // Grid 与 Flex 共享同一实际 frame 输入边界。
        let content_rect = normalize_layout_rect(content_rect);
        if columns.is_empty() || children.is_empty() {
            // 空 track 或空子集不能把父级无界哨兵直传到输出。
            return LayoutOutput {
                positions: Vec::new(),
                total_size: Size::new(content_rect.w, content_rect.h),
            }
            .normalized();
        }

        let grid_children: Vec<GridChild> = children
            .iter()
            .map(|c| GridChild {
                cell: c.grid_cell,
                col_span: c.grid_column_span,
                row_span: c.grid_row_span,
                // 子项测量哨兵不能进入 cell 尺寸与对齐算术。
                measured_size: normalize_layout_size(c.measured_size),
                // 外边距保留有限负值语义并清除非法分量。
                margin: normalize_margin(c.margin),
                align: None,
                justify: None,
            })
            .collect();

        let output = compute_grid_layout(&GridInput {
            container: content_rect,
            // 纯求解器逐值归一 track，保持普通布局继续借用列定义。
            columns,
            // 纯求解器只复制有界窗口内的显式行，并按需追加隐式 Auto 行。
            rows,
            // Grid gap 语义为非负距离。
            col_gap: finite_non_negative(self.col_gap),
            // Grid gap 语义为非负距离。
            row_gap: finite_non_negative(self.row_gap),
            padding: crate::core::EdgeInsets::zero(),
            children: &grid_children,
            align_items: self.align_items,
            justify_items: self.justify_items,
        });

        // Grid 求解结果在公开边界执行最终有限化。
        LayoutOutput {
            positions: output.child_rects,
            total_size: output.total_size,
        }
        .normalized()
    }
}

impl Default for GridLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutEngine for GridLayout {
    fn layout(&self, content_rect: Rect, children: &[LayoutChild]) -> LayoutOutput {
        self.layout_with_tracks(content_rect, &self.columns, &self.rows, children)
    }
}
