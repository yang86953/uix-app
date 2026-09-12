//! 从 WidgetTree 构建布局子项（System 私有边界契约的消费方）。
//!
//! # SMC 边界（SMC-04）
//!
//! 布局引擎（layout Module）保持对 widget 零依赖：从 WidgetTree 构建
//! `LayoutChild` 的辅助归 widget Module（本文件），widgets 经
//! `crate::ui::widget_runtime::tree_measure` 消费；依赖方向为
//! `widgets → widget → layout`。
//!
//! # 尺寸约束
//!
//! 子项的 min/max 约束先取组件内核样式声明，未声明项回退到声明节点
//! 元数据；百分比按调用方给出的父内容盒参照轴解析，参照未定的百分比
//! 视为不约束。解析结果同时钳制测量尺寸并写入子项上下限。

use crate::core::{Constraints, Rect, Size, WidgetId};
use crate::ui::layout::{AlignItems, FlexDirection, LayoutChild};
use crate::ui::theme::style::{PercentReference, SizeConstraints};
use crate::ui::widget_runtime::widget::{WidgetCore, WidgetTree};

// ── 辅助：从 WidgetTree 构建 LayoutChild ──────────────────────────

/// 从 WidgetTree 节点构建统一的 LayoutChild，并使用父级内容框约束测量。
///
/// 百分比约束没有参照轴，视为不约束；父级知道内容盒时应使用
/// [`child_from_tree_with_constraints_in`]。
pub fn child_from_tree_with_constraints(
    widget_id: WidgetId,
    tree: &WidgetTree,
    constraints: Constraints,
) -> LayoutChild {
    child_from_tree_with_constraints_in(widget_id, tree, constraints, PercentReference::NONE)
}

/// 同 [`child_from_tree_with_constraints`]，并给出父内容盒作为百分比参照。
pub fn child_from_tree_with_constraints_in(
    widget_id: WidgetId,
    tree: &WidgetTree,
    constraints: Constraints,
    reference: PercentReference,
) -> LayoutChild {
    // 普通父布局读取组件首选 border-box 尺寸；Flex basis 是独立元数据。
    child_from_tree_with_measure_mode(
        widget_id,
        tree,
        constraints,
        MeasureMode::Preferred,
        reference,
    )
}

/// Flex 父级只在主轴使用组件的 basis 策略；交叉轴必须保留自然内容尺寸。
///
/// Flex 父级总能给出自身内容盒，因此直接要求百分比参照。
pub fn child_from_tree_with_flex_constraints(
    widget_id: WidgetId,
    tree: &WidgetTree,
    constraints: Constraints,
    parent_direction: FlexDirection,
    reference: PercentReference,
) -> LayoutChild {
    let mut child = child_from_tree_with_constraints_in(widget_id, tree, constraints, reference);
    child.flex_basis = tree
        .get(widget_id)
        .and_then(|node| node.as_layout())
        .and_then(|layout| layout.flex_basis(parent_direction));
    child
}

/// 从 WidgetTree 节点构建自然尺寸子项，忽略子组件的 Flex basis 归零策略。
pub fn child_from_tree_with_natural_constraints(
    widget_id: WidgetId,
    tree: &WidgetTree,
    constraints: Constraints,
) -> LayoutChild {
    child_from_tree_with_natural_constraints_in(
        widget_id,
        tree,
        constraints,
        PercentReference::NONE,
    )
}

/// 同 [`child_from_tree_with_natural_constraints`]，并给出百分比参照。
pub fn child_from_tree_with_natural_constraints_in(
    widget_id: WidgetId,
    tree: &WidgetTree,
    constraints: Constraints,
    reference: PercentReference,
) -> LayoutChild {
    // 固有尺寸父容器显式请求子树自然内容尺寸。
    child_from_tree_with_measure_mode(
        widget_id,
        tree,
        constraints,
        MeasureMode::Natural,
        reference,
    )
}

// 方向由父布局持有；不能用子组件内部排列方向推断其 flex 主轴。
#[derive(Clone, Copy)]
enum MeasureMode {
    Preferred,
    Natural,
}

