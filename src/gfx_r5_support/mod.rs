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
use crate::native::backends::windows::display::WindowsDisplay;
use crate::native::backends::windows::dpi::dpi_for_window;
use crate::native::backends::windows::platform::WindowsPlatform;
use crate::native::capabilities::display::IDisplay;
use crate::native::present::{IGraphicsContext, PresentDamage, PresentTestResult};
use crate::native::presentation::graphics::vulkan::platform::VulkanContext;
use crate::native::windowing::{IWindowManager, PlatformWindow};
use std::ffi::c_void;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetGuiResources, GR_GDIOBJECTS, GR_USEROBJECTS,
};

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

pub(crate) fn error_code(code: Errc) -> &'static str {
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

pub(crate) fn map_error(error: Error) -> GfxR5Error {
    GfxR5Error {
        code: error_code(error.code()),
        message: error.to_string(),
    }
}

pub(crate) fn support_failure(message: impl Into<String>) -> GfxR5Error {
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

    /// Moves the native window to an absolute virtual-desktop position.
    pub fn set_position(&mut self, x: i32, y: i32) -> GfxR5Result<()> {
        self.window
            .properties_mut()
            .set_position(x, y)
            .map_err(map_error)
    }

    /// Returns the effective per-window DPI reported by Win32.
    pub fn dpi(&self) -> GfxR5Result<u32> {
        Ok(dpi_for_window(self.surface()?))
    }

    /// Returns the physical bounds of the monitor currently containing the window.
    pub fn monitor_bounds(&self) -> GfxR5Result<(i32, i32, i32, i32)> {
        let hwnd = HWND(self.surface()?);
        let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
        if monitor.0.is_null() {
            return Err(support_failure(
                "GFX-R5 could not resolve the window's current monitor",
            ));
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
            return Err(support_failure(
                "GFX-R5 GetMonitorInfoW failed for the current monitor",
            ));
        }
        Ok((
            info.rcMonitor.left,
            info.rcMonitor.top,
            info.rcMonitor.right,
            info.rcMonitor.bottom,
        ))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MonitorTarget {
    bounds: (i32, i32, i32, i32),
    dpi: u32,
}

pub(crate) fn monitor_targets() -> GfxR5Result<Vec<MonitorTarget>> {
    let display = WindowsDisplay::new();
    let count = display.count().map_err(map_error)?;
    let mut targets = Vec::with_capacity(count.max(0) as usize);
    for index in 0..count {
        let info = display.info(index).map_err(map_error)?;
        let bounds = (
            info.bounds.x.round() as i32,
            info.bounds.y.round() as i32,
            (info.bounds.x + info.bounds.w).round() as i32,
            (info.bounds.y + info.bounds.h).round() as i32,
        );
        let dpi = (info.dpi_scale * 96.0).round() as u32;
        if bounds.2 > bounds.0 && bounds.3 > bounds.1 && dpi > 0 {
            targets.push(MonitorTarget { bounds, dpi });
        }
    }
    Ok(targets)
}

pub(crate) fn move_to_monitor(
    window: &mut NativeWindow,
    monitor: MonitorTarget,
) -> GfxR5Result<(u32, bool, (i32, i32, i32, i32))> {
    window.set_position(monitor.bounds.0 + 32, monitor.bounds.1 + 32)?;
    let actual_bounds = window.monitor_bounds()?;
    let reached = actual_bounds == monitor.bounds;
    Ok((window.dpi()?, reached, actual_bounds))
}

/// Structured evidence for one mixed-DPI transition leg.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DpiTransitionEvidence {
    pub observed_dpi: u32,
    pub logical_resize: Option<(i32, i32)>,
    pub target_monitor_reached: bool,
    pub logical_extent: (i32, i32),
    pub drawable_extent: (i32, i32),
}

/// A monitor topology sample used by the mixed-DPI exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorSample {
    pub bounds: (i32, i32, i32, i32),
    pub dpi_x: u32,
    pub dpi_y: u32,
}

