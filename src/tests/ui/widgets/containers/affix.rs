use crate::tests::common::*;
use crate::ui::widgets::containers::affix::*;
use crate::ui::widgets::{Label, ScrollView};

#[test]
fn measure_clamps_affixed_placeholder_height() {
    let mut affix = Affix::new(12.0);
    affix.set_child_bounds(0.0, 80.0);
    affix.update_scroll(20.0);

    let measured = affix.measure(Constraints::loose(Size::new(120.0, 32.0)));

    assert_eq!(measured, Size::new(0.0, 32.0));
}

#[test]
fn affix_uses_natural_position_as_sticky_threshold() {
    let mut affix = Affix::new(12.0);
    affix.set_child_bounds(100.0, 24.0);

    assert!(!affix.update_scroll(80.0));
    assert!(!affix.is_affixed());
    assert_eq!(affix.child_y(), 20.0);

    assert!(affix.update_scroll(96.0));
    assert!(affix.is_affixed());
    assert_eq!(affix.child_y(), 12.0);
}

#[test]
fn affix_arranges_real_child_at_viewport_top_after_scroll() {
    let mut tree = WidgetTree::new();
    let viewport = tree.set_root(Box::new(
        ScrollView::new(ScrollDirection::Vertical).scroll_to(0.0, 100.0),
    ));
    let affix = tree.add_child(viewport, Box::new(Affix::new(10.0).scroll_y(100.0)));
    let child = tree.add_child(affix, Box::new(Label::new("sticky")));
    assert!(!tree
        .get(affix)
        .expect("affix")
        .child_overflow_expands_parent());
    assert!(tree
        .get(child)
        .expect("ordinary child")
        .child_overflow_expands_parent());
    tree.get_mut(viewport)
        .expect("viewport")
        .set_frame(Rect::new(0.0, 0.0, 200.0, 100.0));
    tree.get_mut(affix)
        .expect("affix")
        .set_frame(Rect::new(0.0, 80.0, 200.0, 24.0));

    let positions = tree.get(affix).expect("affix").layout_children(
        Rect::new(0.0, 80.0, 200.0, 24.0),
        &[child],
        &tree,
    );
    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0].0, child);
    assert_eq!(positions[0].1.x, 0.0);
    assert_eq!(positions[0].1.y, 110.0);
    assert_eq!(positions[0].1.w, 200.0);
    assert!(positions[0].1.h > 0.0);
    tree.get_mut(child)
        .expect("child")
        .set_frame(positions[0].1);

    let visible = tree
        .visible_rect_for(child)
        .expect("sticky child is visible");
    assert!((visible.y - 10.0).abs() < 0.01);
    assert!(tree
        .get(affix)
        .expect("affix")
        .component()
        .as_any()
        .downcast_ref::<Affix>()
        .expect("affix component")
        .is_affixed());
}
