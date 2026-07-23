use uix::core::{Errc, Error, Point, Rect};

#[test]
fn core_geometry_and_error_chain_are_public_contracts() {
    let rect = Rect::new(10.0, 20.0, 30.0, 40.0);
    assert!(rect.contains(Point::new(25.0, 35.0)));

    let root = Error::new(Errc::IoError, "disk unavailable");
    let error = Error::new(Errc::InvalidState, "settings load failed").with_source(root);
    assert_eq!(error.depth(), 1);
    assert_eq!(error.root_cause().code(), Errc::IoError);
    assert!(error.what().contains("disk unavailable"));
}
