use crate::core::error::Result;
use crate::native::traits::present::validate_pixel_buffer;
use crate::tests::common::*;
use std::str::FromStr;

#[test]
fn graphics_backend_parses_common_aliases() {
    assert_eq!(
        GraphicsBackend::from_str("direct3d-12").unwrap(),
        GraphicsBackend::D3d12
    );
    assert_eq!(
        GraphicsBackend::from_str("directx_11").unwrap(),
        GraphicsBackend::D3d11
    );
    assert_eq!(
        GraphicsBackend::from_str("OpenGL ES").unwrap(),
        GraphicsBackend::OpenGlEs
    );
    assert_eq!(
        GraphicsBackend::from_str("vk").unwrap(),
        GraphicsBackend::Vulkan
    );
}

#[test]
fn graphics_backend_display_uses_config_tokens() {
    assert_eq!(GraphicsBackend::Auto.to_string(), "auto");
    assert_eq!(GraphicsBackend::OpenGlEs.to_string(), "opengles");
    assert_eq!(GraphicsBackend::Metal.as_str(), "metal");
}

#[test]
fn d3d11_caps_advertise_offscreen_after_crop_and_scissor_are_correct() {
    assert!(
        NativeRasterCaps::d3d11_full().offscreen_targets,
        "D3D11 Picture offscreen support requires source crop and scissor restoration"
    );
}

#[test]
fn pixel_buffer_validation_rejects_invalid_extent_and_short_payload() {
    assert!(validate_pixel_buffer(&[0; 4], 2, 2).is_ok());
    assert_eq!(
        validate_pixel_buffer(&[0; 3], 2, 2)
            .expect_err("short payload")
            .code(),
        Errc::InvalidArgument
    );
    assert_eq!(
        validate_pixel_buffer(&[], 0, 1)
            .expect_err("empty width")
            .code(),
        Errc::InvalidArgument
    );
}

#[test]
fn compact_soft_tile_has_an_independent_destination_and_exact_payload_size() {
    let tile = SoftFallbackTile::at_destination(37, 19, 4, 3);

    assert_eq!((tile.dst_x, tile.dst_y), (37, 19));
    assert_eq!((tile.width, tile.height), (4, 3));
    assert_eq!(tile.required_pixels(), Some(12));
    assert_eq!(
        tile.validate_payload(&[0; 11])
            .expect_err("short tile payload")
            .code(),
        Errc::InvalidArgument
    );
    assert!(tile.validate_payload(&[0; 12]).is_ok());
}

struct DefaultPresentFailure {
    fail_make_current: bool,
    swap_calls: usize,
}

impl IGraphicsContext for DefaultPresentFailure {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(
            GraphicsBackend::D3d11,
            PresentCoherency::FullOnly,
            1.0,
        )
    }

    fn initialize(
        &mut self,
        _native_window: *mut std::ffi::c_void,
        _width: i32,
        _height: i32,
    ) -> Result<()> {
        Ok(())
    }

    fn resize(&mut self, _width: i32, _height: i32) -> Result<()> {
        Ok(())
    }

    fn make_current(&mut self) -> Result<()> {
        if self.fail_make_current {
            Err(Error::new(Errc::PlatformError, "make current failed"))
        } else {
            Ok(())
        }
    }

    fn swap_buffers(&mut self, _damage: PresentDamage) -> Result<()> {
        self.swap_calls += 1;
        Err(Error::new(Errc::PlatformError, "swap failed"))
    }

    fn try_shutdown(&mut self) -> Result<()> {
        Ok(())
    }

    fn read_pixels(
        &mut self,
        _x: i32,
        _y: i32,
        _width: i32,
        _height: i32,
    ) -> Result<Vec<u32>, Error> {
        Ok(Vec::new())
    }

    fn width(&self) -> i32 {
        1
    }

    fn height(&self) -> i32 {
        1
    }
}

#[test]
fn default_present_propagates_make_current_failure_without_swapping() {
    let mut context = DefaultPresentFailure {
        fail_make_current: true,
        swap_calls: 0,
    };

    let error = context
        .present(&PresentFrame::Swapchain {
            damage: PresentDamage::Full,
        })
        .expect_err("make_current failure must escape default present");

    assert!(error.message().contains("make current failed"));
    assert_eq!(context.swap_calls, 0);
}

#[test]
fn default_present_propagates_swap_failure() {
    let mut context = DefaultPresentFailure {
        fail_make_current: false,
        swap_calls: 0,
    };

    let error = context
        .present(&PresentFrame::Swapchain {
            damage: PresentDamage::Full,
        })
        .expect_err("swap failure must escape default present");

    assert!(error.message().contains("swap failed"));
    assert_eq!(context.swap_calls, 1);
}
