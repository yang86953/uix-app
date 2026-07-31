//! Backend registry types and table-driven context creation (P6.7 M4–M5 / P6.8).

use std::ffi::c_void;

use crate::core::error::{Errc, Error};
use crate::diagnostics::PendingFailureQueue;
use crate::native::factory::thread_bound::bind_to_current_thread;
use crate::native::traits::present::{
    GraphicsBackend, IGraphicsContext, NativeSurfaceHandle, PresentMode, RasterMode,
};

/// One probeable graphics configuration.
///
/// A backend identity is diagnostic data only: one API may expose multiple
/// legal raster × present combinations, each of which must be probed and
/// validated independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphicsRecipe {
    pub backend: GraphicsBackend,
    pub raster: RasterMode,
    pub present: PresentMode,
}

impl GraphicsRecipe {
    pub const fn new(backend: GraphicsBackend, raster: RasterMode, present: PresentMode) -> Self {
        Self {
            backend,
            raster,
            present,
        }
    }
}

impl std::fmt::Display for GraphicsRecipe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "backend={}; raster={}; present={}",
            self.backend, self.raster, self.present
        )
    }
}

/// Compile-time availability of a registry row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendStatus {
    Active,
    /// Placeholder API — skipped during Auto probe with diagnostic.
    Planned,
    /// Feature-off or platform mismatch — skipped during Auto probe.
    Disabled,
}

pub(crate) type GraphicsContextFactory =
    fn(*mut c_void, i32, i32, PendingFailureQueue) -> Result<Box<dyn IGraphicsContext>, Error>;

/// One graphics API factory row — API identity plus declared raster × present axes.
///
/// Engine assembly still reads live [`IGraphicsContext::caps`]; these fields document
/// the combination this entry is expected to provide ([架构 · 图形](docs/架构.md#图形-api与帧提交硬约束)).
pub struct GraphicsBackendEntry {
    pub id: GraphicsBackend,
    pub priority: u8,
    pub status: BackendStatus,
    /// Declared raster axis for this registry row.
    pub raster: RasterMode,
    /// Declared present axis for this registry row.
    pub present: PresentMode,
    /// Construction stays inside native factory routing so every context is
    /// recipe-validated and can later receive the shared thread-affinity
    /// binding. External callers must use `try_create_gpu_recipe` instead of
    /// bypassing the lifecycle contract through a raw function pointer.
    pub(crate) create: GraphicsContextFactory,
}

impl GraphicsBackendEntry {
    pub const fn recipe(&self) -> GraphicsRecipe {
        GraphicsRecipe::new(self.id, self.raster, self.present)
    }

    pub fn is_probe_candidate(&self) -> bool {
        self.status == BackendStatus::Active
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
use crate::native::factory::registry_linux::PLATFORM_ENTRIES;
#[cfg(target_os = "macos")]
use crate::native::factory::registry_macos::PLATFORM_ENTRIES;
#[cfg(windows)]
use crate::native::factory::registry_windows::PLATFORM_ENTRIES;

#[cfg(not(any(windows, all(unix, not(target_os = "macos")), target_os = "macos")))]
pub(crate) const PLATFORM_ENTRIES: &[GraphicsBackendEntry] = &[];

/// All registry rows for the current platform in declaration order.
pub fn active_entries() -> &'static [GraphicsBackendEntry] {
    PLATFORM_ENTRIES
}

/// Lookup a registry row by backend id.
pub fn entry_for(backend: GraphicsBackend) -> Option<&'static GraphicsBackendEntry> {
    PLATFORM_ENTRIES.iter().find(|entry| entry.id == backend)
}

/// Looks up one exact recipe row.
pub fn entry_for_recipe(recipe: GraphicsRecipe) -> Option<&'static GraphicsBackendEntry> {
    PLATFORM_ENTRIES
        .iter()
        .find(|entry| entry.recipe() == recipe)
}

/// Creates a context for one registry row (no probe loop).
pub(crate) fn try_create_context(
    entry: &GraphicsBackendEntry,
    native_surface: *mut c_void,
    width: i32,
    height: i32,
    pending_failures: PendingFailureQueue,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    if entry.status == BackendStatus::Planned {
        return Err(Error::new(
            Errc::PlatformError,
            format!(
                "GraphicsBackend {} is planned but not implemented",
                entry.id
            ),
        ));
    }
    if entry.status == BackendStatus::Disabled {
        return Err(Error::new(
            Errc::PlatformError,
            format!("GraphicsBackend {} is disabled in this build", entry.id),
        ));
    }
    let ctx = (entry.create)(native_surface, width, height, pending_failures)?;
    let caps = ctx.caps();
    let actual = GraphicsRecipe::new(caps.backend, caps.raster, caps.present);
    let expected = entry.recipe();
    if actual != expected {
        let msg = format!("Graphics recipe {expected}: context reported {actual}");
        // Creation succeeded, so a context whose live caps do not match the
        // registry row must be shut down before returning the mismatch.
        let mut ctx = ctx;
        ctx.try_shutdown()?;
        return Err(Error::new(Errc::PlatformError, msg));
    }
    Ok(bind_to_current_thread(ctx))
}