/// 判断当前容器是否位于父 Row 的内容定高交叉轴（非 Stretch）。
/// 从已有直接子项定位容器身份，不扫描树，也不借具体组件的私有样式。
pub fn container_height_is_flex_cross_content(
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

/// 决定容器内容盒的哪些轴可作为子项百分比约束的参照。
///
/// 规则（可解释、不迭代）：某轴“确定”当且仅当容器声明了显式尺寸、容器是根节点，
/// 或该轴由父 Flex 强加（主轴 `flex_grow > 0`、交叉轴有效对齐为 Stretch）且父级
/// 同轴自身确定——沿祖先链用既有 phase2 显式尺寸锁与 Flex 轴事实递推，深度有界。
/// 由内容撑开的轴（含经 Stretch/grow 间接依赖内容撑开祖先的轴）不作参照，百分比
/// 不会通过内容尺寸反馈形成自引用。
///
/// 参照值取父级本轮已分配的 `content_rect`：布局自上而下，容器排列子项时其 frame
/// 已由父级分配，因此零、1px 或任意小的内容盒都是真实参照，不设数值阈值。
pub fn percent_reference_for_children(
    first_child: Option<WidgetId>,
    tree: &WidgetTree,
    content_rect: Rect,
    explicit: (bool, bool),
) -> PercentReference {
    let Some(container) = first_child
        .and_then(|child| tree.get(child))
        .and_then(|child| child.parent())
    else {
        return PercentReference::NONE;
    };
    let axis = |definite: bool, content: f32| {
        (definite && content.is_finite()).then_some(content.max(0.0))
    };
    PercentReference::new(
        axis(
            axis_definite(tree, container, true, explicit.0),
            content_rect.w,
        ),
        axis(
            axis_definite(tree, container, false, explicit.1),
            content_rect.h,
        ),
    )
}

// 判断节点在某轴上的尺寸是否不依赖自身内容：显式声明、根节点，或父 Flex 强加且父级同轴确定。
fn axis_definite(tree: &WidgetTree, id: WidgetId, horizontal: bool, own_explicit: bool) -> bool {
    if own_explicit {
        return true;
    }
    let Some(node) = tree.get(id) else {
        return false;
    };
    let Some(parent_id) = node.parent() else {
        // 根 frame 由窗口客户区提供。
        return true;
    };
    let Some(parent) = tree.get(parent_id) else {
        return false;
    };
    let Some((direction, align)) = parent
        .as_layout()
        .and_then(|layout| layout.flex_layout_axes())
    else {
        // 非 Flex 父级（如 Grid 单元）不强加尺寸。
        return false;
    };
    let own = node.as_layout();
    let grows = own.map(|layout| layout.flex_grow() > 0.0).unwrap_or(false);
    let stretched =
        own.and_then(|layout| layout.align_self()).unwrap_or(align) == AlignItems::Stretch;
    let imposed = match direction {
        FlexDirection::Row | FlexDirection::RowReverse => {
            if horizontal {
                grows
            } else {
                stretched
            }
        }
        FlexDirection::Column | FlexDirection::ColumnReverse => {
            if horizontal {
                stretched
            } else {
                grows
            }
        }
    };
    if !imposed {
        return false;
    }
    // 强加轴只有在父级同轴自身确定时才确定；父级显式锁复用 phase2 既有事实。
    let (parent_width_locked, parent_height_locked) = tree.phase2_explicit_size_locks(parent_id);
    axis_definite(
        tree,
        parent_id,
        horizontal,
        if horizontal {
            parent_width_locked
        } else {
            parent_height_locked
        },
    )
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
    // 接收百分比约束的父内容盒参照轴。
    reference: PercentReference,
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
    // 内核样式声明优先，未声明项回退到声明节点交付的约束。
    let declared = layout
        .map(|layout| layout.size_constraints())
        .unwrap_or(SizeConstraints::NONE)
        .or(node
            .map(|widget| widget.size_constraints())
            .unwrap_or(SizeConstraints::NONE));
    let bounds = declared.resolve(reference);
    let intrinsic_minimum = layout
        .map(|layout| layout.minimum_size())
        .unwrap_or_default();
    // 无约束时保持既有测量结果原样，不改变旧路径行为。
    let measured_size = if declared.is_none() {
        pref
    } else {
        bounds.clamp(pref)
    };

    LayoutChild {
        id: widget_id,
        measured_size,
        flex_basis: None,
        min_size: Size::new(
            intrinsic_minimum.w.max(bounds.min.w),
            intrinsic_minimum.h.max(bounds.min.h),
        ),
        max_size: bounds.max,
        flex_grow: grow,
        flex_shrink: shrink,
        margin,
        align_self,
        grid_cell,
        grid_column_span,
        grid_row_span,
    }
}
