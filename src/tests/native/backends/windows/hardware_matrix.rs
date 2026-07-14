use std::collections::BTreeSet;
use std::fmt;
use std::time::{Duration, Instant};

use crate::native::backends::windows::dpi::{dpi_for_window, logical_extent_to_physical};
use crate::native::backends::windows::platform::WindowsPlatform;
use crate::native::graphics::platform::windows::drawable_size;
use crate::native::shared::OsEventSource;
use crate::native::traits::{IWindowManager, PlatformWindow};
use windows::core::BOOL;
use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, HDC, HMONITOR};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

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
    bounds: MonitorBounds,
    dpi_x: u32,
    dpi_y: u32,
}

impl MonitorDpiSample {
    const fn new(bounds: MonitorBounds, dpi_x: u32, dpi_y: u32) -> Self {
        Self {
            bounds,
            dpi_x,
            dpi_y,
        }
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
    DpiQueryFailed { bounds: MonitorBounds },
}

impl fmt::Display for MonitorInventoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EnumerationFailed => write!(formatter, "Win32 monitor enumeration failed"),
            Self::MissingBounds => write!(formatter, "Win32 monitor callback omitted bounds"),
            Self::DpiQueryFailed { bounds } => {
                write!(formatter, "Win32 DPI query failed for monitor {bounds:?}")
            }
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

    fn mixed_pair(&self) -> Option<(MonitorDpiSample, MonitorDpiSample)> {
        self.monitors.iter().copied().find_map(|first| {
            self.monitors
                .iter()
                .copied()
                .find(|second| (first.dpi_x, first.dpi_y) != (second.dpi_x, second.dpi_y))
                .map(|second| (first, second))
        })
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
    monitors: Vec<MonitorDpiSample>,
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
    let mut dpi_x = 0_u32;
    let mut dpi_y = 0_u32;
    // SAFETY: monitor 来自当前枚举；dpi 输出指针指向本栈帧内的已初始化 u32。
    if unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }.is_err() {
        inventory.error = Some(MonitorInventoryError::DpiQueryFailed { bounds });
        return BOOL(0);
    }
    inventory
        .monitors
        .push(MonitorDpiSample::new(bounds, dpi_x, dpi_y));
    BOOL(1)
}

fn enumerate_monitor_topology() -> Result<GfxR5MonitorTopology, MonitorInventoryError> {
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
    Ok(GfxR5MonitorTopology::new(inventory.monitors))
}

fn wait_for_window_dpi(
    platform: &mut WindowsPlatform,
    window: &dyn PlatformWindow,
    expected_dpi: u32,
) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        let _ = platform.dispatch_timeout(Duration::from_millis(10));
        while platform.next_event().is_some() {}
        if dpi_for_window(window.native_handle().native_window()) == expected_dpi {
            return true;
        }
    }
    false
}

fn sample(dpi: u32, left: i32) -> MonitorDpiSample {
    MonitorDpiSample::new(
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
fn windows_monitor_inventory_reports_valid_samples() {
    let topology = enumerate_monitor_topology().expect("enumerate Win32 monitor topology");
    assert!(!topology.monitors.is_empty());
    assert!(topology.monitors.iter().all(|monitor| {
        monitor.bounds.right > monitor.bounds.left
            && monitor.bounds.bottom > monitor.bounds.top
            && monitor.dpi_x >= BASE_DPI
            && monitor.dpi_y >= BASE_DPI
    }));
}

#[test]
#[ignore = "requires two Windows monitors with mixed DPI and at least one scale above 100%"]
fn windows_gfx_r5_window_crosses_real_mixed_dpi_monitors() {
    let topology = enumerate_monitor_topology().expect("enumerate Win32 monitor topology");
    topology
        .validate()
        .unwrap_or_else(|gap| panic!("{gap}; topology={}", topology.diagnostic_summary()));
    let (first, second) = topology.mixed_pair().expect("validated mixed-DPI pair");

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

    for monitor in [first, second, first] {
        let (x, y) = monitor.bounds.centered_origin();
        window
            .properties_mut()
            .set_position(x, y)
            .expect("move window to target monitor");
        assert!(
            wait_for_window_dpi(&mut platform, window.as_ref(), monitor.dpi_x),
            "window did not adopt target DPI {}; topology={}",
            monitor.dpi_x,
            topology.diagnostic_summary()
        );

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
    }

    println!(
        "GFX-R5 mixed-DPI topology: {}",
        topology.diagnostic_summary()
    );
    window.close().expect("close native window");
}
