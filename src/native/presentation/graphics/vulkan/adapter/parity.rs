//! Vulkan 显式 parity 的原生 fixture、诊断与故障恢复 Adapter。

use std::ffi::c_void;

use crate::core::Errc;
use crate::platform::presentation::rhi::{
    GraphicsContextRhi, HeadlessUiParityAdapter, RhiExtent, SurfaceToken, TextureHandle,
    WsiParityAdapter, WsiParityFramePresenter, WsiParityProfile,
};

use super::context::{VulkanContext, VulkanSurfaceFaultForParity};

pub(crate) struct VulkanHeadlessUiParityAdapter;

impl HeadlessUiParityAdapter for VulkanHeadlessUiParityAdapter {
    type Context = VulkanContext;

    fn create_context(extent: RhiExtent) -> crate::core::Result<Self::Context> {
        VulkanContext::new_headless_for_parity_test(extent)
    }

    fn diagnostic(context: &Self::Context) -> String {
        context.parity_adapter_diagnostic()
    }

    fn readback(context: &mut Self::Context, texture: TextureHandle) -> Vec<u8> {
        context.readback_texture_for_parity_test(texture)
    }
}

pub(crate) struct VulkanWsiParityAdapter;

impl WsiParityAdapter for VulkanWsiParityAdapter {
    type Context = VulkanContext;

    fn profile() -> WsiParityProfile {
        WsiParityProfile::new("Vulkan", "UIX Vulkan WSI parity", true)
    }

    fn create_context(
        native_surface: *mut c_void,
        width: i32,
        height: i32,
    ) -> crate::core::Result<Self::Context> {
        VulkanContext::new(native_surface, width, height)
    }

    fn diagnostic(context: &Self::Context) -> String {
        format!(
            "{}; {}; {}",
            native_session_diagnostic(),
            context.parity_adapter_diagnostic(),
            context.parity_surface_diagnostic(),
        )
    }

    fn verify_surface_recovery(
        context: &mut Self::Context,
        initial: SurfaceToken,
        presenter: &mut dyn WsiParityFramePresenter,
    ) {
        let mut current = initial;
        for fault in [
            VulkanSurfaceFaultForParity::AcquireOutOfDate,
            VulkanSurfaceFaultForParity::PresentOutOfDate,
            VulkanSurfaceFaultForParity::AcquireSuboptimal,
            VulkanSurfaceFaultForParity::PresentSuboptimal,
        ] {
            current = verify_surface_fault(context, presenter, fault);
        }
        assert_eq!(current.generation, initial.generation + 4);
        eprintln!(
            "Vulkan WSI Surface recovery verified: paths=4; generation={}->{}; recovery-ui-presents=4; injected-suboptimal-presents=2",
            initial.generation, current.generation,
        );
    }
}

// 在同一真实 WSI owner 上逐项证明四个原生返回位置的恢复语义。
fn verify_surface_fault(
    context: &mut VulkanContext,
    presenter: &mut dyn WsiParityFramePresenter,
    fault: VulkanSurfaceFaultForParity,
) -> SurfaceToken {
    let old = context.surface_ref().token();
    context
        .inject_surface_fault_for_parity_test(fault)
        .unwrap_or_else(|error| {
            panic!("{fault:?} injection must arm at an idle boundary: {error}")
        });
    assert_eq!(
        context.surface_ref().token(),
        old,
        "arming {fault:?} must not commit authoritative Surface state",
    );

    let injected = presenter.present(context, None);
    let rebuilt = context.surface_ref().token();
    assert_eq!(
        rebuilt.extent, old.extent,
        "{fault:?} must keep the real WSI extent"
    );
    assert_eq!(
        rebuilt.generation,
        old.generation + 1,
        "{fault:?} must commit exactly one recreated generation",
    );

    let semantics = match fault {
        VulkanSurfaceFaultForParity::AcquireOutOfDate
        | VulkanSurfaceFaultForParity::PresentOutOfDate => {
            let error = injected.expect_err("OUT_OF_DATE must reject the old frame");
            assert_eq!(error.code(), Errc::GraphicsSurfaceChanged);
            "RetryFrame(GraphicsSurfaceChanged)"
        }
        VulkanSurfaceFaultForParity::AcquireSuboptimal
        | VulkanSurfaceFaultForParity::PresentSuboptimal => {
            let presented = injected.expect("SUBOPTIMAL must keep the presented frame successful");
            assert_eq!(presented, rebuilt);
            assert_ne!(presented, old, "SUBOPTIMAL must not return the stale token");
            "Presented(Ok)"
        }
    };

    let recovered = presenter
        .present(context, None)
        .unwrap_or_else(|error| panic!("{fault:?} next real WSI frame must present: {error}"));
    assert_eq!(recovered, rebuilt);
    eprintln!(
        "Vulkan WSI recovery path: fault={fault:?}; old={}x{}@{}; new={}x{}@{}; injected={semantics}; recovered-present=ok@{}",
        old.extent.width,
        old.extent.height,
        old.generation,
        rebuilt.extent.width,
        rebuilt.extent.height,
        rebuilt.generation,
        recovered.generation,
    );
    rebuilt
}

#[cfg(all(unix, not(target_os = "macos")))]
fn native_session_diagnostic() -> String {
    let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "<unset>".to_owned());
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "<unset>".to_owned());
    format!("Wayland display={display}; session={session}")
}

#[cfg(windows)]
fn native_session_diagnostic() -> String {
    "Windows native window session".to_owned()
}

#[cfg(target_os = "macos")]
fn native_session_diagnostic() -> String {
    "macOS native window session".to_owned()
}