/// Ordered probe candidates for a backend request, with one item per recipe
/// row.  An explicit backend request deliberately retains every active recipe
/// for that backend instead of selecting the first matching row.
fn matches_probe_request(entry: &GraphicsBackendEntry, requested: GraphicsBackend) -> bool {
    if requested == GraphicsBackend::Auto {
        entry.is_probe_candidate()
    } else {
        entry.id == requested
    }
}

pub(crate) fn active_recipes_by_priority(
    entries: &[GraphicsBackendEntry],
    requested: GraphicsBackend,
) -> Vec<GraphicsRecipe> {
    let mut entries = entries
        .iter()
        .filter(|entry| matches_probe_request(entry, requested))
        .collect::<Vec<_>>();
    // GPU-native raster is the primary rendering tier.  A CPU raster recipe
    // may still use a GPU for presentation, but it must remain behind every
    // usable native raster candidate so Auto never selects PixelUpload merely
    // because that API has a higher platform priority. Stable sort preserves
    // declaration order for equal priorities inside the same tier.
    entries.sort_by_key(|entry| {
        (
            entry.raster != RasterMode::GpuNative,
            std::cmp::Reverse(entry.priority),
        )
    });
    entries
        .into_iter()
        .map(GraphicsBackendEntry::recipe)
        .collect()
}

/// Runtime platform label used by graphics bootstrap diagnostics.
pub fn graphics_runtime_platform() -> &'static str {
    #[cfg(windows)]
    {
        "windows"
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        "linux"
    }
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
    #[cfg(not(any(windows, all(unix, not(target_os = "macos")), target_os = "macos")))]
    {
        "unknown"
    }
}

/// Recipe-level probe candidates used by graphics bootstrap.
pub fn gpu_recipe_candidates(requested: GraphicsBackend) -> Vec<GraphicsRecipe> {
    active_recipes_by_priority(active_entries(), requested)
}

/// Describes why an explicit backend request has no active registry row.
pub fn describe_backend_availability(requested: GraphicsBackend) -> Option<&'static str> {
    if requested == GraphicsBackend::Auto {
        return None;
    }
    match entry_for(requested) {
        Some(entry) => match entry.status {
            BackendStatus::Active => None,
            BackendStatus::Planned => Some("planned but not implemented on this platform"),
            BackendStatus::Disabled => Some("disabled in this build"),
        },
        None => Some("not registered for this platform"),
    }
}

/// Legacy backend-only view of recipe candidates.
///
/// This is intentionally lossy and must not drive bootstrap: repeated backend
/// values represent distinct recipe rows.
pub fn gpu_probe_candidates(requested: GraphicsBackend) -> Vec<GraphicsBackend> {
    gpu_recipe_candidates(requested)
        .into_iter()
        .map(|recipe| recipe.backend)
        .collect()
}

/// Creates a context for one exact recipe row (no probe loop).
pub fn try_create_gpu_recipe(
    recipe: GraphicsRecipe,
    native_surface: NativeSurfaceHandle,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    try_create_gpu_recipe_with_queue(
        recipe,
        native_surface,
        width,
        height,
        PendingFailureQueue::new(),
    )
}

/// Creates one exact recipe context using the runtime-scoped callback queue.
pub(crate) fn try_create_gpu_recipe_with_queue(
    recipe: GraphicsRecipe,
    native_surface: NativeSurfaceHandle,
    width: i32,
    height: i32,
    pending_failures: PendingFailureQueue,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    let entry = entry_for_recipe(recipe).ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            format!("Graphics factory: no registry entry for recipe {recipe}"),
        )
    })?;
    // Only the native factory bridge unwraps the opaque surface handle before
    // it reaches an API/platform constructor.
    try_create_context(
        entry,
        native_surface.as_raw(),
        width,
        height,
        pending_failures,
    )
}

/// Creates a single GPU context for one backend (compatibility helper).
///
/// New bootstrap code must use [`try_create_gpu_recipe`] so it can continue to
/// later recipe rows for the same API.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn try_create_gpu_context(
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
    try_create_context(
        entry,
        native_surface,
        width,
        height,
        PendingFailureQueue::new(),
    )
}
