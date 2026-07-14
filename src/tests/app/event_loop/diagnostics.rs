use crate::app::window_driver::graphics_failure_is_error;
use crate::core::{Errc, Error};
use crate::draw::engine::GraphicsFailure;

fn failure(code: Errc) -> GraphicsFailure {
    GraphicsFailure::from_error(Error::new(code, "injected graphics failure"))
}

#[test]
fn occlusion_is_availability_while_other_graphics_failures_are_errors() {
    assert!(!graphics_failure_is_error(&failure(Errc::GraphicsOccluded)));
    assert!(graphics_failure_is_error(&failure(
        Errc::GraphicsSurfaceLost
    )));
    assert!(graphics_failure_is_error(&failure(
        Errc::GraphicsDeviceLost
    )));
    assert!(graphics_failure_is_error(&failure(
        Errc::GraphicsOutOfMemory
    )));
    assert!(graphics_failure_is_error(&failure(Errc::PlatformError)));
}
