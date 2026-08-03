use super::super::*;
use crate::core::{Constraints, Rect, Size};
use std::collections::HashSet;

#[cfg(test)]
use super::LAYOUT_TRACE_PHASE;

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
}

mod animate;
mod layout;
mod layout_children;

