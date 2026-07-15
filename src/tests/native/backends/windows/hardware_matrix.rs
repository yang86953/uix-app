use std::collections::BTreeSet;
use std::fmt;
use std::time::{Duration, Instant};

use crate::native::backends::windows::dpi::{dpi_for_window, logical_extent_to_physical};
use crate::native::backends::windows::platform::WindowsPlatform;
use crate::native::graphics::platform::windows::{drawable_size, DrawableSize};
#[cfg(feature = "vulkan")]
use crate::native::graphics::vulkan::platform::context::VulkanContext;
use crate::native::shared::OsEventSource;
#[cfg(feature = "vulkan")]
use crate::native::traits::{IGraphicsContext, PresentDamage};
use crate::native::traits::{IWindowManager, PlatformWindow, UiEventPayload, UiEventType};
#[cfg(feature = "vulkan")]
use crate::tests::native::gfx_r5::expected_gfx_r5_vendor;
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, MonitorFromWindow, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST,
};

const BASE_DPI: u32 = 96;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MonitorBounds {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl MonitorBounds {
    const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    fn centered_origin(self) -> (i32, i32) {
        (
            self.left.saturating_add(self.right).saturating_div(2) - 200,
            self.top.saturating_add(self.bottom).saturating_div(2) - 150,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MonitorDpiSample {
    handle: isize,
    bounds: MonitorBounds,
    dpi_x: u32,
    dpi_y: u32,
}

impl MonitorDpiSample {
    const fn new(handle: isize, bounds: MonitorBounds, dpi_x: u32, dpi_y: u32) -> Self {
        Self {
            handle,
            bounds,
            dpi_x,
            dpi_y,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MonitorInventoryEntry {
    handle: isize,
    bounds: MonitorBounds,
}

impl MonitorInventoryEntry {
    const fn new(handle: isize, bounds: MonitorBounds) -> Self {
        Self { handle, bounds }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum GfxR5TopologyGap {
    InsufficientMonitors { found: usize },
    NoScaledMonitor,
    UniformDpi { dpi_x: u32, dpi_y: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MonitorInventoryError {
    EnumerationFailed,
    MissingBounds,
}

impl fmt::Display for MonitorInventoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EnumerationFailed => write!(formatter, "Win32 monitor enumeration failed"),
            Self::MissingBounds => write!(formatter, "Win32 monitor callback omitted bounds"),
        }
    }
}

impl fmt::Display for GfxR5TopologyGap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InsufficientMonitors { found } => {
                write!(
                    formatter,
                    "GFX-R5 requires at least two monitors; found {found}"
                )
            }
            Self::NoScaledMonitor => write!(
                formatter,
                "GFX-R5 requires at least one monitor above 100% scaling"
            ),
            Self::UniformDpi { dpi_x, dpi_y } => write!(
                formatter,
                "GFX-R5 requires mixed DPI; every monitor reports {dpi_x}x{dpi_y}"
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GfxR5MonitorTopology {
    monitors: Vec<MonitorDpiSample>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DpiTransitionEvidence {
    observed_dpi: u32,
    logical_resize: Option<(i32, i32)>,
    target_monitor_reached: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DpiTransitionTimeout {
    expected_dpi: u32,
    observed_dpi: u32,
    target_monitor_reached: bool,
    expected_logical_resize: Option<(i32, i32)>,
    observed_logical_resize: Option<(i32, i32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MonitorDpiSamplingTimeout {
    bounds: MonitorBounds,
    target_reached: bool,
    observed_dpi: u32,
}

impl fmt::Display for MonitorDpiSamplingTimeout {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "monitor DPI sampling timed out: bounds={:?}, target_reached={}, observed_dpi={}",
            self.bounds, self.target_reached, self.observed_dpi
        )
    }
}

impl fmt::Display for DpiTransitionTimeout {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "DPI transition timed out: expected_dpi={}, observed_dpi={}, target_monitor_reached={}, expected_logical_resize={:?}, observed_logical_resize={:?}",
            self.expected_dpi,
            self.observed_dpi,
            self.target_monitor_reached,
            self.expected_logical_resize,
            self.observed_logical_resize
        )
    }
}

impl GfxR5MonitorTopology {
    fn new(monitors: Vec<MonitorDpiSample>) -> Self {
        Self { monitors }
    }

    fn validate(&self) -> Result<(), GfxR5TopologyGap> {
        if self.monitors.len() < 2 {
            return Err(GfxR5TopologyGap::InsufficientMonitors {
                found: self.monitors.len(),
            });
        }
        if !self
            .monitors
            .iter()
            .any(|monitor| monitor.dpi_x > BASE_DPI || monitor.dpi_y > BASE_DPI)
        {
            return Err(GfxR5TopologyGap::NoScaledMonitor);
        }

        let distinct_dpi: BTreeSet<_> = self
            .monitors
            .iter()
            .map(|monitor| (monitor.dpi_x, monitor.dpi_y))
            .collect();
        if distinct_dpi.len() == 1 {
            let (dpi_x, dpi_y) = distinct_dpi
                .first()
                .copied()
                .unwrap_or((BASE_DPI, BASE_DPI));
            return Err(GfxR5TopologyGap::UniformDpi { dpi_x, dpi_y });
        }
        Ok(())
    }

    fn transition_pair(&self) -> Option<(MonitorDpiSample, MonitorDpiSample)> {
        let lower = self
            .monitors
            .iter()
            .copied()
            .min_by_key(|monitor| (monitor.dpi_x, monitor.dpi_y))?;
        let upper = self
            .monitors
            .iter()
            .copied()
            .max_by_key(|monitor| (monitor.dpi_x, monitor.dpi_y))?;
        ((lower.dpi_x, lower.dpi_y) != (upper.dpi_x, upper.dpi_y)).then_some((lower, upper))
    }

    fn diagnostic_summary(&self) -> String {
        self.monitors
            .iter()
            .map(|monitor| {
                format!(
                    "bounds=({},{}..{},{}),dpi={}x{}",
                    monitor.bounds.left,
                    monitor.bounds.top,
                    monitor.bounds.right,
                    monitor.bounds.bottom,
                    monitor.dpi_x,
                    monitor.dpi_y
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    }
}

#[derive(Default)]
struct MonitorInventory {
    monitors: Vec<MonitorInventoryEntry>,
    error: Option<MonitorInventoryError>,
}

unsafe extern "system" fn collect_monitor(
    monitor: HMONITOR,
    _device_context: HDC,
    bounds: *mut RECT,
    inventory: LPARAM,
) -> BOOL {
    if inventory.0 == 0 {
        return BOOL(0);
    }
    // SAFETY: `EnumDisplayMonitors` 在同步回调期间传回调用方提供的独占 inventory 指针。
    let inventory = unsafe { &mut *(inventory.0 as *mut MonitorInventory) };
    if bounds.is_null() {
        inventory.error = Some(MonitorInventoryError::MissingBounds);
        return BOOL(0);
    }
    // SAFETY: Win32 保证回调期间 bounds 指向有效 RECT；此处仅复制值。
    let bounds = unsafe { *bounds };
    let bounds = MonitorBounds::new(bounds.left, bounds.top, bounds.right, bounds.bottom);
    inventory
        .monitors
        .push(MonitorInventoryEntry::new(monitor.0 as isize, bounds));
    BOOL(1)
}

fn enumerate_monitor_inventory() -> Result<Vec<MonitorInventoryEntry>, MonitorInventoryError> {
    let mut inventory = MonitorInventory::default();
    let inventory_ptr = (&mut inventory as *mut MonitorInventory) as isize;
    // SAFETY: 回调与 inventory 均在本函数返回前同步完成，LPARAM 指针在整个枚举期间有效。
    let completed =
        unsafe { EnumDisplayMonitors(None, None, Some(collect_monitor), LPARAM(inventory_ptr)) };
    if let Some(error) = inventory.error {
        return Err(error);
    }
    if !completed.as_bool() {
        return Err(MonitorInventoryError::EnumerationFailed);
    }
    Ok(inventory.monitors)
}

fn monitor_inventory_summary(monitors: &[MonitorInventoryEntry]) -> String {
    monitors
        .iter()
        .map(|monitor| {
            format!(
                "bounds=({},{}..{},{})",
                monitor.bounds.left,
                monitor.bounds.top,
                monitor.bounds.right,
                monitor.bounds.bottom,
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// 用真实 Per-Monitor V2 HWND 采样目标 monitor；避免枚举 API 按调用线程 awareness 虚拟化 DPI。
fn sample_window_monitor_dpi(
    platform: &mut WindowsPlatform,
    window: &mut dyn PlatformWindow,
    monitor: MonitorInventoryEntry,
) -> Result<u32, MonitorDpiSamplingTimeout> {
    let (x, y) = monitor.bounds.centered_origin();
    window
        .properties_mut()
        .set_position(x, y)
        .expect("move window while sampling monitor DPI");

    let deadline = Instant::now() + Duration::from_secs(2);
    let native_window = window.native_handle().native_window();
    let mut target_reached = false;
    let mut observed_dpi = dpi_for_window(native_window);
    while Instant::now() < deadline {
        let _ = platform.dispatch_timeout(Duration::from_millis(10));
        while platform.next_event().is_some() {}
        // SAFETY: native_window 属于仍存活的测试窗口；MonitorFromWindow 只读取句柄。
        let observed_monitor =
            unsafe { MonitorFromWindow(HWND(native_window), MONITOR_DEFAULTTONEAREST) };
        target_reached = observed_monitor.0 as isize == monitor.handle;
        observed_dpi = dpi_for_window(native_window);
        if target_reached {
            return Ok(observed_dpi);
        }
    }
    Err(MonitorDpiSamplingTimeout {
        bounds: monitor.bounds,
        target_reached,
        observed_dpi,
    })
}

fn sample_monitor_topology(
    platform: &mut WindowsPlatform,
    window: &mut dyn PlatformWindow,
    inventory: &[MonitorInventoryEntry],
) -> Result<GfxR5MonitorTopology, MonitorDpiSamplingTimeout> {
    let mut monitors = Vec::with_capacity(inventory.len());
    for monitor in inventory {
        let dpi = sample_window_monitor_dpi(platform, window, *monitor)?;
        monitors.push(MonitorDpiSample::new(
            monitor.handle,
            monitor.bounds,
            dpi,
            dpi,
        ));
    }
    Ok(GfxR5MonitorTopology::new(monitors))
}

fn wait_for_window_dpi(
    platform: &mut WindowsPlatform,
    window: &dyn PlatformWindow,
    expected_monitor: isize,
    expected_dpi: u32,
    expected_logical_resize: Option<(i32, i32)>,
) -> Result<DpiTransitionEvidence, DpiTransitionTimeout> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut observed_dpi = dpi_for_window(window.native_handle().native_window());
    let mut observed_logical_resize = None;
    let mut target_monitor_reached = false;
    while Instant::now() < deadline {
        let _ = platform.dispatch_timeout(Duration::from_millis(10));
        while let Some(event) = platform.next_event() {
            if event.window_id != Some(window.window_id())
                || event.type_ != UiEventType::WindowResize
            {
                continue;
            }
            let UiEventPayload::Resize(resize) = event.payload else {
                continue;
            };
            observed_logical_resize = Some((resize.width, resize.height));
        }
        let native_window = window.native_handle().native_window();
        observed_dpi = dpi_for_window(native_window);
        // SAFETY: native_window 属于仍存活的测试窗口；MonitorFromWindow 只读取句柄。
        let observed_monitor =
            unsafe { MonitorFromWindow(HWND(native_window), MONITOR_DEFAULTTONEAREST) };
        target_monitor_reached = observed_monitor.0 as isize == expected_monitor;
        let resize_matches = expected_logical_resize
            .is_none_or(|expected| observed_logical_resize == Some(expected));
        if target_monitor_reached && observed_dpi == expected_dpi && resize_matches {
            return Ok(DpiTransitionEvidence {
                observed_dpi,
                logical_resize: observed_logical_resize,
                target_monitor_reached,
            });
        }
    }
    Err(DpiTransitionTimeout {
        expected_dpi,
        observed_dpi,
        target_monitor_reached,
        expected_logical_resize,
        observed_logical_resize,
    })
}

fn place_window_on_monitor(
    platform: &mut WindowsPlatform,
    window: &mut dyn PlatformWindow,
    monitor: MonitorDpiSample,
    logical_size: (i32, i32),
    expected_resize: Option<(i32, i32)>,
    topology_summary: &str,
) -> (DpiTransitionEvidence, DrawableSize) {
    while platform.next_event().is_some() {}
    let (x, y) = monitor.bounds.centered_origin();
    window
        .properties_mut()
        .set_position(x, y)
        .expect("move window to target monitor");
    let evidence = wait_for_window_dpi(
        platform,
        window,
        monitor.handle,
        monitor.dpi_x,
        expected_resize,
    )
    .unwrap_or_else(|error| panic!("{error}; topology={topology_summary}"));

    assert_eq!(
        (window.properties().width(), window.properties().height()),
        logical_size,
        "logical client extent must survive a real monitor transition"
    );
    let drawable = drawable_size(
        window.native_handle().native_window(),
        logical_size.0,
        logical_size.1,
    );
    assert_eq!(
        (drawable.logical_width, drawable.logical_height),
        logical_size
    );
    assert_eq!(
        (drawable.width, drawable.height),
        (
            logical_extent_to_physical(logical_size.0, monitor.dpi_x),
            logical_extent_to_physical(logical_size.1, monitor.dpi_y),
        )
    );
    (evidence, drawable)
}

#[cfg(feature = "vulkan")]
fn present_mixed_dpi_frame(context: &mut VulkanContext, drawable: DrawableSize, color: u32) {
    context
        .resize(drawable.logical_width, drawable.logical_height)
        .expect("resize Vulkan surface after DPI transition");
    assert_eq!(
        (context.width(), context.height()),
        (drawable.width, drawable.height),
        "Vulkan extent must follow the physical drawable extent"
    );
    let pixels = vec![color; (context.width() * context.height()) as usize];
    context
        .present_pixels(
            &pixels,
            context.width(),
            context.height(),
            PresentDamage::Full,
        )
        .expect("present after DPI transition");
    assert_eq!(
        context
            .read_pixels(context.width() - 1, context.height() - 1, 1, 1)
            .expect("far-corner readback after DPI transition"),
        vec![color]
    );
}

fn sample(dpi: u32, left: i32) -> MonitorDpiSample {
    MonitorDpiSample::new(
        left as isize,
        MonitorBounds::new(left, 0, left.saturating_add(1920), 1080),
        dpi,
        dpi,
    )
}

#[test]
fn gfx_r5_topology_requires_two_monitors() {
    assert_eq!(
        GfxR5MonitorTopology::new(vec![sample(BASE_DPI, 0)]).validate(),
        Err(GfxR5TopologyGap::InsufficientMonitors { found: 1 })
    );
}

#[test]
fn gfx_r5_topology_requires_a_scaled_monitor() {
    assert_eq!(
        GfxR5MonitorTopology::new(vec![sample(BASE_DPI, 0), sample(BASE_DPI, 1920)]).validate(),
        Err(GfxR5TopologyGap::NoScaledMonitor)
    );
}

#[test]
fn gfx_r5_topology_rejects_uniform_scaled_dpi() {
    assert_eq!(
        GfxR5MonitorTopology::new(vec![sample(144, 0), sample(144, 1920)]).validate(),
        Err(GfxR5TopologyGap::UniformDpi {
            dpi_x: 144,
            dpi_y: 144,
        })
    );
}

#[test]
fn gfx_r5_topology_accepts_scaled_mixed_dpi() {
    assert_eq!(
        GfxR5MonitorTopology::new(vec![sample(BASE_DPI, 0), sample(144, 1920)]).validate(),
        Ok(())
    );
}

#[test]
fn gfx_r5_transition_pair_uses_the_widest_available_dpi_span() {
    let topology = GfxR5MonitorTopology::new(vec![
        sample(144, 1920),
        sample(120, 3840),
        sample(BASE_DPI, 0),
    ]);

    let (lower, upper) = topology
        .transition_pair()
        .expect("mixed topology must have a transition pair");

    assert_eq!((lower.dpi_x, lower.dpi_y), (BASE_DPI, BASE_DPI));
    assert_eq!((upper.dpi_x, upper.dpi_y), (144, 144));
}

#[test]
fn windows_monitor_inventory_reports_valid_bounds() {
    let inventory = enumerate_monitor_inventory().expect("enumerate Win32 monitor inventory");
    assert!(!inventory.is_empty());
    assert!(inventory.iter().all(|monitor| {
        monitor.bounds.right > monitor.bounds.left && monitor.bounds.bottom > monitor.bounds.top
    }));
}

#[test]
fn windows_monitor_dpi_sampling_uses_a_live_per_monitor_window() {
    let inventory = enumerate_monitor_inventory().expect("enumerate Win32 monitor inventory");
    let inventory_summary = monitor_inventory_summary(&inventory);
    let logical_size = (120, 80);
    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window("UIX monitor DPI sampling", logical_size.0, logical_size.1)
        .expect("native window");
    window.show().expect("show native window");

    let topology = sample_monitor_topology(&mut platform, window.as_mut(), &inventory)
        .unwrap_or_else(|error| panic!("{error}; inventory={inventory_summary}"));
    assert_eq!(topology.monitors.len(), inventory.len());
    assert!(topology.monitors.iter().all(|monitor| {
        monitor.dpi_x >= BASE_DPI && monitor.dpi_y >= BASE_DPI && monitor.dpi_x == monitor.dpi_y
    }));

    window.close().expect("close native window");
}

#[test]
#[ignore = "requires two Windows monitors with mixed DPI and at least one scale above 100%"]
fn windows_gfx_r5_window_crosses_real_mixed_dpi_monitors() {
    let logical_size = (321, 219);
    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window(
            "UIX GFX-R5 mixed-DPI transition",
            logical_size.0,
            logical_size.1,
        )
        .expect("native window");
    window.show().expect("show native window");

    let inventory = enumerate_monitor_inventory().expect("enumerate Win32 monitor inventory");
    let inventory_summary = monitor_inventory_summary(&inventory);
    let topology = sample_monitor_topology(&mut platform, window.as_mut(), &inventory)
        .unwrap_or_else(|error| panic!("{error}; inventory={inventory_summary}"));
    topology
        .validate()
        .unwrap_or_else(|gap| panic!("{gap}; topology={}", topology.diagnostic_summary()));
    let (first, second) = topology
        .transition_pair()
        .expect("validated mixed-DPI pair");

    let topology_summary = topology.diagnostic_summary();
    let (initial, _) = place_window_on_monitor(
        &mut platform,
        window.as_mut(),
        first,
        logical_size,
        None,
        &topology_summary,
    );
    let (forward, _) = place_window_on_monitor(
        &mut platform,
        window.as_mut(),
        second,
        logical_size,
        Some(logical_size),
        &topology_summary,
    );
    let (return_trip, _) = place_window_on_monitor(
        &mut platform,
        window.as_mut(),
        first,
        logical_size,
        Some(logical_size),
        &topology_summary,
    );

    println!(
        "GFX-R5 mixed-DPI topology: {topology_summary}; initial={initial:?}; forward={forward:?}; return={return_trip:?}",
    );
    window.close().expect("close native window");
}

#[cfg(feature = "vulkan")]
#[test]
#[ignore = "requires a Vulkan-capable Windows driver, UIX_GFX_R5_EXPECT_VENDOR, and two mixed-DPI monitors with one above 100%"]
fn windows_vulkan_gfx_r5_crosses_real_mixed_dpi_monitors() {
    let expected = expected_gfx_r5_vendor().unwrap_or_else(|error| panic!("{error}"));
    let logical_size = (321, 219);
    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window(
            "UIX GFX-R5 Vulkan mixed-DPI transition",
            logical_size.0,
            logical_size.1,
        )
        .expect("native window");
    window.show().expect("show native window");

    let inventory = enumerate_monitor_inventory().expect("enumerate Win32 monitor inventory");
    let inventory_summary = monitor_inventory_summary(&inventory);
    let topology = sample_monitor_topology(&mut platform, window.as_mut(), &inventory)
        .unwrap_or_else(|error| panic!("{error}; inventory={inventory_summary}"));
    topology
        .validate()
        .unwrap_or_else(|gap| panic!("{gap}; topology={}", topology.diagnostic_summary()));
    let (first, second) = topology
        .transition_pair()
        .expect("validated mixed-DPI pair");
    let topology_summary = topology.diagnostic_summary();

    let (initial, initial_drawable) = place_window_on_monitor(
        &mut platform,
        window.as_mut(),
        first,
        logical_size,
        None,
        &topology_summary,
    );
    let mut context =
        VulkanContext::new(window.native_surface_ptr(), logical_size.0, logical_size.1)
            .expect("Vulkan context on the initial monitor");
    expected.assert_runtime(
        &context.adapter_info,
        context.swapchain_maintenance1_enabled_for_test(),
    );
    present_mixed_dpi_frame(&mut context, initial_drawable, 0xFF34_78BC);

    let (forward, forward_drawable) = place_window_on_monitor(
        &mut platform,
        window.as_mut(),
        second,
        logical_size,
        Some(logical_size),
        &topology_summary,
    );
    present_mixed_dpi_frame(&mut context, forward_drawable, 0xFF9A_5C21);

    let (return_trip, return_drawable) = place_window_on_monitor(
        &mut platform,
        window.as_mut(),
        first,
        logical_size,
        Some(logical_size),
        &topology_summary,
    );
    present_mixed_dpi_frame(&mut context, return_drawable, 0xFF52_7193);

    println!(
        "GFX-R5 Vulkan mixed-DPI evidence: expected={}; {topology_summary}; initial={initial:?}; forward={forward:?}; return={return_trip:?}; {}",
        expected.label(),
        context.adapter_info.diagnostic_summary()
    );
    context.try_shutdown().expect("shutdown Vulkan context");
    window.close().expect("close native window");
}
