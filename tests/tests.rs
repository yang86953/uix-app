#![cfg(all(windows, feature = "vulkan"))]

mod tests {
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

    pub mod native {
        pub mod graphics {
            pub mod vulkan {
                pub mod platform {
                    pub mod context {
                        use super::super::super::super::super::expected_vendor;
                        use uix::gfx_r5_support::{
                            run_destroyed_hwnd_surface_lost, run_native_out_of_date_recovery,
                            run_vendor_resize_present_readback, NativeWindow,
                        };

                        #[test]
                        #[ignore = "requires an interactive Windows desktop and Vulkan driver"]
                        fn windows_vulkan_gfx_r5_expected_vendor_resize_present_readback() {
                            let (vendor, vendor_id) = expected_vendor();
                            let mut window = NativeWindow::new(137, 103).unwrap_or_else(|error| {
                                panic!("GFX-R5 test window failed: {error}")
                            });
                            let evidence = run_vendor_resize_present_readback(&mut window)
                                .unwrap_or_else(|error| {
                                    panic!("GFX-R5 vendor exact failed: {error}")
                                });

                            assert_eq!(evidence.vendor_id, vendor_id);
                            assert!(
                                evidence.swapchain_maintenance1,
                                "GFX-R5 requires VK_EXT_swapchain_maintenance1"
                            );
                            assert_eq!(evidence.initial_readback, 0xFF3478BC);
                            assert_eq!(evidence.resized_readback, 0xFF9A5C21);
                            println!(
                                "GFX-R5 Vulkan vendor evidence: expected={vendor}; {}; swapchain_maintenance1={}; initial_logical=137x103; initial_drawable={}x{}; initial_readback=0x{:08X}; resized_logical=211x149; resized_drawable={}x{}; resized_readback=0x{:08X}",
                                evidence.adapter_diagnostic,
                                evidence.swapchain_maintenance1,
                                evidence.initial_drawable.0,
                                evidence.initial_drawable.1,
                                evidence.initial_readback,
                                evidence.resized_drawable.0,
                                evidence.resized_drawable.1,
                                evidence.resized_readback,
                            );
                        }

                        #[test]
                        #[ignore = "requires an interactive Windows desktop and Vulkan driver"]
                        fn windows_vulkan_gfx_r5_native_out_of_date_is_typed_and_recovers() {
                            let (vendor, _) = expected_vendor();
                            let mut window = NativeWindow::new(137, 103).unwrap_or_else(|error| {
                                panic!("GFX-R5 test window failed: {error}")
                            });
                            let evidence = run_native_out_of_date_recovery(&mut window)
                                .unwrap_or_else(|error| {
                                    panic!("GFX-R5 native surface exact failed: {error}")
                                });

                            assert_eq!(evidence.fault.code, "graphics_surface_lost");
                            assert!(evidence.fault.message.contains("ERROR_OUT_OF_DATE_KHR"));
                            assert_ne!(evidence.initial_drawable, evidence.resized_drawable);
                            assert_eq!(evidence.recovered_readback, 0xFFB7642D);
                            println!(
                                "GFX-R5 Vulkan native surface fault error: {}",
                                evidence.fault
                            );
                            println!(
                                "GFX-R5 Vulkan native surface fault evidence: expected={vendor}; {}; fault_code={}; initial_drawable={}x{}; resized_logical={}x{}; resized_drawable={}x{}; recovered_drawable={}x{}; recovered_readback=0x{:08X}; recovered_present=true",
                                evidence.adapter_diagnostic,
                                evidence.fault.code,
                                evidence.initial_drawable.0,
                                evidence.initial_drawable.1,
                                evidence.logical_extent.0,
                                evidence.logical_extent.1,
                                evidence.resized_drawable.0,
                                evidence.resized_drawable.1,
                                evidence.resized_drawable.0,
                                evidence.resized_drawable.1,
                                evidence.recovered_readback,
                            );
                        }

                        #[test]
                        #[ignore = "requires an interactive Windows desktop and Vulkan driver"]
                        fn windows_vulkan_gfx_r5_destroyed_hwnd_returns_native_surface_lost() {
                            let (vendor, _) = expected_vendor();
                            let mut window = NativeWindow::new(137, 103).unwrap_or_else(|error| {
                                panic!("GFX-R5 test window failed: {error}")
                            });
                            let evidence = run_destroyed_hwnd_surface_lost(&mut window)
                                .unwrap_or_else(|error| {
                                    panic!("GFX-R5 fatal surface exact failed: {error}")
                                });

                            assert_eq!(evidence.fault.code, "graphics_surface_lost");
                            assert!(evidence.fault.message.contains("ERROR_SURFACE_LOST_KHR"));
                            println!("GFX-R5 Vulkan fatal surface error: {}", evidence.fault);
                            println!(
                                "GFX-R5 Vulkan fatal surface evidence: expected={vendor}; {}; fault_code={}; root_code={}; destroyed_hwnd=true",
                                evidence.adapter_diagnostic,
                                evidence.fault.code,
                                evidence.fault.code,
                            );
                        }
                    }
                }
            }
        }

        pub mod backends {
            pub mod windows {
                pub mod vulkan_fault_recovery {
                    use super::super::super::super::expected_vendor;
                    use uix::gfx_r5_support::{run_engine_recovery_boundary, NativeWindow};

                    #[test]
                    #[ignore = "requires an interactive Windows desktop and Vulkan driver"]
                    fn native_vulkan_surface_fault_reaches_engine_recovery_boundary() {
                        let (vendor, _) = expected_vendor();
                        let mut window = NativeWindow::new(137, 103)
                            .unwrap_or_else(|error| panic!("GFX-R5 test window failed: {error}"));
                        let evidence =
                            run_engine_recovery_boundary(&mut window).unwrap_or_else(|error| {
                                panic!("GFX-R5 engine recovery exact failed: {error}")
                            });

                        assert_eq!(evidence.fault.code, "graphics_surface_lost");
                        assert!(evidence.fault.message.contains("ERROR_OUT_OF_DATE_KHR"));
                        assert_eq!(evidence.action, "RebuildSurface");
                        assert_eq!(evidence.logical_extent, (229, 163));
                        assert_eq!(evidence.drawable_extent, (229, 163));
                        assert_eq!(evidence.recovered_readback, 0xFFB7642D);
                        println!("GFX-R5 engine recovery fault: {}", evidence.fault);
                        println!(
                            "GFX-R5 engine recovery evidence: expected={vendor}; {}; fault_code={}; action={}; logical_extent={}x{}; drawable_extent={}x{}; recovered_readback=0x{:08X}; recovered_present=true",
                            evidence.adapter_diagnostic,
                            evidence.fault.code,
                            evidence.action,
                            evidence.logical_extent.0,
                            evidence.logical_extent.1,
                            evidence.drawable_extent.0,
                            evidence.drawable_extent.1,
                            evidence.recovered_readback,
                        );
                    }
                }
            }
        }
    }
}
