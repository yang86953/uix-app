//! 从 WidgetTree 构建布局子项（System 私有边界契约的消费方）。
//!
//! # SMC 边界（SMC-04）
//!
//! 布局引擎（layout Module）保持对 widget 零依赖：从 WidgetTree 构建
//! `LayoutChild` 的辅助归 widget Module（本文件），widgets 经
//! `crate::ui::widget_runtime::tree_measure` 消费；依赖方向为
//! `widgets → widget → layout`。

use crate::core::{Constraints, WidgetId};
use crate::ui::layout::{AlignItems, FlexDirection, LayoutChild};
use crate::ui::widget_runtime::widget::{WidgetCore, WidgetTree};

// ── 辅助：从 WidgetTree 构建 LayoutChild ──────────────────────────

/// 从 WidgetTree 节点构建统一的 LayoutChild，并使用父级内容框约束测量。
pub(crate) fn child_from_tree_with_constraints(
    widget_id: WidgetId,
    tree: &WidgetTree,
    constraints: Constraints,
) -> LayoutChild {
    // 普通父布局读取组件首选 border-box 尺寸；Flex basis 是独立元数据。
    child_from_tree_with_measure_mode(widget_id, tree, constraints, MeasureMode::Preferred)
}

/// Flex 父级只在主轴使用组件的 basis 策略；交叉轴必须保留自然内容尺寸。
pub(crate) fn child_from_tree_with_flex_constraints(
    widget_id: WidgetId,
    tree: &WidgetTree,
    constraints: Constraints,
    parent_direction: FlexDirection,
) -> LayoutChild {
    let mut child = child_from_tree_with_constraints(widget_id, tree, constraints);
    child.flex_basis = tree
        .get(widget_id)
        .and_then(|node| node.as_layout())
        .and_then(|layout| layout.flex_basis(parent_direction));
    child
}

/// 从 WidgetTree 节点构建自然尺寸子项，忽略子组件的 Flex basis 归零策略。
pub(crate) fn child_from_tree_with_natural_constraints(
    widget_id: WidgetId,
    tree: &WidgetTree,
    constraints: Constraints,
) -> LayoutChild {
    // 固有尺寸父容器显式请求子树自然内容尺寸。
    child_from_tree_with_measure_mode(widget_id, tree, constraints, MeasureMode::Natural)
}

// 方向由父布局持有；不能用子组件内部排列方向推断其 flex 主轴。
#[derive(Clone, Copy)]
enum MeasureMode {
    Preferred,
    Natural,
}

/// 判断当前容器是否位于父 Row 的内容定高交叉轴（非 Stretch）。
/// 从已有直接子项定位容器身份，不扫描树，也不借具体组件的私有样式。
pub(crate) fn container_height_is_flex_cross_content(
    children: &[LayoutChild],
    tree: &WidgetTree,
) -> bool {
    let Some(node) = children
        .first()
        .and_then(|child| tree.get(child.id))
        .and_then(|child| child.parent())
        .and_then(|id| tree.get(id))
    else {
        return false;
    };
    let Some((direction, align)) = node
        .parent()
        .and_then(|id| tree.get(id))
        .and_then(|parent| parent.as_layout())
        .and_then(|layout| layout.flex_layout_axes())
    else {
        return false;
    };
    matches!(direction, FlexDirection::Row | FlexDirection::RowReverse)
        && node
            .as_layout()
            .and_then(|layout| layout.align_self())
            .unwrap_or(align)
            != AlignItems::Stretch
}

// 用单一分发路径构造 LayoutChild，避免普通测量与自然测量的元数据发生漂移。
fn child_from_tree_with_measure_mode(
    // 接收需要测量的稳定组件标识。
    widget_id: WidgetId,
    // 接收当前布局事实所属的组件树。
    tree: &WidgetTree,
    // 接收父级提供的尺寸约束。
    constraints: Constraints,
    // 指定组件首选尺寸或显式自然测量；都不覆盖 Flex basis。
    mode: MeasureMode,
) -> LayoutChild {
    let node = tree.get(widget_id);
    let layout = node.and_then(|widget| widget.as_layout());
    let grow = layout.map(|layout| layout.flex_grow()).unwrap_or(0.0);
    // 透明包装节点可在同一轮读取直接子测量，避免用上一帧缓存猜测固有尺寸。
    let pref = node
        // 优先请求组件明确声明的直接子节点代理测量。
        .and_then(|widget| widget.measure_from_children(constraints, tree))
        // 普通组件继续使用原有阶段无关测量入口。
        .or_else(|| {
            // 保留兼容组件显式自然测量入口，不再把 basis 混入尺寸。
            node.map(|widget| {
                // 根据调用方声明选择唯一测量语义。
                if matches!(mode, MeasureMode::Natural) {
                    // 固有尺寸容器需要子树真实内容尺寸。
                    widget.measure_natural(constraints)
                } else {
                    // 普通父布局读取组件首选 border-box 尺寸。
                    widget.measure(constraints)
                }
            })
        })
        // 节点已经失效时保持有限零尺寸。
        .unwrap_or_default();
    let shrink = layout.map(|layout| layout.flex_shrink()).unwrap_or(1.0);
    let margin = layout
        .map(|layout| layout.layout_margin())
        .unwrap_or_default();
    let align_self = layout.and_then(|layout| layout.align_self());
    let grid_cell = layout.and_then(|layout| layout.grid_cell());
    let grid_column_span = layout.map(|l| l.grid_column_span().max(1)).unwrap_or(1);
    let grid_row_span = layout.map(|l| l.grid_row_span().max(1)).unwrap_or(1);

    LayoutChild {
        id: widget_id,
        measured_size: pref,
        flex_basis: None,
        min_size: layout
            .map(|layout| layout.minimum_size())
            .unwrap_or_default(),
        flex_grow: grow,
        flex_shrink: shrink,
        margin,
        align_self,
        grid_cell,
        grid_column_span,
        grid_row_span,
    }
}
