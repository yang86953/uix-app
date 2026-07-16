use crate::draw::Transform;
use crate::tests::common::*;

#[test]
fn affine_concat_applies_right_hand_transform_first() {
    let transform = Transform::translate(10.0, 20.0).concat(Transform::scale(2.0, 3.0));

    assert_eq!(
        transform.transform_point(Point::new(4.0, 5.0)),
        Point::new(18.0, 35.0)
    );
    assert_eq!(
        transform.transform_rect(Rect::new(1.0, 2.0, 3.0, 4.0)),
        Rect::new(12.0, 26.0, 6.0, 12.0)
    );
}

#[test]
fn affine_inverse_roundtrips_points_and_rejects_singular_matrices() {
    let transform = Transform::translate(-7.0, 11.0).concat(Transform::scale(1.5, 0.5));
    let inverse = transform.inverse().expect("invertible transform");
    let point = Point::new(8.0, -3.0);
    let restored = inverse.transform_point(transform.transform_point(point));

    assert!((restored.x - point.x).abs() < 1e-5);
    assert!((restored.y - point.y).abs() < 1e-5);
    assert!(Transform::scale(0.0, 1.0).inverse().is_none());
}
