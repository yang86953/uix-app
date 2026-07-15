use crate::core::Result;
use crate::draw::engine::bootstrap::ProbeStage;
use crate::draw::engine::bootstrap::*;
use crate::draw::engine::factory::create_graphics_engine;
#[cfg(feature = "d3d11")]
use crate::native::factory::gpu_recipe_candidates;
use crate::native::factory::GraphicsRecipe;
#[cfg(feature = "d3d11")]
use crate::native::traits::present::NativeSurfaceHandle;
use crate::native::traits::present::PresentOcclusionSupport;
use crate::tests::common::*;
use std::ffi::c_void;

#[cfg(feature = "d3d11")]
fn null_surface() -> NativeSurfaceHandle {
    // SAFETY: probe tests deliberately exercise context creation failure
    // without a real platform surface and never dereference this handle.
    unsafe { NativeSurfaceHandle::from_raw(std::ptr::null_mut()) }
}

struct ShutdownTrackingContext {
    shutdown_called: Rc<Cell<bool>>,
    raster: RasterMode,
    present: PresentMode,
}

impl IGraphicsContext for ShutdownTrackingContext {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps {
            backend: GraphicsBackend::D3d11,
            raster: self.raster,
            present: self.present,
            present_occlusion: PresentOcclusionSupport::Unsupported,
            present_coherency: PresentCoherency::FullOnly,
            device_pixel_ratio: 1.0,
        }
    }

    fn initialize(&mut self, _native_window: *mut c_void, _width: i32, _height: i32) -> Result<()> {
        Ok(())
    }

    fn resize(&mut self, _width: i32, _height: i32) -> crate::core::Result<()> {
        Ok(())
    }

    fn make_current(&mut self) -> crate::core::Result<()> {
        Ok(())
    }

    fn swap_buffers(&mut self, _damage: PresentDamage) -> crate::core::Result<()> {
        Ok(())
    }

    fn try_shutdown(&mut self) -> crate::core::Result<()> {
        self.shutdown_called.set(true);
        Ok(())
    }

    fn read_pixels(
        &mut self,
        _x: i32,
        _y: i32,
        _width: i32,
        _height: i32,
    ) -> crate::core::Result<Vec<u32>> {
        Ok(Vec::new())
    }

    fn width(&self) -> i32 {
        1
    }

    fn height(&self) -> i32 {
        1
    }

    fn present_pixels(
        &mut self,
        _pixels: &[u32],
        _width: i32,
        _height: i32,
        _damage: PresentDamage,
    ) -> Result<()> {
        Ok(())
    }
}

#[test]
fn create_graphics_engine_shuts_down_context_on_cpu_presenter() {
    let shutdown_called = Rc::new(Cell::new(false));
    let context = ShutdownTrackingContext {
        shutdown_called: Rc::clone(&shutdown_called),
        raster: RasterMode::Cpu,
        present: PresentMode::CpuPresenter,
    };

    let err = match create_graphics_engine(Box::new(context)) {
        Ok(_) => panic!("CpuPresenter must be rejected"),
        Err(err) => err,
    };

    assert!(err.message().contains("CpuPresenter"));
    assert!(shutdown_called.get());
}

#[test]
fn caps_axes_drive_supports_helpers() {
    let context = ShutdownTrackingContext {
        shutdown_called: Rc::new(Cell::new(false)),
        raster: RasterMode::Cpu,
        present: PresentMode::PixelUpload,
    };
    assert!(context.supports_pixel_present());
    assert!(!context.supports_gl_proc_address());
}

#[cfg(feature = "d3d11")]
#[test]
fn bootstrap_shuts_down_context_when_engine_creation_fails() {
    let shutdown_called = Rc::new(Cell::new(false));
    match bootstrap_graphics_engine_with(
        null_surface(),
        640,
        480,
        GraphicsBackend::D3d11,
        |_candidate| {
            Ok(Box::new(ShutdownTrackingContext {
                shutdown_called: Rc::clone(&shutdown_called),
                raster: RasterMode::Cpu,
                present: PresentMode::CpuPresenter,
            }) as Box<dyn IGraphicsContext>)
        },
    ) {
        Ok(_) => panic!("CpuPresenter must fail engine creation"),
        Err(report) => {
            assert!(shutdown_called.get());
            let failure = report.failures.first().expect("engine failure");
            assert_eq!(failure.backend, GraphicsBackend::D3d11);
            assert!(failure.message.contains("stage=engine_create"));
            assert!(failure
                .message
                .contains("selected=backend=d3d11; raster=cpu; present=cpu_presenter"));
            assert!(failure.message.contains("CpuPresenter"));
        }
    }
}

#[cfg(feature = "d3d11")]
struct MakeCurrentFailingNativeContext {
    shutdown_called: Rc<Cell<bool>>,
    checked_shutdown_failures: Rc<Cell<usize>>,
}

