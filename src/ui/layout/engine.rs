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
use crate::core::{ComponentId, Constraints, EdgeInsets, Rect, Size};
use crate::draw::spatial::AABB3D;
use crate::ui::core::widget::WidgetCore;
use crate::ui::core::widget::WidgetTree;

pub fn natural_measure_constraints() -> Constraints {
    Constraints::loose(Size::infinite())
}

fn frame_measure_constraints(frame: Rect) -> Option<Constraints> {
    (frame.w > 0.0 && frame.h > 0.0).then(|| Constraints::loose(Size::new(frame.w, frame.h)))
}

// ── 统一盒模型 ────────────────────────────────────────────────────

/// 统一盒模型 — 所有容器共享的 margin/border/padding 计算。
///
/// 与 Web CSS 盒模型一致：frame → 扣除 margin → 扣除 border → 扣除 padding → 内容区域。
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

    /// 从 frame 计算内框（扣除 margin + border + padding）。
    pub fn content_rect(&self, frame: Rect) -> Rect {
        let mh = self.margin.horizontal();
        let mv = self.margin.vertical();
        let bh = self.border_width.horizontal();
        let bv = self.border_width.vertical();
        Rect::new(
            frame.x + self.margin.left + self.border_width.left + self.padding.left,
            frame.y + self.margin.top + self.border_width.top + self.padding.top,
            (frame.w - mh - bh - self.padding.horizontal()).max(0.0),
            (frame.h - mv - bv - self.padding.vertical()).max(0.0),
        )
    }

    /// 从 frame 计算视觉区域（仅扣除 margin，供渲染用）。
    pub fn visual_rect(&self, frame: Rect) -> Rect {
        Rect::new(
            frame.x + self.margin.left,
            frame.y + self.margin.top,
            (frame.w - self.margin.horizontal()).max(0.0),
            (frame.h - self.margin.vertical()).max(0.0),
        )
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

// LayoutEngine trait 定义已迁移至 ui::traits::layout。
use crate::ui::traits::layout::LayoutEngine;

/// 布局引擎的输出：子节点位置 + 内容总尺寸。
#[derive(Debug, Clone)]
pub struct LayoutOutput {
    pub positions: Vec<Rect>,
    pub total_size: Size,
}

/// 3D 感知的布局输出：子节点位置（含 z 深度） + 内容总尺寸。
///
/// `z` 用于 3D 空间中的深度排序和透视变换。
/// 默认 z=0。
#[derive(Debug, Clone)]
pub struct LayoutOutput3D {
    pub positions: Vec<Rect>,
    pub z_values: Vec<f32>,
    pub total_size: Size,
}

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
        if children.is_empty() {
            return LayoutOutput {
                positions: Vec::new(),
                total_size: Size::new(content_rect.w, content_rect.h),
            };
        }

        // 溢出模式：使用简单流式堆叠
        if self.overflow_content {
            return overflow_layout(self, content_rect, children);
        }

        // 标准 FlexBox 模式
        let child_sizes: Vec<Size> = children.iter().map(|c| c.measured_size).collect();
        let child_margins: Vec<_> = children.iter().map(|c| c.margin).collect();

        let flex_children: Vec<FlexChild> = children
            .iter()
            .map(|c| FlexChild {
                flex_grow: c.flex_grow,
                flex_shrink: c.flex_shrink,
                align_self: c.align_self,
                ..FlexChild::default()
            })
            .collect();

        let input = FlexInput {
            direction: self.direction,
            wrap: self.wrap,
            gap: self.gap,
            padding: crate::core::EdgeInsets::zero(),
            container: content_rect,
            children: flex_children,
            child_sizes,
            child_margins,
            justify_content: self.justify,
            align_items: self.align,
        };

        let output = compute_flex_layout(&input);

        LayoutOutput {
            positions: output.child_rects,
            total_size: output.total_size,
        }
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
    let gap = engine.gap;

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

    let main_sizes: Vec<f32> = children
        .iter()
        .map(|c| {
            let pref = if is_row {
                c.measured_size.w
            } else {
                c.measured_size.h
            };
            pref.max(0.0)
        })
        .collect();

    let total_margin_main: f32 = children
        .iter()
        .map(|child| child.margin_main(engine.direction))
        .sum();
    let total_main: f32 =
        main_sizes.iter().sum::<f32>() + total_margin_main + gap * (count as f32 - 1.0).max(0.0);
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

    for i in 0..count {
        let main = main_sizes[i];
        let cross_size = if is_row {
            children[i].measured_size.h
        } else {
            children[i].measured_size.w
        };
        let margin_cross = if is_row {
            children[i].margin.vertical()
        } else {
            children[i].margin.horizontal()
        };

        let cross_align = children[i].align_self.unwrap_or(engine.align);
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
                content_rect.x + cursor + children[i].margin_start(engine.direction),
                content_rect.y + cross_offset + children[i].margin_cross_start(engine.direction),
                main,
                child_cross,
            )
        } else {
            (
                content_rect.x + cross_offset + children[i].margin_cross_start(engine.direction),
                content_rect.y + cursor + children[i].margin_start(engine.direction),
                child_cross,
                main,
            )
        };

        positions.push(Rect::new(x, y, w, h));

        let occupied_main = main + children[i].margin_main(engine.direction);
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
}

