use crate::draw::primitives::path::{PathBuilder, PathSegment};

#[test]
fn arc_uses_bounded_cubic_segments_and_preserves_endpoints() {
    let path = PathBuilder::new()
        .arc(10.0, 20.0, 5.0, 0.0, std::f32::consts::PI)
        .build();

    assert_eq!(path.segments().len(), 3);
    let PathSegment::MoveTo(start) = path.segments()[0] else {
        panic!("arc must start with MoveTo");
    };
    let PathSegment::CubicTo(_, _, end) = path.segments()[2] else {
        panic!("arc must end with CubicTo");
    };
    assert!((start.x - 15.0).abs() < 1e-5);
    assert!((start.y - 20.0).abs() < 1e-5);
    assert!((end.x - 5.0).abs() < 1e-5);
    assert!((end.y - 20.0).abs() < 1e-5);
}

#[test]
fn arc_rejects_invalid_geometry_and_caps_multi_turn_sweeps() {
    let invalid = PathBuilder::new().arc(0.0, 0.0, -1.0, 0.0, 1.0).build();
    let capped = PathBuilder::new()
        .arc(0.0, 0.0, 5.0, 0.0, std::f32::consts::TAU * 3.0)
        .build();

    assert!(invalid.is_empty());
    assert_eq!(capped.segments().len(), 6);
    assert!(matches!(capped.segments().last(), Some(PathSegment::Close)));
}
