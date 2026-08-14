//! 供 tests 目录下集成测试使用的 Windows Vulkan 支持。
//!
//! 本模块刻意不包含测试入口点：原生资源边界保留在库内部，
//! 测试名称、断言与观测仍属于集成测试代码。

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
// 引入 Vulkan 证据路径使用的 PixelUpload 操作、类型化生命周期与提交诊断契约。
use crate::native::present::{
    GraphicsContextLifecycle, PixelUploadSurface, PresentDamage, PresentTestResult,
};
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

/// 仅向 Windows Vulkan 集成测试夹具暴露的类型化失败边界。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GfxR5Error {
    /// 测试断言使用的稳定 UIX 错误类别。
    pub code: &'static str,
    /// 完整原生错误文本，可用时包含 Vulkan 结果码。
    pub message: String,
}

impl fmt::Display for GfxR5Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for GfxR5Error {}

/// GFX-R5 测试支持操作共享的结果类型。
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

// 从单一 PresentSurface 快照读取 GFX-R5 诊断使用的 drawable extent。
pub(crate) fn drawable_extent(context: &dyn PixelUploadSurface) -> (i32, i32) {
    // 一次读取 extent、DPR 与 generation，避免诊断证据由分离查询拼接。
    let surface = context.present_surface();
    // 返回诊断场景需要的物理 drawable 宽高。
    (surface.drawable_width, surface.drawable_height)
}

/// 一个可见的 Windows 窗口，其原生句柄在整个被测 Vulkan surface 生命周期内保持有效。
pub struct NativeWindow {
    // WindowBinding 存储指向 WindowsPlatform 的裸指针；把平台固定在堆上，
    // 使移动本包装器不会使该回调指针失效。
    _platform: Box<WindowsPlatform>,
    window: Box<dyn PlatformWindow>,
    closed: bool,
}

impl NativeWindow {
    /// 创建并显示一个真实的 Win32 窗口。
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

    /// 返回用于创建 Vulkan surface 的 HWND。
    pub fn surface(&self) -> GfxR5Result<*mut c_void> {
        let surface = self.window.native_surface_ptr();
        if surface.is_null() {
            return Err(support_failure("GFX-R5 test window returned a null HWND"));
        }
        Ok(surface)
    }

    /// 调整原生窗口尺寸，不触碰任何 Vulkan context。
    pub fn resize(&mut self, width: i32, height: i32) -> GfxR5Result<()> {
        self.window
            .properties_mut()
            .set_size(width, height)
            .map_err(map_error)
    }

    /// 将原生窗口移动到虚拟桌面的绝对位置。
    pub fn set_position(&mut self, x: i32, y: i32) -> GfxR5Result<()> {
        self.window
            .properties_mut()
            .set_position(x, y)
            .map_err(map_error)
    }

    /// 返回 Win32 报告的每窗口有效 DPI。
    pub fn dpi(&self) -> GfxR5Result<u32> {
        Ok(dpi_for_window(self.surface()?))
    }

    /// 返回当前包含该窗口的监视器的物理边界。
    pub fn monitor_bounds(&self) -> GfxR5Result<(i32, i32, i32, i32)> {
        let hwnd = HWND(self.surface()?);
        // SAFETY：hwnd 由本对象持有的平台窗口提供且尚未销毁（surface() 已校验非空）；
        // MonitorFromWindow 为纯查询，不写调用方内存，返回 NULL 时由下方显式处理。
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
        // SAFETY：monitor 非空（上方已校验）；info 为存活且 cbSize 已初始化为
        // MONITORINFO 大小的输出缓冲，函数最多写入该结构体大小的内容。
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

    /// 关闭原生窗口一次，保留已销毁的 HWND 供显式原生 surface 失败测试使用。
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

/// 一次混合 DPI 迁移段的结构化证据。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DpiTransitionEvidence {
    pub observed_dpi: u32,
    pub logical_resize: Option<(i32, i32)>,
    pub target_monitor_reached: bool,
    pub logical_extent: (i32, i32),
    pub drawable_extent: (i32, i32),
}

/// 混合 DPI 场景使用的监视器拓扑样本。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorSample {
    pub bounds: (i32, i32, i32, i32),
    pub dpi_x: u32,
    pub dpi_y: u32,
}

/// 真实监视器间 DPI 往返的证据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixedDpiEvidence {
    pub adapter_diagnostic: String,
    pub monitor_samples: Vec<MonitorSample>,
    pub initial: DpiTransitionEvidence,
    pub forward: DpiTransitionEvidence,
    pub return_transition: DpiTransitionEvidence,
}

/// 将一个原生窗口在两块真实监视器间迁移，保持同一逻辑尺寸，
/// 并在每次观测到的 DPI 下重建 Vulkan drawable。
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
            drawable_extent: drawable_extent(&context),
        };

        let (forward_dpi, forward_reached, forward_bounds) =
            move_to_monitor(window, forward_target)?;
        // mixed-DPI 诊断显式使用 PixelUpload surface 生命周期。
        context
            // 以相同逻辑 extent 重建目标监视器上的 Vulkan drawable。
            .resize_pixel_upload_surface(LOGICAL_EXTENT.0, LOGICAL_EXTENT.1)
            .map_err(map_error)?;
        present_solid(&mut context, 0xFF9A5C21)?;
        let forward = DpiTransitionEvidence {
            observed_dpi: forward_dpi,
            logical_resize: Some(LOGICAL_EXTENT),
            target_monitor_reached: forward_reached,
            logical_extent: LOGICAL_EXTENT,
            drawable_extent: drawable_extent(&context),
        };

        let (return_dpi, return_reached, return_bounds) = move_to_monitor(window, initial_target)?;
        // 返回初始监视器时继续使用同一 PixelUpload surface 契约。
        context
            // 以相同逻辑 extent 重建返回路径的 Vulkan drawable。
            .resize_pixel_upload_surface(LOGICAL_EXTENT.0, LOGICAL_EXTENT.1)
            .map_err(map_error)?;
        present_solid(&mut context, 0xFFB7642D)?;
        let return_transition = DpiTransitionEvidence {
            observed_dpi: return_dpi,
            logical_resize: Some(LOGICAL_EXTENT),
            target_monitor_reached: return_reached,
            logical_extent: LOGICAL_EXTENT,
            drawable_extent: drawable_extent(&context),
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
