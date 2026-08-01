//! Windows Vulkan support used by the integration tests in the tests
//! directory.
//!
//! This module deliberately contains no test entry points. It keeps the
//! native resource boundary inside the library while the test names,
//! assertions, and evidence output remain integration-test code.

use crate::core::{Errc, Error};
use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
use crate::draw::backend::DamageRegion;
use crate::draw::renderer::{
    RecoveryAction, RecoveryDriver, RenderOutcome, RenderTarget, RenderTargetRebuilder,
    UpdateStrategy,
};
use crate::draw::Canvas2D;
use crate::native::backends::windows::platform::WindowsPlatform;
use crate::native::present::{IGraphicsContext, PresentDamage, PresentTestResult};
use crate::native::presentation::graphics::vulkan::platform::VulkanContext;
use crate::native::windowing::{IWindowManager, PlatformWindow};
use std::ffi::c_void;
use std::fmt;
use std::sync::{Arc, Mutex};

/// A typed failure boundary exposed only for the Windows Vulkan integration
/// test harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GfxR5Error {
    /// Stable UIX error category used by the test assertions.
    pub code: &'static str,
    /// Full native error text, including the Vulkan result when available.
    pub message: String,
}

impl fmt::Display for GfxR5Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for GfxR5Error {}

/// Result type shared by the GFX-R5 test support operations.
pub type GfxR5Result<T> = Result<T, GfxR5Error>;

fn error_code(code: Errc) -> &'static str {
    match code {
        Errc::GraphicsSurfaceLost => "graphics_surface_lost",
        Errc::GraphicsDeviceLost => "graphics_device_lost",
        Errc::GraphicsOutOfMemory => "graphics_out_of_memory",
        Errc::GraphicsOccluded => "graphics_occluded",
        Errc::WindowCreationFailed => "window_creation_failed",
        Errc::ClassRegistrationFailed => "class_registration_failed",
        _ => "native_error",
    }
}

fn map_error(error: Error) -> GfxR5Error {
    GfxR5Error {
        code: error_code(error.code()),
        message: error.to_string(),
    }
}

fn support_failure(message: impl Into<String>) -> GfxR5Error {
    GfxR5Error {
        code: "test_support_failure",
        message: message.into(),
    }
}

/// A visible Windows window whose native handle remains valid for the whole
/// lifetime of the Vulkan surface under test.
pub struct NativeWindow {
    // WindowBinding stores a raw pointer to WindowsPlatform. Pin the platform
    // on the heap so moving this wrapper cannot invalidate that callback ptr.
    _platform: Box<WindowsPlatform>,
    window: Box<dyn PlatformWindow>,
    closed: bool,
}

impl NativeWindow {
    /// Creates and shows a real Win32 window.
    pub fn new(width: i32, height: i32) -> GfxR5Result<Self> {
        let mut platform = Box::new(WindowsPlatform::new());
        let mut window = platform
            .create_window("UIX GFX-R5", width, height)
            .map_err(map_error)?;
        window.show().map_err(map_error)?;
        Ok(Self {
            _platform: platform,
            window,
            closed: false,
        })
    }

    /// Returns the HWND used to create the Vulkan surface.
    pub fn surface(&self) -> GfxR5Result<*mut c_void> {
        let surface = self.window.native_surface_ptr();
        if surface.is_null() {
            return Err(support_failure("GFX-R5 test window returned a null HWND"));
        }
        Ok(surface)
    }

    /// Resizes the native window without touching a Vulkan context.
    pub fn resize(&mut self, width: i32, height: i32) -> GfxR5Result<()> {
        self.window
            .properties_mut()
            .set_size(width, height)
            .map_err(map_error)
    }

    /// Closes the native window once, preserving the destroyed HWND for
    /// explicit native-surface failure tests.
    pub fn close(&mut self) -> GfxR5Result<()> {
        if self.closed {
            return Ok(());
        }
        self.window.close().map_err(map_error)?;
        self.closed = true;
        Ok(())
    }
}

impl Drop for NativeWindow {
    fn drop(&mut self) {
        if !self.closed {
            let _ = self.window.close();
        }
    }
}

fn present_solid(context: &mut VulkanContext, color: u32) -> GfxR5Result<()> {
    let width = context.width();
    let height = context.height();
    let pixels = vec![color; (width as usize) * (height as usize)];
    context
        .present_pixels(&pixels, width, height, PresentDamage::Full)
        .map_err(map_error)
}

fn readback_one(context: &mut VulkanContext) -> GfxR5Result<u32> {
    context
        .read_pixels(0, 0, 1, 1)
        .map(|pixels| pixels[0])
        .map_err(map_error)
}

/// Evidence for the vendor/resize/present/readback exact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendorResizeEvidence {
    pub adapter_diagnostic: String,
    pub vendor_id: u32,
    pub swapchain_maintenance1: bool,
    pub initial_drawable: (i32, i32),
    pub initial_readback: u32,
    pub resized_drawable: (i32, i32),
    pub resized_readback: u32,
}

