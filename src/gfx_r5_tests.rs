use std::ffi::c_void;

use crate::native::backends::windows::platform::WindowsPlatform;
use crate::native::windowing::{IWindowManager, PlatformWindow};

pub(crate) struct NativeWindowFixture {
    // WindowBinding keeps a raw pointer to WindowsPlatform. Keep the platform
    // on the heap so moving this fixture cannot invalidate that callback ptr.
    _platform: Box<WindowsPlatform>,
    window: Box<dyn PlatformWindow>,
}

impl NativeWindowFixture {
    pub(crate) fn new(width: i32, height: i32) -> Self {
        let mut platform = Box::new(WindowsPlatform::new());
        let mut window = match platform.create_window("UIX GFX-R5", width, height) {
            Ok(window) => window,
            Err(error) => panic!("GFX-R5 test window creation failed: {error}"),
        };
        if let Err(error) = window.show() {
            panic!("GFX-R5 test window show failed: {error}");
        }
        Self {
            _platform: platform,
            window,
        }
    }

    pub(crate) fn surface(&self) -> *mut c_void {
        let surface = self.window.native_surface_ptr();
        assert!(
            !surface.is_null(),
            "GFX-R5 test window returned a null HWND"
        );
        surface
    }

    pub(crate) fn resize(&mut self, width: i32, height: i32) {
        if let Err(error) = self.window.properties_mut().set_size(width, height) {
            panic!("GFX-R5 test window resize failed: {error}");
        }
    }
}

impl Drop for NativeWindowFixture {
    fn drop(&mut self) {
        let _ = self.window.close();
    }
}

fn expected_vendor() -> (&'static str, u32) {
    let value = match std::env::var("UIX_GFX_R5_EXPECT_VENDOR") {
        Ok(value) => value,
        Err(_) => "nvidia".to_owned(),
    };
    match value.as_str() {
        "nvidia" => ("nvidia", 0x10DE),
        "amd" => ("amd", 0x1002),
        "intel" => ("intel", 0x8086),
        other => panic!("unsupported GFX-R5 vendor expectation: {other}"),
    }
}

pub(crate) mod native {
    pub(crate) mod graphics {
        pub(crate) mod vulkan {
            pub(crate) mod platform {
                pub(crate) mod context {
                    use super::super::super::super::super::{expected_vendor, NativeWindowFixture};
                    use crate::native::present::{IGraphicsContext, PresentDamage};
                    use crate::native::presentation::graphics::vulkan::platform::VulkanContext;

                    #[test]
                    #[ignore = "requires an interactive Windows desktop and Vulkan driver"]
                    fn windows_vulkan_gfx_r5_expected_vendor_resize_present_readback() {
                        let (vendor, vendor_id) = expected_vendor();
                        let mut fixture = NativeWindowFixture::new(137, 103);
                        let mut context = match VulkanContext::new(fixture.surface(), 137, 103) {
                            Ok(context) => context,
                            Err(error) => panic!("GFX-R5 Vulkan context creation failed: {error}"),
                        };
                        assert_eq!(context.adapter_info.vendor_id, vendor_id);
                        assert!(
                            context.swapchain_maintenance1_enabled_for_test(),
                            "GFX-R5 requires VK_EXT_swapchain_maintenance1"
                        );

                        let initial_drawable = (context.width(), context.height());
                        let initial_pixels = vec![
                            0xFF3478BC;
                            (initial_drawable.0 as usize)
                                * (initial_drawable.1 as usize)
                        ];
                        if let Err(error) = context.present_pixels(
                            &initial_pixels,
                            initial_drawable.0,
                            initial_drawable.1,
                            PresentDamage::Full,
                        ) {
                            panic!("GFX-R5 initial Vulkan present failed: {error}");
                        }
                        let initial_readback = match context.read_pixels(0, 0, 1, 1) {
                            Ok(pixels) => pixels[0],
                            Err(error) => panic!("GFX-R5 initial Vulkan readback failed: {error}"),
                        };

                        fixture.resize(211, 149);
                        if let Err(error) = context.resize(211, 149) {
                            panic!("GFX-R5 Vulkan resize failed: {error}");
                        }
                        let resized_drawable = (context.width(), context.height());
                        let resized_pixels = vec![
                            0xFF9A5C21;
                            (resized_drawable.0 as usize)
                                * (resized_drawable.1 as usize)
                        ];
                        if let Err(error) = context.present_pixels(
                            &resized_pixels,
                            resized_drawable.0,
                            resized_drawable.1,
                            PresentDamage::Full,
                        ) {
                            panic!("GFX-R5 resized Vulkan present failed: {error}");
                        }
                        let resized_readback = match context.read_pixels(0, 0, 1, 1) {
                            Ok(pixels) => pixels[0],
                            Err(error) => panic!("GFX-R5 resized Vulkan readback failed: {error}"),
                        };

                        println!(
                            "GFX-R5 Vulkan vendor evidence: expected={vendor}; {}; swapchain_maintenance1=true; initial_logical=137x103; initial_drawable={}x{}; initial_readback=0x{initial_readback:08X}; resized_logical=211x149; resized_drawable={}x{}; resized_readback=0x{resized_readback:08X}",
                            context.adapter_info.diagnostic_summary(),
                            initial_drawable.0,
                            initial_drawable.1,
                            resized_drawable.0,
                            resized_drawable.1,
                        );

                        assert_eq!(initial_readback, 0xFF3478BC);
                        assert_eq!(resized_readback, 0xFF9A5C21);
                        if let Err(error) = context.try_shutdown() {
                            panic!("GFX-R5 Vulkan context shutdown failed: {error}");
                        }
                    }
                }
            }
        }
    }
}
