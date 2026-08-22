use super::super::*;
use crate::core::{Constraints, Rect, Size};
use std::collections::HashSet;

fn frame_constraints(frame: Rect) -> Constraints {
    Constraints::loose(Size::new(frame.w, frame.h))
}

#[derive(Default)]
pub(crate) struct LayoutTraversalScratch {
    roots: HashSet<WidgetId>,
    paths: HashSet<WidgetId>,
    stack: Vec<(WidgetId, bool)>,
}

#[derive(Clone, Copy)]
pub(crate) struct ShrinkOp {
    id: WidgetId,
    needed_h: f32,
}

/// 聚合同一次布局收敛内的 frame 变化，只为首态与终态计算视觉脏区。
#[derive(Default)]
pub(crate) struct LayoutFrameDamage {
    pub(crate) entries: Vec<(WidgetId, Option<Rect>)>,
    pub(crate) seen: HashSet<WidgetId>,
    pub(crate) roots: HashSet<WidgetId>,
    pub(crate) prepainted: HashSet<WidgetId>,
}

#[derive(Default)]
pub(crate) struct LayoutFrameScratch {
    order: Vec<WidgetId>,
    traversal: LayoutTraversalScratch,
    prev_expand_sig: Vec<(WidgetId, i32, i32, i32, i32)>,
    pass_expand_sig: Vec<(WidgetId, i32, i32, i32, i32)>,
    resized_children: HashSet<WidgetId>,
    visibility_changes: Vec<(WidgetId, bool)>,
    expand_children: Vec<WidgetId>,
    shrink_ops: Vec<ShrinkOp>,
    shrink_children: Vec<WidgetId>,
    shrink_parent_children: Vec<WidgetId>,
    layout_damage: LayoutFrameDamage,
    effective_visible: HashSet<WidgetId>,
}

mod animate;
mod layout;
mod layout_children;
// 统一求解正常流、包含块、根视口和 sticky 视觉偏移。
mod position;