#[cfg(feature = "d3d11")]
impl IGraphicsContext for MakeCurrentFailingNativeContext {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(
            GraphicsBackend::D3d11,
            PresentCoherency::FullOnly,
            1.0,
        )
    }

    fn native_raster_caps(&self) -> NativeRasterCaps {
        NativeRasterCaps::d3d11_full()
    }

    fn initialize(&mut self, _native_window: *mut c_void, _width: i32, _height: i32) -> Result<()> {
        Ok(())
    }

    fn resize(&mut self, _width: i32, _height: i32) -> crate::core::Result<()> {
        Ok(())
    }

    fn make_current(&mut self) -> crate::core::Result<()> {
        Err(Error::new(
            Errc::PlatformError,
            "make-current failed for test",
        ))
    }

    fn swap_buffers(&mut self, _damage: PresentDamage) -> crate::core::Result<()> {
        Ok(())
    }

    fn try_shutdown(&mut self) -> crate::core::Result<()> {
        self.shutdown_called.set(true);
        let failures = self.checked_shutdown_failures.get();
        if failures > 0 {
            self.checked_shutdown_failures.set(failures - 1);
            return Err(Error::new(
                Errc::InvalidState,
                "checked shutdown failed for test",
            ));
        }
        Ok(())
    }

    fn read_pixels(
        &mut self,
        _x: i32,
        _y: i32,
        _width: i32,
        _height: i32,
    ) -> crate::core::Result<Vec<u32>> {
        Ok(Vec::new())
    }

    fn width(&self) -> i32 {
        640
    }

    fn height(&self) -> i32 {
        480
    }

    fn present_pixels(
        &mut self,
        _pixels: &[u32],
        _width: i32,
        _height: i32,
        _damage: PresentDamage,
    ) -> Result<()> {
        Ok(())
    }
}

#[cfg(feature = "d3d11")]
#[test]
fn bootstrap_shuts_down_context_when_engine_initialize_fails() {
    let shutdown_called = Rc::new(Cell::new(false));
    let checked_shutdown_failures = Rc::new(Cell::new(0));
    match bootstrap_graphics_engine_with(
        null_surface(),
        640,
        480,
        GraphicsBackend::D3d11,
        |_candidate| {
            Ok(Box::new(MakeCurrentFailingNativeContext {
                shutdown_called: Rc::clone(&shutdown_called),
                checked_shutdown_failures: Rc::clone(&checked_shutdown_failures),
            }) as Box<dyn IGraphicsContext>)
        },
    ) {
        Ok(_) => panic!("engine initialization failure must reject bootstrap"),
        Err(report) => {
            assert!(shutdown_called.get());
            let failure = report.failures.first().expect("init failure");
            assert_eq!(failure.backend, GraphicsBackend::D3d11);
            assert!(failure.message.contains("stage=engine_initialize"));
            assert!(failure
                .message
                .contains("selected=backend=d3d11; raster=gpu_native; present=swapchain"));
            assert!(failure.message.contains("make-current failed for test"));
        }
    }
}

#[cfg(feature = "d3d11")]
#[test]
fn bootstrap_preserves_initialize_failure_when_checked_teardown_fails() {
    let shutdown_called = Rc::new(Cell::new(false));
    let checked_shutdown_failures = Rc::new(Cell::new(1));
    let report = match bootstrap_graphics_engine_with(
        null_surface(),
        640,
        480,
        GraphicsBackend::D3d11,
        |_candidate| {
            Ok(Box::new(MakeCurrentFailingNativeContext {
                shutdown_called: Rc::clone(&shutdown_called),
                checked_shutdown_failures: Rc::clone(&checked_shutdown_failures),
            }) as Box<dyn IGraphicsContext>)
        },
    ) {
        Ok(_) => panic!("checked teardown failure must reject bootstrap"),
        Err(report) => report,
    };

    assert!(shutdown_called.get());
    let failure = report.failures.first().expect("init failure");
    assert!(failure.message.contains("checked shutdown failed for test"));
    assert!(failure.message.contains("make-current failed for test"));
}

#[cfg(all(feature = "d3d12", feature = "d3d11", feature = "opengles"))]
struct BootstrapD3d12Context;

#[cfg(all(feature = "d3d12", feature = "d3d11", feature = "opengles"))]
impl IGraphicsContext for BootstrapD3d12Context {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(
            GraphicsBackend::D3d12,
            PresentCoherency::FullOnly,
            1.0,
        )
    }

    fn native_raster_caps(&self) -> NativeRasterCaps {
        NativeRasterCaps {
            clear_target: true,
            soft_blit: true,
            solid_rects: true,
            ..NativeRasterCaps::default()
        }
    }

    fn initialize(&mut self, _native_window: *mut c_void, _width: i32, _height: i32) -> Result<()> {
        Ok(())
    }

    fn resize(&mut self, _width: i32, _height: i32) -> crate::core::Result<()> {
        Ok(())
    }
    fn make_current(&mut self) -> crate::core::Result<()> {
        Ok(())
    }
    fn swap_buffers(&mut self, _damage: PresentDamage) -> crate::core::Result<()> {
        Ok(())
    }
    fn try_shutdown(&mut self) -> crate::core::Result<()> {
        Ok(())
    }
    fn read_pixels(
        &mut self,
        _x: i32,
        _y: i32,
        _width: i32,
        _height: i32,
    ) -> crate::core::Result<Vec<u32>> {
        Ok(Vec::new())
    }
    fn width(&self) -> i32 {
        64
    }
    fn height(&self) -> i32 {
        48
    }
}

