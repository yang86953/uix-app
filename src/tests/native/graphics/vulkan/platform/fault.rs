use crate::native::graphics::vulkan::platform::fault::{
    select_feature_query_mode, DeviceFaultAddress, DeviceFaultFeatureQueryMode, DeviceFaultReport,
    DeviceFaultVendor, DeviceLossState,
};
use crate::tests::common::*;
use ash::vk;

#[cfg(windows)]
use crate::native::graphics::vulkan::platform::context::VulkanContext;
#[cfg(windows)]
use crate::tests::native::gfx_r5::{
    expected_device_fault_capability, expected_gfx_r5_vendor,
    requested_external_device_loss_timeout,
};

#[cfg(windows)]
const EXTERNAL_RESET_RECOVERY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
#[cfg(windows)]
const EXTERNAL_RESET_WATCHDOG_GRACE: std::time::Duration = std::time::Duration::from_secs(15);

#[cfg(windows)]
struct ExternalResetWatchdog {
    completion: Option<std::sync::mpsc::Sender<()>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

#[cfg(windows)]
impl ExternalResetWatchdog {
    fn arm(observation_timeout: std::time::Duration) -> Self {
        let hard_timeout =
            observation_timeout + EXTERNAL_RESET_RECOVERY_TIMEOUT + EXTERNAL_RESET_WATCHDOG_GRACE;
        let (completion, receiver) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            if receiver.recv_timeout(hard_timeout)
                == Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            {
                eprintln!(
                    "GFX-R5 external reset hard timeout after {hard_timeout:?}; aborting a blocked driver call"
                );
                std::process::abort();
            }
        });
        Self {
            completion: Some(completion),
            worker: Some(worker),
        }
    }
}