/// Runs the real Win32 Vulkan vendor and resize boundary without making any
/// test assertions in the library.
pub fn run_vendor_resize_present_readback(
    window: &mut NativeWindow,
) -> GfxR5Result<VendorResizeEvidence> {
    let surface = window.surface()?;
    let mut context = VulkanContext::new(surface, 137, 103).map_err(map_error)?;
    let evidence = (|| {
        let adapter_diagnostic = context.adapter_info.diagnostic_summary();
        let vendor_id = context.adapter_info.vendor_id;
        let swapchain_maintenance1 = context.swapchain_maintenance1_enabled_for_test();
        let initial_drawable = (context.width(), context.height());
        present_solid(&mut context, 0xFF3478BC)?;
        let initial_readback = readback_one(&mut context)?;

        window.resize(211, 149)?;
        context.resize(211, 149).map_err(map_error)?;
        let resized_drawable = (context.width(), context.height());
        present_solid(&mut context, 0xFF9A5C21)?;
        let resized_readback = readback_one(&mut context)?;

        Ok(VendorResizeEvidence {
            adapter_diagnostic,
            vendor_id,
            swapchain_maintenance1,
            initial_drawable,
            initial_readback,
            resized_drawable,
            resized_readback,
        })
    })();
    context.try_shutdown().map_err(map_error)?;
    evidence
}

/// Evidence for a native out-of-date fault followed by an explicit surface
/// resize and recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOutOfDateEvidence {
    pub adapter_diagnostic: String,
    pub fault: GfxR5Error,
    pub initial_drawable: (i32, i32),
    pub logical_extent: (i32, i32),
    pub resized_drawable: (i32, i32),
    pub recovered_readback: u32,
}

/// Runs the native Vulkan out-of-date transition and returns the typed fault
/// for assertions in the integration test.
pub fn run_native_out_of_date_recovery(
    window: &mut NativeWindow,
) -> GfxR5Result<NativeOutOfDateEvidence> {
    let surface = window.surface()?;
    let mut context = VulkanContext::new(surface, 137, 103).map_err(map_error)?;
    let evidence = (|| {
        let adapter_diagnostic = context.adapter_info.diagnostic_summary();
        let initial_drawable = (context.width(), context.height());
        present_solid(&mut context, 0xFF3478BC)?;

        window.resize(223, 157)?;
        let stale_width = initial_drawable.0;
        let stale_height = initial_drawable.1;
        let stale_pixels = vec![0xFF3478BC; (stale_width as usize) * (stale_height as usize)];
        let fault = match context.present_pixels(
            &stale_pixels,
            stale_width,
            stale_height,
            PresentDamage::Full,
        ) {
            Ok(()) => {
                return Err(support_failure(
                    "GFX-R5 native resize did not produce ERROR_OUT_OF_DATE_KHR",
                ))
            }
            Err(error) => map_error(error),
        };
        let logical_extent = (223, 157);

        context
            .resize(logical_extent.0, logical_extent.1)
            .map_err(map_error)?;
        let resized_drawable = (context.width(), context.height());
        present_solid(&mut context, 0xFFB7642D)?;
        let recovered_readback = readback_one(&mut context)?;

        Ok(NativeOutOfDateEvidence {
            adapter_diagnostic,
            fault,
            initial_drawable,
            logical_extent,
            resized_drawable,
            recovered_readback,
        })
    })();
    context.try_shutdown().map_err(map_error)?;
    evidence
}

/// Evidence for attempting to create a Vulkan surface from a destroyed HWND.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FatalSurfaceEvidence {
    pub adapter_diagnostic: String,
    pub fault: GfxR5Error,
}

/// Runs the fatal native-surface exact against a real destroyed Win32 HWND.
pub fn run_destroyed_hwnd_surface_lost(
    window: &mut NativeWindow,
) -> GfxR5Result<FatalSurfaceEvidence> {
    let surface = window.surface()?;
    let mut context = VulkanContext::new(surface, 137, 103).map_err(map_error)?;
    let adapter_diagnostic = context.adapter_info.diagnostic_summary();
    present_solid(&mut context, 0xFF3478BC)?;
    context.try_shutdown().map_err(map_error)?;
    drop(context);
    window.close()?;

    let fault = match VulkanContext::new(surface, 137, 103) {
        Ok(mut replacement) => {
            let _ = replacement.try_shutdown();
            return Err(support_failure(
                "GFX-R5 destroyed HWND unexpectedly created a Vulkan context",
            ));
        }
        Err(error) => map_error(error),
    };
    Ok(FatalSurfaceEvidence {
        adapter_diagnostic,
        fault,
    })
}

struct VulkanRecoveryTarget {
    context: VulkanContext,
    canvas: NoopCanvas2D,
    recovery_evidence: Arc<Mutex<Option<(i32, i32, u32)>>>,
}

