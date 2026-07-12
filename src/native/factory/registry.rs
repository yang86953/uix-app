//! Backend registry types and table-driven context creation (P6.7 M4–M5 / P6.8).

use std::ffi::c_void;

use crate::core::error::{Errc, Error};
use crate::native::factory::thread_bound::bind_to_current_thread;
use crate::native::traits::present::{GraphicsBackend, IGraphicsContext, PresentMode, RasterMode};

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

/// One graphics API factory row — API identity plus declared raster × present axes.
///
/// Engine assembly still reads live [`IGraphicsContext::caps`]; these fields document
/// the combination this entry is expected to provide ([#169](docs/决策.md#d169)).
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
    pub(crate) create: fn(*mut c_void, i32, i32) -> Result<Box<dyn IGraphicsContext>, Error>,
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
pub fn try_create_context(
    entry: &GraphicsBackendEntry,
    native_surface: *mut c_void,
    width: i32,
    height: i32,
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
    let ctx = (entry.create)(native_surface, width, height)?;
    let caps = ctx.caps();
    let actual = GraphicsRecipe::new(caps.backend, caps.raster, caps.present);
    let expected = entry.recipe();
    if actual != expected {
        let msg = format!("Graphics recipe {expected}: context reported {actual}");
        // Creation succeeded, so a context whose live caps do not match the
        // registry row must be shut down before returning the mismatch.
        let mut ctx = ctx;
        ctx.shutdown();
        return Err(Error::new(Errc::PlatformError, msg));
    }
    Ok(bind_to_current_thread(ctx))
}

/// Ordered probe candidates for a backend request, with one item per recipe
/// row.  An explicit backend request deliberately retains every active recipe
/// for that backend instead of selecting the first matching row.
fn active_recipes_by_priority(
    entries: &[GraphicsBackendEntry],
    requested: GraphicsBackend,
) -> Vec<GraphicsRecipe> {
    let mut entries = entries
        .iter()
        .filter(|entry| {
            entry.is_probe_candidate()
                && (requested == GraphicsBackend::Auto || entry.id == requested)
        })
        .collect::<Vec<_>>();
    // Stable sort preserves declaration order for equal priorities.
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.priority));
    entries
        .into_iter()
        .map(GraphicsBackendEntry::recipe)
        .collect()
}

/// Recipe-level probe candidates used by graphics bootstrap.
pub fn gpu_recipe_candidates(requested: GraphicsBackend) -> Vec<GraphicsRecipe> {
    active_recipes_by_priority(active_entries(), requested)
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
    native_surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    let entry = entry_for_recipe(recipe).ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            format!("Graphics factory: no registry entry for recipe {recipe}"),
        )
    })?;
    try_create_context(entry, native_surface, width, height)
}