impl Default for GridLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutEngine for GridLayout {
    fn layout(&self, content_rect: Rect, children: &[LayoutChild]) -> LayoutOutput {
        if self.columns.is_empty() || children.is_empty() {
            return LayoutOutput {
                positions: Vec::new(),
                total_size: Size::new(content_rect.w, content_rect.h),
            };
        }

        // 自动计算行数
        let rows: Vec<GridTrack> = if self.rows.is_empty() {
            let n_cols = self.columns.len();
            let n_rows = children.len().div_ceil(n_cols);
            vec![GridTrack::Auto; n_rows.max(1)]
        } else {
            self.rows.clone()
        };

        let grid_children: Vec<GridChild> = children
            .iter()
            .map(|c| GridChild {
                cell: c.grid_cell,
                col_span: c.grid_column_span,
                row_span: c.grid_row_span,
                measured_size: c.measured_size,
                align: None,
                justify: None,
            })
            .collect();

        let output = compute_grid_layout(&GridInput {
            container: content_rect,
            columns: self.columns.clone(),
            rows,
            col_gap: self.col_gap,
            row_gap: self.row_gap,
            padding: crate::core::EdgeInsets::zero(),
            children: grid_children,
            align_items: self.align_items,
            justify_items: self.justify_items,
        });

        let positions = output
            .child_rects
            .into_iter()
            .zip(children.iter())
            .map(|(rect, child)| {
                Rect::new(
                    rect.x + child.margin.left,
                    rect.y + child.margin.top,
                    (rect.w - child.margin.horizontal()).max(0.0),
                    (rect.h - child.margin.vertical()).max(0.0),
                )
            })
            .collect();

        LayoutOutput {
            positions,
            total_size: output.total_size,
        }
    }
}

// ── 辅助：从 WidgetTree 构建 LayoutChild ──────────────────────────

/// 从 WidgetTree 节点构建统一的 LayoutChild。
///
/// 这是自然尺寸 fallback；生产布局路径应优先使用
/// [`child_from_tree_with_constraints`] 传入父级 content rect 约束。
pub fn child_from_tree(component_id: ComponentId, tree: &WidgetTree) -> LayoutChild {
    child_from_tree_with_constraints(
        component_id,
        tree,
        child_fallback_constraints(component_id, tree),
    )
}

fn child_fallback_constraints(component_id: ComponentId, tree: &WidgetTree) -> Constraints {
    let Some(node) = tree.get(component_id) else {
        return natural_measure_constraints();
    };

    node.parent()
        .and_then(|parent| tree.get(parent))
        .and_then(|parent| frame_measure_constraints(parent.frame()))
        .or_else(|| frame_measure_constraints(node.frame()))
        .unwrap_or_else(natural_measure_constraints)
}

/// 从 WidgetTree 节点构建统一的 LayoutChild，并使用父级内容框约束测量。
pub fn child_from_tree_with_constraints(
    component_id: ComponentId,
    tree: &WidgetTree,
    constraints: Constraints,
) -> LayoutChild {
    let node = tree.get(component_id);
    let pref = node.map(|c| c.measure(constraints)).unwrap_or_default();
    let h = if pref.h > 0.0 {
        pref.h
    } else {
        node.map(|c| c.frame().h).unwrap_or(0.0)
    };

    let grow = node
        .and_then(|c| c.as_layout())
        .map(|l| l.flex_grow())
        .unwrap_or(0.0);
    let shrink = node
        .and_then(|c| c.as_layout())
        .map(|l| l.flex_shrink())
        .unwrap_or(1.0);
    let margin = node
        .and_then(|c| c.as_layout())
        .map(|l| l.layout_margin())
        .unwrap_or_default();
    let align_self = node
        .and_then(|c| c.as_layout())
        .and_then(|l| l.align_self());
    let grid_cell = node.and_then(|c| c.as_layout()).and_then(|l| l.grid_cell());
    let grid_column_span = node
        .and_then(|c| c.as_layout())
        .map(|l| l.grid_column_span().max(1))
        .unwrap_or(1);
    let grid_row_span = node
        .and_then(|c| c.as_layout())
        .map(|l| l.grid_row_span().max(1))
        .unwrap_or(1);

    LayoutChild {
        id: component_id,
        measured_size: Size::new(pref.w, h),
        flex_grow: grow,
        flex_shrink: shrink,
        margin,
        align_self,
        grid_cell,
        grid_column_span,
        grid_row_span,
    }
}
