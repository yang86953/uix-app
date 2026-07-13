use crate::tests::common::*;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::thread::{self, ThreadId};
use crate::core::{ Result };
use crate::native::traits::present::{ GpuBoxShadow, GpuGlyphBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSolidMesh, GpuSolidRect, GpuStrokeRect, OffscreenTargetId };
use crate::native::factory::thread_bound::ThreadBoundGraphicsContext;
use std::ffi::c_void;

struct PanicIfCalled {
    readback_error: Option<Error>,
}

impl PanicIfCalled {
    fn foreign_only() -> Self {
        Self {
            readback_error: None,
        }
    }

    fn with_readback_error(error: Error) -> Self {
        Self {
            readback_error: Some(error),
        }
    }
}

impl IGraphicsContext for PanicIfCalled {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::cpu_pixel_upload(GraphicsBackend::D3d11, 1.0)
    }

    fn initialize(
        &mut self,
        _native_window: *mut c_void,
        _width: i32,
        _height: i32,
    ) -> Result<()> {
        panic!("foreign thread must not call the native context")
    }

    fn resize(&mut self, _width: i32, _height: i32) -> Result<()> {
        panic!("foreign thread must not call the native context")
    }

    fn make_current(&mut self) -> Result<()> {
        panic!("foreign thread must not call the native context")
    }

    fn swap_buffers(&mut self, _damage: PresentDamage) -> Result<()> {
        panic!("foreign thread must not call the native context")
    }

    fn try_shutdown(&mut self) -> Result<()> {
        Ok(())
    }

    fn read_pixels(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) -> Result<Vec<u32>> {
        if let Some(error) = &self.readback_error {
            return Err(error.clone());
        }
        panic!("foreign thread must not call the native context")
    }

    fn width(&self) -> i32 {
        1
    }

    fn height(&self) -> i32 {
        1
    }

    fn present(&mut self, _frame: &PresentFrame) -> Result<()> {
        panic!("foreign thread must not call the native context")
    }

    fn destroy_offscreen_target(
        &mut self,
        _id: crate::native::traits::present::OffscreenTargetId,
    ) {
        panic!("foreign thread must not call the native context")
    }
}

#[test]
fn wrong_owner_returns_typed_errors_before_native_calls() {
    let foreign_owner = std::thread::spawn(|| std::thread::current().id())
        .join()
        .expect("thread id");
    let mut context = ThreadBoundGraphicsContext::with_test_owner(
        Box::new(PanicIfCalled::foreign_only()),
        foreign_owner,
    );

    let initialize = context
        .initialize(std::ptr::null_mut(), 1, 1)
        .expect_err("owner mismatch");
    assert_eq!(initialize.code(), Errc::InvalidState);
    assert!(initialize.message().contains("initialize"));

    let present = context
        .present(&PresentFrame::Swapchain {
            damage: PresentDamage::Full,
        })
        .expect_err("owner mismatch");
    assert_eq!(present.code(), Errc::InvalidState);
    assert!(present.message().contains("present"));

    let offscreen = context
        .create_offscreen_target(4, 4)
        .expect_err("owner mismatch");
    assert_eq!(offscreen.code(), Errc::InvalidState);
    assert!(offscreen.message().contains("create_offscreen_target"));

    let shutdown = context.try_shutdown().expect_err("owner mismatch");
    assert_eq!(shutdown.code(), Errc::InvalidState);
    assert!(shutdown.message().contains("try_shutdown"));

    let readback = context.read_pixels(0, 0, 1, 1).expect_err("owner mismatch");
    assert_eq!(readback.code(), Errc::InvalidState);
    assert!(readback.message().contains("read_pixels"));

    let destroy = context
        .try_destroy_offscreen_target(crate::native::traits::present::OffscreenTargetId(7))
        .expect_err("owner mismatch");
    assert_eq!(destroy.code(), Errc::InvalidState);
    assert!(destroy.message().contains("try_destroy_offscreen_target"));

    // Legacy metadata access cannot return a typed failure, so it must be
    // served from the creation-thread snapshot rather than touch native
    // state on this foreign thread.
    assert_eq!(context.caps().backend, GraphicsBackend::D3d11);
    assert_eq!(context.width(), 1);
    assert_eq!(context.height(), 1);
    assert_eq!(context.device_pixel_ratio(), 1.0);
}

#[test]
fn owner_readback_propagates_the_native_typed_failure() {
    let mut context = ThreadBoundGraphicsContext::with_test_owner(
        Box::new(PanicIfCalled::with_readback_error(Error::new(
            Errc::GraphicsDeviceLost,
            "injected native readback failure",
        ))),
        std::thread::current().id(),
    );

    let error = context
        .read_pixels(0, 0, 1, 1)
        .expect_err("readback must preserve the native failure");
    assert_eq!(error.code(), Errc::GraphicsDeviceLost);
    assert!(error.message().contains("injected native readback failure"));
}

#[test]
fn foreign_thread_drop_does_not_reach_native_teardown() {
    let foreign_owner = std::thread::spawn(|| std::thread::current().id())
        .join()
        .expect("thread id");
    let context = ThreadBoundGraphicsContext::with_test_owner(
        Box::new(PanicIfCalled::foreign_only()),
        foreign_owner,
    );
    // Drop on this thread must log a typed owner mismatch and leak the
    // inner context rather than calling native shutdown/Drop.
    drop(context);
}