/// Creates a single GPU context for one backend (compatibility helper).
///
/// New bootstrap code must use [`try_create_gpu_recipe`] so it can continue to
/// later recipe rows for the same API.
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    static IDENTITY_MISMATCH_SHUTDOWNS: AtomicUsize = AtomicUsize::new(0);

    struct D3d11IdentityContext;

    impl IGraphicsContext for D3d11IdentityContext {
        fn caps(&self) -> crate::native::traits::present::GraphicsContextCaps {
            crate::native::traits::present::GraphicsContextCaps::gpu_native_swapchain(
                GraphicsBackend::D3d11,
                false,
                1.0,
            )
        }

        fn initialize(&mut self, _: *mut c_void, _: i32, _: i32) -> crate::core::Result<()> {
            Ok(())
        }
        fn resize(&mut self, _: i32, _: i32) -> crate::core::Result<()> {
            Ok(())
        }
        fn make_current(&mut self) -> crate::core::Result<()> {
            Ok(())
        }
        fn swap_buffers(&mut self, _: crate::core::PresentDamage) -> crate::core::Result<()> {
            Ok(())
        }
        fn shutdown(&mut self) {
            IDENTITY_MISMATCH_SHUTDOWNS.fetch_add(1, Ordering::SeqCst);
        }
        fn read_pixels(&mut self, _: i32, _: i32, _: i32, _: i32) -> Vec<u32> {
            Vec::new()
        }
        fn width(&self) -> i32 {
            1
        }
        fn height(&self) -> i32 {
            1
        }
    }

    fn create_d3d11_identity(
        _: *mut c_void,
        _: i32,
        _: i32,
    ) -> Result<Box<dyn IGraphicsContext>, Error> {
        Ok(Box::new(D3d11IdentityContext))
    }

    #[test]
    fn auto_candidates_follow_registry_priority_order() {
        let candidates = gpu_probe_candidates(GraphicsBackend::Auto);
        let mut entries: Vec<_> = active_entries()
            .iter()
            .filter(|entry| entry.is_probe_candidate())
            .collect();
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.priority));
        assert_eq!(
            candidates,
            entries
                .into_iter()
                .map(|entry| entry.id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn recipe_candidate_sort_uses_numeric_priority_and_is_stable_for_ties() {
        let entries = [
            GraphicsBackendEntry {
                id: GraphicsBackend::D3d12,
                priority: 5,
                status: BackendStatus::Active,
                raster: RasterMode::GpuNative,
                present: PresentMode::Swapchain,
                create: create_d3d11_identity,
            },
            GraphicsBackendEntry {
                id: GraphicsBackend::D3d11,
                priority: 20,
                status: BackendStatus::Active,
                raster: RasterMode::GpuNative,
                present: PresentMode::Swapchain,
                create: create_d3d11_identity,
            },
            GraphicsBackendEntry {
                id: GraphicsBackend::OpenGlEs,
                priority: 10,
                status: BackendStatus::Active,
                raster: RasterMode::GpuNative,
                present: PresentMode::Swapchain,
                create: create_d3d11_identity,
            },
            GraphicsBackendEntry {
                id: GraphicsBackend::Vulkan,
                priority: 20,
                status: BackendStatus::Active,
                raster: RasterMode::Cpu,
                present: PresentMode::PixelUpload,
                create: create_d3d11_identity,
            },
        ];
        assert_eq!(
            active_recipes_by_priority(&entries, GraphicsBackend::Auto),
            vec![
                GraphicsRecipe::new(
                    GraphicsBackend::D3d11,
                    RasterMode::GpuNative,
                    PresentMode::Swapchain
                ),
                GraphicsRecipe::new(
                    GraphicsBackend::Vulkan,
                    RasterMode::Cpu,
                    PresentMode::PixelUpload
                ),
                GraphicsRecipe::new(
                    GraphicsBackend::OpenGlEs,
                    RasterMode::GpuNative,
                    PresentMode::Swapchain
                ),
                GraphicsRecipe::new(
                    GraphicsBackend::D3d12,
                    RasterMode::GpuNative,
                    PresentMode::Swapchain
                ),
            ]
        );
    }

    #[cfg(not(any(
        feature = "d3d11",
        feature = "d3d12",
        feature = "opengles",
        feature = "vulkan",
        feature = "metal"
    )))]
    #[test]
    fn feature_off_registry_rows_refuse_context_creation_without_linking_an_api_module() {
        for entry in active_entries() {
            assert_eq!(entry.status, BackendStatus::Disabled);
            let error = match try_create_context(entry, std::ptr::null_mut(), 1, 1) {
                Ok(_) => panic!("feature-off registry row must not construct a context"),
                Err(error) => error,
            };
            assert_eq!(error.code(), Errc::PlatformError);
            assert!(error.message().contains("disabled"));
        }
    }

    #[test]
    fn explicit_backend_keeps_each_active_recipe_row() {
        let entries = [
            GraphicsBackendEntry {
                id: GraphicsBackend::D3d11,
                priority: 20,
                status: BackendStatus::Active,
                raster: RasterMode::GpuNative,
                present: PresentMode::Swapchain,
                create: create_d3d11_identity,
            },
            GraphicsBackendEntry {
                id: GraphicsBackend::D3d11,
                priority: 10,
                status: BackendStatus::Active,
                raster: RasterMode::Cpu,
                present: PresentMode::PixelUpload,
                create: create_d3d11_identity,
            },
            GraphicsBackendEntry {
                id: GraphicsBackend::OpenGlEs,
                priority: 30,
                status: BackendStatus::Active,
                raster: RasterMode::GpuNative,
                present: PresentMode::Swapchain,
                create: create_d3d11_identity,
            },
        ];

        assert_eq!(
            active_recipes_by_priority(&entries, GraphicsBackend::D3d11),
            vec![
                GraphicsRecipe::new(
                    GraphicsBackend::D3d11,
                    RasterMode::GpuNative,
                    PresentMode::Swapchain
                ),
                GraphicsRecipe::new(
                    GraphicsBackend::D3d11,
                    RasterMode::Cpu,
                    PresentMode::PixelUpload
                ),
            ]
        );
    }

    #[test]
    fn explicit_backend_probes_only_requested_backend() {
        let candidates = gpu_recipe_candidates(GraphicsBackend::Vulkan);
        assert!(
            candidates
                .iter()
                .all(|recipe| recipe.backend == GraphicsBackend::Vulkan),
            "explicit requests may have no active rows on this platform, but must never probe another API"
        );
    }

    #[test]
    fn registry_rejects_context_with_wrong_backend_identity() {
        IDENTITY_MISMATCH_SHUTDOWNS.store(0, Ordering::SeqCst);
        let entry = GraphicsBackendEntry {
            id: GraphicsBackend::D3d12,
            priority: 1,
            status: BackendStatus::Active,
            raster: RasterMode::GpuNative,
            present: PresentMode::Swapchain,
            create: create_d3d11_identity,
        };
        let err = match try_create_context(&entry, std::ptr::null_mut(), 1, 1) {
            Ok(_) => panic!("wrong backend identity must be rejected"),
            Err(err) => err,
        };
        assert!(err.message().contains("reported backend=d3d11"));
        assert_eq!(IDENTITY_MISMATCH_SHUTDOWNS.load(Ordering::SeqCst), 1);

        IDENTITY_MISMATCH_SHUTDOWNS.store(0, Ordering::SeqCst);
        let entry = GraphicsBackendEntry {
            id: GraphicsBackend::D3d11,
            priority: 1,
            status: BackendStatus::Active,
            raster: RasterMode::Cpu,
            present: PresentMode::Swapchain,
            create: create_d3d11_identity,
        };
        let err = match try_create_context(&entry, std::ptr::null_mut(), 1, 1) {
            Ok(_) => panic!("wrong raster axis must be rejected"),
            Err(err) => err,
        };
        assert!(err
            .message()
            .contains("reported backend=d3d11; raster=gpu_native"));
        assert!(err.message().contains("recipe backend=d3d11; raster=cpu"));
        assert_eq!(IDENTITY_MISMATCH_SHUTDOWNS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn inactive_backend_is_skipped_in_auto_chain() {
        let candidates = gpu_probe_candidates(GraphicsBackend::Auto);
        if entry_for(GraphicsBackend::D3d12).is_some_and(|entry| entry.is_probe_candidate()) {
            assert!(candidates.contains(&GraphicsBackend::D3d12));
        } else {
            assert!(!candidates.contains(&GraphicsBackend::D3d12));
        }
    }

    #[test]
    fn auto_backend_uses_platform_default_order() {
        let candidates = gpu_probe_candidates(GraphicsBackend::Auto);

        #[cfg(windows)]
        {
            let mut expected = Vec::new();
            if cfg!(feature = "d3d11") {
                expected.push(GraphicsBackend::D3d11);
            }
            if cfg!(feature = "opengles") {
                expected.push(GraphicsBackend::OpenGlEs);
            }
            if cfg!(feature = "d3d12") {
                expected.push(GraphicsBackend::D3d12);
            }
            assert_eq!(candidates, expected);
        }

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
                GraphicsBackend::D3d12 => {
                    assert_eq!(entry.raster, RasterMode::GpuNative);
                    assert_eq!(entry.present, PresentMode::Swapchain);
                }
                GraphicsBackend::Auto => {}
            }
        }
    }
}