/// Evidence for a real monitor-to-monitor DPI round trip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixedDpiEvidence {
    pub adapter_diagnostic: String,
    pub monitor_samples: Vec<MonitorSample>,
    pub initial: DpiTransitionEvidence,
    pub forward: DpiTransitionEvidence,
    pub return_transition: DpiTransitionEvidence,
}

/// Moves one native window across two real monitors and keeps one logical
/// extent while rebuilding the Vulkan drawable at each observed DPI.
pub fn run_mixed_dpi_transition(window: &mut NativeWindow) -> GfxR5Result<MixedDpiEvidence> {
    let targets = monitor_targets()?;
    let pair = targets.iter().enumerate().find_map(|(index, first)| {
        targets[index + 1..]
            .iter()
            .find(|second| second.bounds != first.bounds && second.dpi != first.dpi)
            .map(|second| (*first, *second))
    });
    let Some((initial_target, forward_target)) = pair else {
        return Err(support_failure(
            "GFX-R5 mixed-DPI requires two real monitors with distinct effective DPIs",
        ));
    };
    const LOGICAL_EXTENT: (i32, i32) = (137, 103);
    let (initial_dpi, initial_reached, initial_bounds) = move_to_monitor(window, initial_target)?;
    let surface = window.surface()?;
    let mut context =
        VulkanContext::new(surface, LOGICAL_EXTENT.0, LOGICAL_EXTENT.1).map_err(map_error)?;
    let evidence = (|| {
        let adapter_diagnostic = context.adapter_info.diagnostic_summary();
        present_solid(&mut context, 0xFF3478BC)?;
        let initial = DpiTransitionEvidence {
            observed_dpi: initial_dpi,
            logical_resize: None,
            target_monitor_reached: initial_reached,
            logical_extent: LOGICAL_EXTENT,
            drawable_extent: (context.width(), context.height()),
        };

        let (forward_dpi, forward_reached, forward_bounds) =
            move_to_monitor(window, forward_target)?;
        context
            .resize(LOGICAL_EXTENT.0, LOGICAL_EXTENT.1)
            .map_err(map_error)?;
        present_solid(&mut context, 0xFF9A5C21)?;
        let forward = DpiTransitionEvidence {
            observed_dpi: forward_dpi,
            logical_resize: Some(LOGICAL_EXTENT),
            target_monitor_reached: forward_reached,
            logical_extent: LOGICAL_EXTENT,
            drawable_extent: (context.width(), context.height()),
        };

        let (return_dpi, return_reached, return_bounds) = move_to_monitor(window, initial_target)?;
        context
            .resize(LOGICAL_EXTENT.0, LOGICAL_EXTENT.1)
            .map_err(map_error)?;
        present_solid(&mut context, 0xFFB7642D)?;
        let return_transition = DpiTransitionEvidence {
            observed_dpi: return_dpi,
            logical_resize: Some(LOGICAL_EXTENT),
            target_monitor_reached: return_reached,
            logical_extent: LOGICAL_EXTENT,
            drawable_extent: (context.width(), context.height()),
        };

        if initial_dpi == forward_dpi || (initial_dpi <= 96 && forward_dpi <= 96) {
            return Err(support_failure(format!(
                "GFX-R5 mixed-DPI observed unusable round trip: initial={initial_dpi}, forward={forward_dpi}"
            )));
        }
        if initial_bounds == forward_bounds || return_bounds != initial_bounds {
            return Err(support_failure(
                "GFX-R5 mixed-DPI did not reach two distinct monitors and return",
            ));
        }
        Ok(MixedDpiEvidence {
            adapter_diagnostic,
            monitor_samples: vec![
                MonitorSample {
                    bounds: initial_bounds,
                    dpi_x: initial_dpi,
                    dpi_y: initial_dpi,
                },
                MonitorSample {
                    bounds: forward_bounds,
                    dpi_x: forward_dpi,
                    dpi_y: forward_dpi,
                },
            ],
            initial,
            forward,
            return_transition,
        })
    })();
    context.try_shutdown().map_err(map_error)?;
    evidence
}

mod evidence;

pub use self::evidence::*;