#[cfg(windows)]
impl Drop for ExternalResetWatchdog {
    fn drop(&mut self) {
        if let Some(completion) = self.completion.take() {
            let _ = completion.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[test]
fn device_fault_feature_query_prefers_vulkan_1_1_core() {
    assert_eq!(
        select_feature_query_mode(Some(vk::API_VERSION_1_3), true),
        (vk::API_VERSION_1_1, DeviceFaultFeatureQueryMode::Core11)
    );
}

#[test]
fn device_fault_feature_query_uses_khr_on_vulkan_1_0() {
    assert_eq!(
        select_feature_query_mode(Some(vk::API_VERSION_1_0), true),
        (vk::API_VERSION_1_0, DeviceFaultFeatureQueryMode::Khr)
    );
    assert_eq!(
        select_feature_query_mode(None, true),
        (vk::API_VERSION_1_0, DeviceFaultFeatureQueryMode::Khr)
    );
}

#[test]
fn device_fault_feature_query_stays_optional_without_a_query_path() {
    assert_eq!(
        select_feature_query_mode(Some(vk::API_VERSION_1_0), false),
        (
            vk::API_VERSION_1_0,
            DeviceFaultFeatureQueryMode::Unavailable
        )
    );
}

#[test]
fn device_fault_report_keeps_bounded_driver_diagnostics_readable() {
    let report = DeviceFaultReport {
        description: "  page fault\nwhile executing \"shader\"  ".to_owned(),
        addresses: vec![DeviceFaultAddress {
            address_type: vk::DeviceFaultAddressTypeEXT::READ_INVALID.as_raw(),
            reported_address: 0x1234,
            address_precision: 64,
        }],
        vendors: vec![DeviceFaultVendor {
            description: "warp timeout".to_owned(),
            code: 0xA,
            data: 0xB,
        }],
        vendor_binary_bytes: 4096,
        truncated: true,
    };

    let summary = report.diagnostic_summary();

    assert!(summary.contains("description=\"page fault while executing 'shader'\""));
    assert!(summary.contains("read_invalid@0x0000000000001234±64"));
    assert!(summary.contains("\"warp timeout\" code=0x000000000000000A"));
    assert!(summary.contains("vendor_binary_bytes=4096"));
    assert!(summary.contains("truncated=true"));
    assert!(!summary.contains('\n'));
}

#[test]
fn empty_device_fault_report_is_explicit() {
    let report = DeviceFaultReport {
        description: String::new(),
        addresses: Vec::new(),
        vendors: Vec::new(),
        vendor_binary_bytes: 0,
        truncated: false,
    };

    assert_eq!(
        report.diagnostic_summary(),
        "VK_EXT_device_fault: description=\"unavailable\"; addresses=[none]; vendors=[none]; vendor_binary_bytes=0; truncated=false"
    );
}

#[test]
fn device_loss_state_keeps_the_first_enriched_error_for_peers() {
    let state = DeviceLossState::default();
    let first = state.record_with(
        Error::new(Errc::GraphicsDeviceLost, "vkQueueSubmit failed"),
        |error| {
            error.with_source(Error::new(
                Errc::GraphicsDeviceLost,
                "VK_EXT_device_fault: page fault",
            ))
        },
    );

    assert_eq!(first.depth(), 1);
    let peer = state.peer_error().expect("loss must be visible to peers");
    assert_eq!(peer.code(), Errc::GraphicsDeviceLost);
    assert!(peer.message().contains("shared logical device"));
    assert_eq!(
        peer.root_cause().message(),
        "VK_EXT_device_fault: page fault"
    );
}

#[test]
fn repeated_device_loss_keeps_current_operation_and_first_diagnosis() {
    let state = DeviceLossState::default();
    let first = state.record_with(
        Error::new(Errc::GraphicsDeviceLost, "first device loss"),
        |error| error.with_source(Error::new(Errc::GraphicsDeviceLost, "first diagnosis")),
    );
    let repeated = state.record_with(
        Error::new(Errc::GraphicsDeviceLost, "second device loss").with_source(Error::new(
            Errc::PlatformError,
            "second operation cleanup failed",
        )),
        |error| error.with_source(Error::new(Errc::GraphicsDeviceLost, "must not replace")),
    );

    assert_eq!(repeated.message(), "second device loss");
    let cleanup = repeated
        .source_error()
        .expect("current operation context must remain first");
    assert_eq!(cleanup.message(), "second operation cleanup failed");
    assert_eq!(cleanup.source_error(), Some(&first));
    assert_eq!(repeated.root_cause().message(), "first diagnosis");
}

#[test]
fn non_device_loss_does_not_poison_shared_device_state() {
    let state = DeviceLossState::default();
    let error = state.record_with(
        Error::new(Errc::GraphicsSurfaceLost, "surface only"),
        |error| error,
    );

    assert_eq!(error.code(), Errc::GraphicsSurfaceLost);
    assert!(state.peer_error().is_none());
}

#[cfg(windows)]
#[test]
#[ignore = "requires a Vulkan-capable Windows driver and UIX_VULKAN_EXPECT_DEVICE_FAULT=true|false"]
fn windows_vulkan_device_fault_reporting_matches_expected_capability() {
    let expected = expected_device_fault_capability().unwrap_or_else(|error| panic!("{error}"));
    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("Vulkan device fault capability", 96, 64)
        .expect("window");
    let mut context = VulkanContext::new(window.native_surface_ptr(), 96, 64)
        .expect("VulkanContext for device fault capability");

    assert_eq!(
        context.device_fault_reporting_enabled_for_test(),
        expected,
        "unexpected VK_EXT_device_fault capability: {}",
        context.adapter_info.diagnostic_summary()
    );
    println!(
        "Vulkan device fault capability: enabled={expected}; {}",
        context.adapter_info.diagnostic_summary()
    );

    context.try_shutdown().expect("shutdown");
    window.close().expect("close window");
}

#[cfg(windows)]
#[test]
#[ignore = "requires UIX_GFX_R5_EXPECT_VENDOR, UIX_VULKAN_EXPECT_DEVICE_FAULT, and an external driver reset while running"]
fn windows_vulkan_gfx_r5_external_reset_returns_device_lost_with_diagnostics() {
    let expected_vendor = expected_gfx_r5_vendor().unwrap_or_else(|error| panic!("{error}"));
    let expect_fault_report =
        expected_device_fault_capability().unwrap_or_else(|error| panic!("{error}"));
    let timeout =
        requested_external_device_loss_timeout().unwrap_or_else(|error| panic!("{error}"));
    let mut platform = crate::native::create_platform().expect("platform");
    let mut first_window = platform
        .window_manager()
        .create_window("Vulkan external device loss A", 128, 96)
        .expect("first window");
    let mut second_window = platform
        .window_manager()
        .create_window("Vulkan external device loss B", 128, 96)
        .expect("second window");
    first_window.show().expect("show first window");
    second_window.show().expect("show second window");
    let _ = platform.event_loop().poll_event(&|_| true);

    let mut first = VulkanContext::new(first_window.native_surface_ptr(), 128, 96)
        .expect("first VulkanContext");
    let mut second = VulkanContext::new(second_window.native_surface_ptr(), 128, 96)
        .expect("second VulkanContext");
    expected_vendor.assert_runtime(
        &first.adapter_info,
        first.swapchain_maintenance1_enabled_for_test(),
    );
    expected_vendor.assert_runtime(
        &second.adapter_info,
        second.swapchain_maintenance1_enabled_for_test(),
    );
    assert_eq!(
        first.shared_device_identity(),
        second.shared_device_identity()
    );
    assert_eq!(
        first.device_fault_reporting_enabled_for_test(),
        expect_fault_report
    );
    let lost_identity = first.shared_device_identity();
    let first_pixels = vec![0xFF21_5A8C; (first.width() * first.height()) as usize];
    let second_pixels = vec![0xFF8C_5A21; (second.width() * second.height()) as usize];
    let deadline = std::time::Instant::now() + timeout;
    let mut frames = 0_u64;
    let mut surface_faults = 0_u64;
    let mut last_surface_fault = None;

    eprintln!(
        "GFX-R5 external reset armed: trigger the driver reset within {timeout:?}; expected={}; device_fault={expect_fault_report}; {}; swapchain_maintenance1=true",
        expected_vendor.label(),
        first.adapter_info.diagnostic_summary()
    );
    let _watchdog = ExternalResetWatchdog::arm(timeout);

    let (fault, first_detected) = loop {
        let _ = platform.event_loop().poll_event(&|_| true);
        let first_result = first.present_pixels(
            &first_pixels,
            first.width(),
            first.height(),
            PresentDamage::Full,
        );
        match first_result {
            Ok(()) => {}
            Err(error) if error.code() == Errc::GraphicsDeviceLost => break (error, true),
            Err(error) if error.code() == Errc::GraphicsSurfaceLost => {
                surface_faults += 1;
                last_surface_fault = Some(error.what());
            }
            Err(error) => panic!("unexpected first-context failure: {}", error.what()),
        }
        let second_result = second.present_pixels(
            &second_pixels,
            second.width(),
            second.height(),
            PresentDamage::Full,
        );
        match second_result {
            Ok(()) => {}
            Err(error) if error.code() == Errc::GraphicsDeviceLost => break (error, false),
            Err(error) if error.code() == Errc::GraphicsSurfaceLost => {
                surface_faults += 1;
                last_surface_fault = Some(error.what());
            }
            Err(error) => panic!("unexpected second-context failure: {}", error.what()),
        }
        frames += 1;
        assert!(
            std::time::Instant::now() < deadline,
            "no native device loss observed within {timeout:?}; frames={frames}; surface_faults={surface_faults}; last_surface_fault={last_surface_fault:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(16));
    };

    assert!(
        fault.what().contains("ERROR_DEVICE_LOST"),
        "{}",
        fault.what()
    );
    if expect_fault_report {
        assert!(
            fault.what().contains("VK_EXT_device_fault"),
            "enabled fault reporting must remain in the cause chain: {}",
            fault.what()
        );
    }
    let peer_error = if first_detected {
        second
            .present_pixels(
                &second_pixels,
                second.width(),
                second.height(),
                PresentDamage::Full,
            )
            .expect_err("second context must observe the shared loss")
    } else {
        first
            .present_pixels(
                &first_pixels,
                first.width(),
                first.height(),
                PresentDamage::Full,
            )
            .expect_err("first context must observe the shared loss")
    };
    assert_eq!(peer_error.code(), Errc::GraphicsDeviceLost);
    assert_eq!(peer_error.root_cause(), fault.root_cause());

    first.try_shutdown().expect("shutdown first lost context");
    second.try_shutdown().expect("shutdown second lost context");
    drop((first, second));
    first_window.close().expect("close first window");
    second_window.close().expect("close second window");

    let mut replacement_window = platform
        .window_manager()
        .create_window("Vulkan device loss replacement", 128, 96)
        .expect("replacement window");
    replacement_window.show().expect("show replacement window");
    let _ = platform.event_loop().poll_event(&|_| true);
    let recovery_deadline = std::time::Instant::now() + EXTERNAL_RESET_RECOVERY_TIMEOUT;
    let mut replacement_attempts = 0_u64;
    let mut replacement = loop {
        replacement_attempts += 1;
        match VulkanContext::new(replacement_window.native_surface_ptr(), 128, 96) {
            Ok(context) => break context,
            Err(error) => {
                let last_error = error.what();
                assert!(
                    std::time::Instant::now() < recovery_deadline,
                    "replacement VulkanContext did not recover within {:?}; attempts={replacement_attempts}; last_error={last_error}",
                    EXTERNAL_RESET_RECOVERY_TIMEOUT
                );
                let _ = platform.event_loop().poll_event(&|_| true);
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    };
    assert_ne!(replacement.shared_device_identity(), lost_identity);
    expected_vendor.assert_runtime(
        &replacement.adapter_info,
        replacement.swapchain_maintenance1_enabled_for_test(),
    );
    assert_eq!(
        replacement.device_fault_reporting_enabled_for_test(),
        expect_fault_report,
        "replacement changed VK_EXT_device_fault capability: {}",
        replacement.adapter_info.diagnostic_summary()
    );
    let replacement_pixels =
        vec![0xFF3C_7AB5; (replacement.width() * replacement.height()) as usize];
    replacement
        .present_pixels(
            &replacement_pixels,
            replacement.width(),
            replacement.height(),
            PresentDamage::Full,
        )
        .expect("replacement present after external reset");
    println!(
        "GFX-R5 external device loss: expected={}; detector={}; frames={frames}; surface_faults={surface_faults}; fault={}; peer={}; replacement_attempts={replacement_attempts}; replacement={}",
        expected_vendor.label(),
        if first_detected { "first" } else { "second" },
        fault.what(),
        peer_error.what(),
        replacement.adapter_info.diagnostic_summary()
    );

    replacement.try_shutdown().expect("shutdown replacement");
    replacement_window
        .close()
        .expect("close replacement window");
}