#[cfg(all(
    feature = "d3d12",
    feature = "d3d11",
    feature = "opengles",
    feature = "vulkan"
))]
#[test]
fn auto_falls_through_established_backends_to_low_priority_d3d12() {
    let expected = gpu_recipe_candidates(GraphicsBackend::Auto);
    if !expected
        .iter()
        .any(|recipe| recipe.backend == GraphicsBackend::D3d12)
    {
        // D3D12 is not an Auto candidate on this platform registry.
        return;
    }

    let calls = Rc::new(RefCell::new(Vec::new()));
    let calls_for_probe = Rc::clone(&calls);
    let mut bootstrap = bootstrap_graphics_engine_with(
        null_surface(),
        64,
        48,
        GraphicsBackend::Auto,
        move |candidate| {
            calls_for_probe.borrow_mut().push(candidate);
            if candidate.backend == GraphicsBackend::D3d12 {
                Ok(Box::new(BootstrapD3d12Context) as Box<dyn IGraphicsContext>)
            } else {
                Err(Error::new(
                    Errc::PlatformError,
                    format!("{candidate} unavailable for ordered probe test"),
                ))
            }
        },
    )
    .expect("D3D12 should be selected after established candidates fail");
    assert_eq!(calls.borrow().as_slice(), expected.as_slice());
    assert_eq!(bootstrap.selected, GraphicsBackend::D3d12);
    assert_eq!(
        bootstrap.selected_recipe,
        GraphicsRecipe::new(
            GraphicsBackend::D3d12,
            RasterMode::GpuNative,
            PresentMode::Swapchain
        )
    );
    let expected_failures = expected.len().saturating_sub(1);
    assert_eq!(bootstrap.report.failures.len(), expected_failures);
    for (index, recipe) in expected.iter().take(expected_failures).enumerate() {
        assert_eq!(bootstrap.report.failures[index].backend, recipe.backend);
    }
    assert!(bootstrap
        .report
        .failures
        .iter()
        .all(|failure| failure.message.contains("stage=context_create")));
    bootstrap.engine.try_shutdown().expect("checked shutdown");
}

#[test]
fn same_api_recipe_probe_falls_through_to_later_recipe() {
    let native_recipe = GraphicsRecipe::new(
        GraphicsBackend::D3d11,
        RasterMode::GpuNative,
        PresentMode::Swapchain,
    );
    let upload_recipe = GraphicsRecipe::new(
        GraphicsBackend::D3d11,
        RasterMode::Cpu,
        PresentMode::PixelUpload,
    );
    let calls = Rc::new(RefCell::new(Vec::new()));
    let calls_for_probe = Rc::clone(&calls);

    let mut bootstrap = bootstrap_graphics_engine_with_candidates(
        64,
        48,
        GraphicsBackend::D3d11,
        vec![native_recipe, upload_recipe],
        move |candidate| {
            calls_for_probe.borrow_mut().push(candidate);
            if candidate == native_recipe {
                return Err(Error::new(
                    Errc::PlatformError,
                    "native recipe intentionally unavailable",
                ));
            }
            Ok(Box::new(ShutdownTrackingContext {
                shutdown_called: Rc::new(Cell::new(false)),
                raster: RasterMode::Cpu,
                present: PresentMode::PixelUpload,
            }) as Box<dyn IGraphicsContext>)
        },
    )
    .expect("second recipe for the same API should be selected");

    assert_eq!(calls.borrow().as_slice(), &[native_recipe, upload_recipe]);
    assert_eq!(bootstrap.selected, GraphicsBackend::D3d11);
    assert_eq!(bootstrap.selected_recipe, upload_recipe);
    assert_eq!(
        bootstrap.present_occlusion,
        PresentOcclusionSupport::Unsupported
    );
    assert_eq!(bootstrap.report.failures.len(), 1);
    assert_eq!(bootstrap.report.failures[0].backend, GraphicsBackend::D3d11);
    assert!(bootstrap.report.failures[0]
        .message
        .contains("recipe=backend=d3d11; raster=gpu_native; present=swapchain"));
    bootstrap.engine.try_shutdown().expect("checked shutdown");
}

#[test]
fn probe_report_preserves_context_create_stage_and_message() {
    let mut report = ProbeReport::default();
    report.record_failure_at(
        GraphicsRecipe::new(
            GraphicsBackend::OpenGlEs,
            RasterMode::GpuNative,
            PresentMode::Swapchain,
        ),
        ProbeStage::ContextCreate,
        None,
        &Error::new(Errc::PlatformError, "WGL context creation failed"),
    );

    let failure = report.failures.first().expect("context failure");
    assert_eq!(failure.backend, GraphicsBackend::OpenGlEs);
    assert!(failure.message.contains("stage=context_create"));
    assert!(failure
        .message
        .contains("recipe=backend=opengles; raster=gpu_native; present=swapchain"));
    assert!(failure.message.contains("selected=none"));
    assert!(failure.message.contains("WGL context creation failed"));
}
