use crate::app::window_driver::{graphics_failure_diagnostic, graphics_failure_is_error};
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

#[test]
fn graphics_failure_diagnostic_keeps_the_complete_typed_cause_chain() {
    let failure = GraphicsFailure::from_error(
        Error::new(Errc::GraphicsDeviceLost, "shared logical device is lost").with_source(
            Error::new(
                Errc::GraphicsDeviceLost,
                "vkQueueSubmit returned ERROR_DEVICE_LOST",
            )
            .with_source(Error::new(
                Errc::GraphicsDeviceLost,
                "VK_EXT_device_fault: page fault at 0x1234",
            )),
        ),
    );

    let diagnostic = graphics_failure_diagnostic(&failure);

    assert!(diagnostic.contains("shared logical device is lost"));
    assert!(diagnostic.contains("vkQueueSubmit returned ERROR_DEVICE_LOST"));
    assert!(diagnostic.contains("VK_EXT_device_fault: page fault at 0x1234"));
    assert_eq!(diagnostic.lines().count(), 3);
}
