use uix::core::{Point, Rect};
use uix::draw::{Color, PathBuilder, PathSegment};

#[test]
fn draw_color_contract_preserves_channels_and_alpha() {
    let color = Color::hex("#33669980");
    assert_eq!(color, Color::from_rgba(0x33, 0x66, 0x99, 0x80));
    assert_eq!(color.with_alpha(0xff).a, 0xff);
    assert_eq!(Color::RED.mix(&Color::BLUE, 0.0), Color::RED);
}

#[test]
fn draw_path_contract_exposes_geometry_without_renderer_internals() {
    let mut builder = PathBuilder::new();
    builder.polygon(&[
        Point::new(10.0, 20.0),
        Point::new(40.0, 20.0),
        Point::new(40.0, 60.0),
    ]);
    let path = builder.build();

    assert!(!path.is_empty());
    assert_eq!(path.bounds(), Some(Rect::new(10.0, 20.0, 30.0, 40.0)));
    assert!(matches!(path.segments().last(), Some(PathSegment::Close)));

    let translated = path.translated(5.0, -5.0);
    assert_eq!(translated.bounds(), Some(Rect::new(15.0, 15.0, 30.0, 40.0)));
}
