use crate::core::{Errc, Error};

fn nested_error() -> Error {
    Error::with_location(Errc::InvalidState, "outer", "outer.rs", 11).with_source(
        Error::with_location(Errc::PlatformError, "middle", "middle.rs", 22).with_source(
            Error::with_location(Errc::GraphicsDeviceLost, "device fault", "root.rs", 33),
        ),
    )
}

#[test]
fn full_error_text_renders_every_cause_in_order() {
    let text = nested_error().what();

    let outer = text.find("outer (outer.rs:11)").expect("outer error");
    let middle = text.find("middle (middle.rs:22)").expect("middle cause");
    let root = text.find("device fault (root.rs:33)").expect("root cause");
    assert!(outer < middle && middle < root);
    assert!(text.contains("\n  cause: [platform_error] middle"));
    assert!(text.contains("\n    cause: [graphics_device_lost] device fault"));
}

#[test]
fn display_uses_the_complete_error_chain() {
    let error = nested_error();

    assert_eq!(error.to_string(), error.what());
    assert!(error.to_string().contains("device fault (root.rs:33)"));
}

#[test]
fn short_error_text_remains_single_line() {
    let text = nested_error().short_what();

    assert!(!text.contains('\n'));
    assert!(!text.contains("middle"));
}
