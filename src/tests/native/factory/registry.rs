use crate::native::factory::registry::*;
use crate::tests::common::*;
use std::ffi::c_void;

static IDENTITY_MISMATCH_SHUTDOWNS: AtomicUsize = AtomicUsize::new(0);

struct D3d11IdentityContext;

impl IGraphicsContext for D3d11IdentityContext {
    fn caps(&self) -> crate::native::traits::present::GraphicsContextCaps {
        crate::native::traits::present::GraphicsContextCaps::gpu_native_swapchain(
            GraphicsBackend::D3d11,
            PresentCoherency::FullOnly,
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
    fn try_shutdown(&mut self) -> crate::core::Result<()> {
        IDENTITY_MISMATCH_SHUTDOWNS.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn read_pixels(&mut self, _: i32, _: i32, _: i32, _: i32) -> crate::core::Result<Vec<u32>> {
        Ok(Vec::new())
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
fn auto_candidates_follow_gpu_native_then_registry_priority_order() {
    let candidates = gpu_probe_candidates(GraphicsBackend::Auto);
    let mut entries: Vec<_> = active_entries()
        .iter()
        .filter(|entry| entry.is_probe_candidate())
        .collect();
    entries.sort_by_key(|entry| {
        (
            entry.raster != RasterMode::GpuNative,
            std::cmp::Reverse(entry.priority),
        )
    });
    assert_eq!(
        candidates,
        entries
            .into_iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>()
    );
}

#[test]
fn recipe_candidate_sort_prefers_gpu_native_before_cpu_then_uses_priority() {
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
                GraphicsBackend::OpenGlEs,
                RasterMode::GpuNative,
                PresentMode::Swapchain
            ),
            GraphicsRecipe::new(
                GraphicsBackend::D3d12,
                RasterMode::GpuNative,
                PresentMode::Swapchain
            ),
            GraphicsRecipe::new(
                GraphicsBackend::Vulkan,
                RasterMode::Cpu,
                PresentMode::PixelUpload
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

#[cfg(windows)]
#[test]
fn auto_backend_on_windows_includes_active_vulkan_when_enabled() {
    if !cfg!(feature = "vulkan") {
        return;
    }
    let candidates = gpu_probe_candidates(GraphicsBackend::Auto);
    assert!(
        candidates.contains(&GraphicsBackend::Vulkan),
        "Windows Vulkan Win32 adapter is Active and must be Auto-probeable"
    );
}

#[cfg(windows)]
#[test]
fn explicit_vulkan_includes_active_registry_row_on_windows() {
    if !cfg!(feature = "vulkan") {
        return;
    }
    let candidates = gpu_recipe_candidates(GraphicsBackend::Vulkan);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].backend, GraphicsBackend::Vulkan);
    assert_eq!(
        entry_for(GraphicsBackend::Vulkan)
            .expect("vulkan row")
            .status,
        BackendStatus::Active
    );
}

#[cfg(target_os = "macos")]
#[test]
fn explicit_vulkan_includes_active_registry_row_on_macos() {
    if !cfg!(feature = "vulkan") {
        return;
    }
    let candidates = gpu_recipe_candidates(GraphicsBackend::Vulkan);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].backend, GraphicsBackend::Vulkan);
    assert_eq!(
        entry_for(GraphicsBackend::Vulkan)
            .expect("vulkan row")
            .status,
        BackendStatus::Active
    );
}

#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn explicit_vulkan_includes_active_registry_row_on_linux() {
    if !cfg!(feature = "vulkan") {
        return;
    }
    let candidates = gpu_recipe_candidates(GraphicsBackend::Vulkan);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].backend, GraphicsBackend::Vulkan);
    assert_eq!(
        entry_for(GraphicsBackend::Vulkan)
            .expect("vulkan row")
            .status,
        BackendStatus::Active
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
        if cfg!(feature = "vulkan") {
            expected.push(GraphicsBackend::Vulkan);
        }
        if cfg!(feature = "d3d12") {
            expected.push(GraphicsBackend::D3d12);
        }
        if cfg!(feature = "opengles") {
            expected.push(GraphicsBackend::OpenGlEs);
        }
        assert_eq!(candidates, expected);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    assert_eq!(
        candidates,
        vec![GraphicsBackend::Vulkan, GraphicsBackend::OpenGlEs]
    );

    #[cfg(target_os = "macos")]
    {
        let mut expected = Vec::new();
        if cfg!(feature = "vulkan") {
            expected.push(GraphicsBackend::Vulkan);
        }
        if cfg!(feature = "metal") {
            expected.push(GraphicsBackend::Metal);
        }
        assert_eq!(candidates, expected);
    }
}

#[test]
fn registry_rows_declare_orthogonal_axes() {
    for entry in active_entries() {
        match entry.id {
            GraphicsBackend::OpenGlEs
            | GraphicsBackend::D3d11
            | GraphicsBackend::D3d12
            | GraphicsBackend::Vulkan
            | GraphicsBackend::Metal => {
                assert_eq!(entry.raster, RasterMode::GpuNative);
                assert_eq!(entry.present, PresentMode::Swapchain);
            }
            GraphicsBackend::Auto => {}
        }
    }
}
