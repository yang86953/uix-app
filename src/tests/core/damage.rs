use crate::tests::common::*;
use crate::core::damage::*;

#[test]
fn for_paint_clear_keeps_single_rect() {
    let region = DirtyRegion::area(Rect::new(10.0, 20.0, 30.0, 40.0));
    let paint = region.for_paint_clear();
    assert_eq!(paint.rects(), &[Rect::new(10.0, 20.0, 30.0, 40.0)]);
    assert!(!paint.full_frame);
}

#[test]
fn for_paint_clear_unions_disjoint_rects() {
    let mut region = DirtyRegion::empty();
    region.add_rect(Rect::new(0.0, 0.0, 10.0, 10.0));
    region.add_rect(Rect::new(0.0, 90.0, 10.0, 10.0));
    let paint = region.for_paint_clear();
    assert_eq!(paint.rects().len(), 1);
    assert_eq!(paint.rects()[0], Rect::new(0.0, 0.0, 10.0, 100.0));
    // 并集覆盖中间空隙，子节点剪枝与清屏一致
    assert!(paint.intersects(Rect::new(0.0, 40.0, 10.0, 10.0)));
}