impl VulkanRecoveryTarget {
    fn new(context: VulkanContext, recovery_evidence: Arc<Mutex<Option<(i32, i32, u32)>>>) -> Self {
        Self {
            context,
            canvas: NoopCanvas2D,
            recovery_evidence,
        }
    }
}

impl RenderTarget for VulkanRecoveryTarget {
    fn initialize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
        Ok(())
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.context.try_shutdown()
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.context.resize(width, height)
    }

    fn begin_frame(&mut self, _strategy: UpdateStrategy) -> RenderOutcome {
        RenderOutcome::FrameReady(DamageRegion::full())
    }

    fn end_frame(&mut self, _present_damage: &DamageRegion) -> RenderOutcome {
        RenderOutcome::Present(DamageRegion::full())
    }

    fn test_present(&mut self) -> Result<PresentTestResult, Error> {
        let width = self.context.width();
        let height = self.context.height();
        let pixels = vec![0xFFB7642D; (width as usize) * (height as usize)];
        self.context
            .present_pixels(&pixels, width, height, PresentDamage::Full)?;
        let readback = self.context.read_pixels(0, 0, 1, 1)?[0];
        *self
            .recovery_evidence
            .lock()
            .expect("GFX-R5 recovery evidence lock") = Some((width, height, readback));
        Ok(PresentTestResult::Presentable)
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        &mut self.canvas
    }

    fn logical_extent(&mut self) -> (i32, i32) {
        (self.context.width(), self.context.height())
    }
}

/// Evidence for a native surface fault crossing the engine recovery driver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineRecoveryEvidence {
    pub adapter_diagnostic: String,
    pub fault: GfxR5Error,
    pub action: &'static str,
    pub logical_extent: (i32, i32),
    pub drawable_extent: (i32, i32),
    pub recovered_readback: u32,
}

/// Runs the real Vulkan fault through the bounded engine recovery wrapper.
pub fn run_engine_recovery_boundary(
    window: &mut NativeWindow,
) -> GfxR5Result<EngineRecoveryEvidence> {
    let surface = window.surface()?;
    let mut context = VulkanContext::new(surface, 137, 103).map_err(map_error)?;
    let adapter_diagnostic = context.adapter_info.diagnostic_summary();
    present_solid(&mut context, 0xFF3478BC)?;

    window.resize(229, 163)?;
    let stale_width = context.width();
    let stale_height = context.height();
    let stale_pixels = vec![0xFF3478BC; (stale_width as usize) * (stale_height as usize)];
    let fault = match context.present_pixels(
        &stale_pixels,
        stale_width,
        stale_height,
        PresentDamage::Full,
    ) {
        Ok(()) => {
            return Err(support_failure(
                "GFX-R5 engine recovery did not observe ERROR_OUT_OF_DATE_KHR",
            ))
        }
        Err(error) => error,
    };
    let fault_evidence = map_error(fault.clone());

    let recovery_evidence = Arc::new(Mutex::new(None));
    let rebuilder_evidence = Arc::clone(&recovery_evidence);
    let rebuilder: RenderTargetRebuilder = Box::new(move |action, width, height| {
        if action != RecoveryAction::RebuildSurface {
            return Err(Error::new(
                Errc::InvalidState,
                format!("GFX-R5 expected RebuildSurface, got {action:?}"),
            ));
        }
        if (width, height) != (229, 163) {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!("GFX-R5 recovery extent was {width}x{height}, expected 229x163"),
            ));
        }
        let replacement = VulkanContext::new(surface, width, height)?;
        Ok(Box::new(VulkanRecoveryTarget::new(
            replacement,
            Arc::clone(&rebuilder_evidence),
        )) as Box<dyn RenderTarget>)
    });
    let target = VulkanRecoveryTarget::new(context, Arc::clone(&recovery_evidence));
    let mut driver = RecoveryDriver::new(Box::new(target), rebuilder).with_extent(229, 163);
    driver.external_present_failed(fault);

    let outcome = driver.begin_frame(UpdateStrategy::FullRedraw);
    if !matches!(outcome, RenderOutcome::FrameReady(_)) {
        let _ = driver.try_shutdown();
        return Err(support_failure(format!(
            "GFX-R5 engine recovery did not resume after rebuild: {outcome:?}"
        )));
    }
    if driver.test_present().map_err(map_error)? != PresentTestResult::Presentable {
        let _ = driver.try_shutdown();
        return Err(support_failure(
            "GFX-R5 engine recovery replacement was not presentable",
        ));
    }

    let recovered = recovery_evidence
        .lock()
        .map_err(|_| support_failure("GFX-R5 recovery evidence lock poisoned"))?
        .ok_or_else(|| support_failure("GFX-R5 recovery present evidence missing"))?;
    driver.external_present_succeeded();
    driver.try_shutdown().map_err(map_error)?;

    Ok(EngineRecoveryEvidence {
        adapter_diagnostic,
        fault: fault_evidence,
        action: "RebuildSurface",
        logical_extent: (229, 163),
        drawable_extent: (recovered.0, recovered.1),
        recovered_readback: recovered.2,
    })
}
