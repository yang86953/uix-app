use super::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::traits::{WidgetCapabilities, WidgetLayout};

struct FixedChild(Size);

impl WidgetComponent for FixedChild {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(WidgetCapabilities::LAYOUT)
    }

    crate::wc_upcast!(FixedChild; WidgetLayout);
}

impl WidgetLayout for FixedChild {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.clamp(self.0)
    }

    fn flex_grow(&self) -> f32 {
        2.0
    }

    fn flex_shrink(&self) -> f32 {
        0.25
    }

    fn align_self(&self) -> Option<AlignItems> {
        Some(AlignItems::End)
    }
}

#[test]
fn measure_clamps_fixed_space_size() {
    let measured = Space::new()
        .width(80.0)
        .height(24.0)
        .measure(Constraints::loose(Size::new(40.0, 32.0)));

    assert_eq!(measured, Size::new(40.0, 24.0));
}

#[test]
fn measure_uses_cached_content_size_without_fixed_axes() {
    let space = Space::new();
    space.cached_content_size.set(Size::new(96.0, 40.0));

    let measured = space.measure(Constraints::unconstrained());
    assert_eq!(measured, Size::new(96.0, 40.0));
}

#[test]
fn measure_children_respects_axes_and_preserves_flex_metadata() {
    let mut tree = WidgetTree::new();
    let child = tree.set_root(Box::new(FixedChild(Size::new(120.0, 30.0))));
    let frame = Rect::new(0.0, 0.0, 40.0, 20.0);

    let row = Space::new().height(20.0).measure_children(frame, &[child], &tree);
    assert_eq!(row[0].measured_size, Size::new(120.0, 20.0));
    assert_eq!(row[0].flex_grow, 2.0);
    assert_eq!(row[0].flex_shrink, 0.25);
    assert_eq!(row[0].align_self, Some(AlignItems::End));

    let column = Space::new()
        .vertical()
        .width(40.0)
        .measure_children(frame, &[child], &tree);
    assert_eq!(column[0].measured_size, Size::new(40.0, 30.0));
}

#[test]
fn wrap_row_uses_frame_width_not_intrinsic_overflow() {
    // flow_row 场景：无固定宽 + wrap，窄 frame 下必须换行，而不是撑破父级。
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        Space::new()
            .height(56.0)
            .direction(FlexDirection::Row)
            .wrap(true)
            .align(AlignItems::Start)
            .child(FixedChild(Size::new(40.0, 20.0)))
            .child(FixedChild(Size::new(40.0, 20.0)))
            .child(FixedChild(Size::new(40.0, 20.0)))
            .child(FixedChild(Size::new(40.0, 20.0)))
            .child(FixedChild(Size::new(40.0, 20.0)))
            .child(FixedChild(Size::new(40.0, 20.0))),
    ));
    if let Some(node) = tree.get_mut(root) {
        // 两列才够：40+gap+40 ≈ 96，第三项必须换行。
        node.set_frame(Rect::new(0.0, 0.0, 100.0, 56.0));
    }
    tree.layout();

    let kids = tree.get(root).unwrap().children().to_vec();
    let frames: Vec<_> = kids
        .iter()
        .map(|&id| tree.get(id).unwrap().frame())
        .collect();
    let max_right = frames.iter().map(|f| f.x + f.w).fold(0.0f32, f32::max);
    let unique_ys = {
        let mut ys: Vec<i32> = frames.iter().map(|f| f.y.round() as i32).collect();
        ys.sort_unstable();
        ys.dedup();
        ys
    };
    assert!(
        unique_ys.len() > 1,
        "wrap row should use multiple lines under narrow frame, frames={frames:?}"
    );
    assert!(
        max_right <= 100.5,
        "wrapped children must stay within frame width, max_right={max_right}, frames={frames:?}"
    );
}

#[test]
fn nested_sample_block_bootstraps_from_zero_frame() {
    // 复现 showcase::sample_block：无固定尺寸的 Column Space 嵌在 Row 里。
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        Space::new()
            .height(48.0)
            .direction(FlexDirection::Row)
            .align(AlignItems::Start)
            .child(
                Space::new()
                    .vertical()
                    .align(AlignItems::Start)
                    .child(FixedChild(Size::new(40.0, 12.0)))
                    .child(FixedChild(Size::new(56.0, 14.0))),
            ),
    ));
    if let Some(node) = tree.get_mut(root) {
        node.set_frame(Rect::new(0.0, 0.0, 400.0, 48.0));
    }
    tree.layout();

    let sample = tree.get(root).unwrap().children()[0];
    let sample_frame = tree.get(sample).unwrap().frame();
    assert!(
        sample_frame.w > 0.0 && sample_frame.h > 0.0,
        "sample_block Space must expand from children, got {sample_frame:?}"
    );
    let kids = tree.get(sample).unwrap().children().to_vec();
    assert_eq!(kids.len(), 2);
    for kid in kids {
        let f = tree.get(kid).unwrap().frame();
        assert!(f.w > 0.0 && f.h > 0.0, "child collapsed: {f:?}");
    }
}

#[test]
fn layout_children_allow_main_axis_overflow_and_clamp_cross_axis() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        Space::new()
            .width(40.0)
            .height(20.0)
            .child(FixedChild(Size::new(120.0, 30.0))),
    ));

    tree.layout();

    let child = tree.get(root).unwrap().children()[0];
    assert_eq!(tree.get(child).unwrap().frame().w, 120.0);
    assert_eq!(tree.get(child).unwrap().frame().h, 20.0);
}

#[test]
fn layout_children_consumes_snapshot_instead_of_tree_measure_or_frame() {
    let mut tree = WidgetTree::new();
    let child = tree.set_root(Box::new(FixedChild(Size::new(12.0, 12.0))));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, -20.0, 120.0, 60.0));
    let snapshot = LayoutChild::new(child, Size::new(120.0, 0.0));

    let placements =
        Space::new().layout_children(Rect::new(0.0, 0.0, 40.0, 20.0), &[snapshot], &tree);

    assert_eq!(placements[0].1, Rect::new(0.0, 10.0, 120.0, 0.0));
}
