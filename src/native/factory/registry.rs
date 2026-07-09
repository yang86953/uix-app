//! Backend registry types and table-driven context creation (P6.7 M4–M5 / P6.8).

use std::ffi::c_void;

use crate::core::error::{Errc, Error};
use crate::native::traits::present::{GraphicsBackend, IGraphicsContext, PresentMode, RasterMode};

/// Compile-time availability of a registry row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendStatus {
    Active,
    /// Placeholder API — skipped during Auto probe with diagnostic.
    Planned,
    /// Feature-off or platform mismatch — skipped during Auto probe.
    Disabled,
}

/// One graphics API factory row — API identity plus declared raster × present axes.
///
/// Engine assembly still reads live [`IGraphicsContext::caps`]; these fields document
/// the combination this entry is expected to provide ([#169](docs/decisions.md#d169)).
pub struct GraphicsBackendEntry {
    pub id: GraphicsBackend,
    pub priority: u8,
    pub status: BackendStatus,
    /// Declared raster axis for this registry row.
    pub raster: RasterMode,
    /// Declared present axis for this registry row.
    pub present: PresentMode,
    pub create: fn(*mut c_void, i32, i32) -> Result<Box<dyn IGraphicsContext>, Error>,
}

impl GraphicsBackendEntry {
    pub fn is_probe_candidate(&self) -> bool {
        self.status == BackendStatus::Active
    }
}

#[cfg(windows)]
use crate::native::factory::registry_windows::PLATFORM_ENTRIES;
#[cfg(all(unix, not(target_os = "macos")))]
use crate::native::factory::registry_linux::PLATFORM_ENTRIES;
#[cfg(target_os = "macos")]
use crate::native::factory::registry_macos::PLATFORM_ENTRIES;

#[cfg(not(any(windows, all(unix, not(target_os = "macos")), target_os = "macos")))]
pub(crate) const PLATFORM_ENTRIES: &[GraphicsBackendEntry] = &[];

/// All Active rows for the current platform, sorted by descending priority.
pub fn active_entries() -> &'static [GraphicsBackendEntry] {
    PLATFORM_ENTRIES
}

/// Lookup a registry row by backend id.
pub fn entry_for(backend: GraphicsBackend) -> Option<&'static GraphicsBackendEntry> {
    PLATFORM_ENTRIES.iter().find(|entry| entry.id == backend)
}

/// Creates a context for one registry row (no probe loop).
pub fn try_create_context(
    entry: &GraphicsBackendEntry,
    native_surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    if entry.status == BackendStatus::Planned {
        return Err(Error::new(
            Errc::PlatformError,
            format!("GraphicsBackend {} is planned but not implemented", entry.id),
        ));
    }
    if entry.status == BackendStatus::Disabled {
        return Err(Error::new(
            Errc::PlatformError,
            format!("GraphicsBackend {} is disabled in this build", entry.id),
        ));
    }
    let ctx = (entry.create)(native_surface, width, height)?;
    let caps = ctx.caps();
    if caps.raster != entry.raster || caps.present != entry.present {
        let msg = format!(
            "GraphicsBackend {}: context caps {} × {} do not match registry row {} × {}",
            entry.id, caps.raster, caps.present, entry.raster, entry.present
        );
        // Drop without calling shutdown — create failed before ownership transfer
        // to a live engine; contexts that need teardown should still shut down.
        let mut ctx = ctx;
        ctx.shutdown();
        return Err(Error::new(Errc::PlatformError, msg));
    }
    Ok(ctx)
}

/// Ordered probe candidates for a graphics backend request.
pub fn gpu_probe_candidates(requested: GraphicsBackend) -> Vec<GraphicsBackend> {
    match requested {
        GraphicsBackend::Auto => active_entries()
            .iter()
            .filter(|entry| entry.is_probe_candidate())
            .map(|entry| entry.id)
            .collect(),
        backend => vec![backend],
    }
}

/// Creates a single GPU context for one backend (no probe loop).
pub fn try_create_gpu_context(
    backend: GraphicsBackend,
    native_surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    let entry = entry_for(backend).ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            format!("Graphics factory: no registry entry for {backend}"),
        )
    })?;
    try_create_context(entry, native_surface, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_candidates_follow_registry_priority_order() {
        let candidates = gpu_probe_candidates(GraphicsBackend::Auto);
        let entries: Vec<_> = active_entries()
            .iter()
            .filter(|entry| entry.is_probe_candidate())
            .map(|entry| entry.id)
            .collect();
        assert_eq!(candidates, entries);
    }

    #[test]
    fn explicit_backend_probes_only_requested_backend() {
        assert_eq!(
            gpu_probe_candidates(GraphicsBackend::Vulkan),
            vec![GraphicsBackend::Vulkan]
        );
    }

    #[test]
    fn planned_backend_is_skipped_in_auto_chain() {
        let candidates = gpu_probe_candidates(GraphicsBackend::Auto);
        assert!(!candidates.contains(&GraphicsBackend::D3d12));
    }

    #[test]
    fn auto_backend_uses_platform_default_order() {
        let candidates = gpu_probe_candidates(GraphicsBackend::Auto);

        #[cfg(windows)]
        assert_eq!(
            candidates,
            vec![GraphicsBackend::D3d11, GraphicsBackend::OpenGlEs]
        );

        #[cfg(all(unix, not(target_os = "macos")))]
        assert_eq!(
            candidates,
            vec![GraphicsBackend::Vulkan, GraphicsBackend::OpenGlEs]
        );

        #[cfg(target_os = "macos")]
        assert_eq!(candidates, vec![GraphicsBackend::Metal]);
    }

    #[test]
    fn registry_rows_declare_orthogonal_axes() {
        for entry in active_entries() {
            match entry.id {
                GraphicsBackend::OpenGlEs => {
                    assert_eq!(entry.raster, RasterMode::GpuNative);
                    assert_eq!(entry.present, PresentMode::Swapchain);
                }
                GraphicsBackend::D3d11 => {
                    assert_eq!(entry.raster, RasterMode::GpuNative);
                    assert_eq!(entry.present, PresentMode::Swapchain);
                }
                GraphicsBackend::Vulkan | GraphicsBackend::Metal => {
                    assert_eq!(entry.raster, RasterMode::Cpu);
                    assert_eq!(entry.present, PresentMode::PixelUpload);
                }
                GraphicsBackend::D3d12 | GraphicsBackend::Auto => {}
            }
        }
    }
}
