use super::super::*;
use crate::core::{Constraints, Rect, Size};
use crate::ui::{LayoutChild, LayoutEngineScratch};
use std::collections::{HashMap, HashSet};

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

/// 布局协调器跨容器复用的测量、定位与求解工作区。
#[derive(Default)]
pub(crate) struct LayoutArrangeScratch {
    pub(crate) visible_children: Vec<WidgetId>,
    pub(crate) measured: Vec<LayoutChild>,
    pub(crate) in_flow: Vec<LayoutChild>,
    pub(crate) out_of_flow: Vec<WidgetId>,
    pub(crate) positions: Vec<(WidgetId, Rect)>,
    pub(crate) engine: LayoutEngineScratch,
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
    arrange: LayoutArrangeScratch,
    subtree_bottoms: HashMap<WidgetId, (f32, f32)>,
    layout_damage: LayoutFrameDamage,
    effective_visible: HashSet<WidgetId>,
}

mod animate;
mod layout;
mod layout_children;
// 统一求解正常流、包含块、根视口和 sticky 视觉偏移。
mod position;
