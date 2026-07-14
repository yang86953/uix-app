use std::collections::BTreeSet;
use std::fmt;

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
