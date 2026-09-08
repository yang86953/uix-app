//! 主轴分配后用最终宽度重测交叉轴；不改变已经确定的 flex basis。

use std::borrow::Cow;

use crate::core::{Constraints, Rect, Size};
use crate::ui::layout::LayoutChild;
use crate::ui::widget_runtime::tree_measure::child_from_tree_with_constraints;
use crate::ui::widget_runtime::widget::WidgetTree;

pub(super) fn row_children_at_allocated_width<'a>(
    children: &'a [LayoutChild],
    positions: &[Rect],
    tree: &WidgetTree,
    constraints: Constraints,
) -> Cow<'a, [LayoutChild]> {
    let mut result = Cow::Borrowed(children);
    for (index, (child, rect)) in children.iter().zip(positions).enumerate() {
        if !rect.w.is_finite() || (rect.w - child.measured_size.w).abs() <= 0.01 {
            continue;
        }
        let measured = child_from_tree_with_constraints(
            child.id,
            tree,
            Constraints::loose(Size::new(rect.w.max(0.0), constraints.max.h)),
        );
        if (measured.measured_size.h - child.measured_size.h).abs() > 0.01 {
            // 宽度仍使用第一轮 basis，避免 shrink 被重复分配；只更新依宽度变化的高度。
            result.to_mut()[index].measured_size.h = measured.measured_size.h;
        }
    }
    result
}
