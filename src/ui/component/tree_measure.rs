//! 从 WidgetTree 构建布局子项（System 私有边界契约的消费方）。
//!
//! # SMC 边界（SMC-04）
//!
//! 布局引擎（layout Module）保持对 component 零依赖：从 WidgetTree 构建
//! `LayoutChild` 的辅助归 component Module（本文件），widgets 经
//! `crate::ui::component::tree_measure` 消费；依赖方向为
//! `widgets → component → layout`。

use crate::core::{ComponentId, Constraints};
use crate::ui::component::widget::WidgetTree;
use crate::ui::layout::LayoutChild;

// ── 辅助：从 WidgetTree 构建 LayoutChild ──────────────────────────

/// 从 WidgetTree 节点构建统一的 LayoutChild，并使用父级内容框约束测量。
pub(crate) fn child_from_tree_with_constraints(
    component_id: ComponentId,
    tree: &WidgetTree,
    constraints: Constraints,
) -> LayoutChild {
    let node = tree.get(component_id);
    // 透明包装节点可在同一轮读取直接子测量，避免用上一帧缓存猜测固有尺寸。
    let pref = node
        // 优先请求组件明确声明的直接子节点代理测量。
        .and_then(|component| component.measure_from_children(constraints, tree))
        // 普通组件继续使用原有阶段无关测量入口。
        .or_else(|| node.map(|component| component.measure(constraints)))
        // 节点已经失效时保持有限零尺寸。
        .unwrap_or_default();
    let layout = node.and_then(|component| component.as_layout());
    let grow = layout.map(|layout| layout.flex_grow()).unwrap_or(0.0);
    let shrink = layout.map(|layout| layout.flex_shrink()).unwrap_or(1.0);
    let margin = layout
        .map(|layout| layout.layout_margin())
        .unwrap_or_default();
    let align_self = layout.and_then(|layout| layout.align_self());
    let grid_cell = layout.and_then(|layout| layout.grid_cell());
    let grid_column_span = layout.map(|l| l.grid_column_span().max(1)).unwrap_or(1);
    let grid_row_span = layout.map(|l| l.grid_row_span().max(1)).unwrap_or(1);

    LayoutChild {
        id: component_id,
        measured_size: pref,
        flex_grow: grow,
        flex_shrink: shrink,
        margin,
        align_self,
        grid_cell,
        grid_column_span,
        grid_row_span,
    }
}
