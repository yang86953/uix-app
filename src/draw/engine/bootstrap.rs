//! GPU graphics bootstrap — sole owner of the init-time backend probe loop (P6.7 M3).

use std::ffi::c_void;

use crate::core::{Errc, Error, Result};
use crate::draw::engine::factory::create_graphics_engine;
use crate::draw::traits::GraphicsEngine;
use crate::native::factory::{gpu_recipe_candidates, try_create_gpu_recipe, GraphicsRecipe};
use crate::native::traits::present::{GraphicsBackend, IGraphicsContext};

/// One failed probe attempt recorded for diagnostics and tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeFailure {
    pub backend: GraphicsBackend,
    pub message: String,
}

/// Aggregated probe failures when every GPU candidate fails.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProbeReport {
    pub failures: Vec<ProbeFailure>,
}

impl ProbeReport {
    pub fn record_failure(&mut self, recipe: GraphicsRecipe, err: &Error) {
        self.record_failure_at(recipe, ProbeStage::Unspecified, Some(recipe), err);
    }

    fn record_failure_at(
        &mut self,
        candidate: GraphicsRecipe,
        stage: ProbeStage,
        selected: Option<GraphicsRecipe>,
        err: &Error,
    ) {
        let selected = selected
            .map(|recipe| recipe.to_string())
            .unwrap_or_else(|| "none".to_string());
        self.failures.push(ProbeFailure {
            backend: candidate.backend,
            message: format!(
                "stage={}; recipe={candidate}; selected={selected}; error={}",
                stage.as_str(),
                err.what()
            ),
        });
    }

    fn record_no_candidates(&mut self, request: GraphicsBackend, err: &Error) {
        self.failures.push(ProbeFailure {
            backend: request,
            message: format!(
                "stage={}; recipe=none; selected=none; error={}",
                ProbeStage::CandidateSelection.as_str(),
                err.what()
            ),
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeStage {
    Unspecified,
    CandidateSelection,
    ContextCreate,
    EngineCreate,
    EngineInitialize,
}

impl ProbeStage {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Unspecified => "unspecified",
            Self::CandidateSelection => "candidate_selection",
            Self::ContextCreate => "context_create",
            Self::EngineCreate => "engine_create",
            Self::EngineInitialize => "engine_initialize",
        }
    }
}

/// Successful GPU bootstrap result.
pub struct GpuBootstrap {
    pub engine: Box<dyn GraphicsEngine>,
    pub selected: GraphicsBackend,
    pub selected_recipe: GraphicsRecipe,
    pub report: ProbeReport,
}

/// Probes GPU backends in platform order and returns the first working engine.
///
/// CPU fallback (`SoftwareEngine`) stays in app; this function only handles GPU paths.
pub fn bootstrap_graphics_engine(
    surface: *mut c_void,
    width: i32,
    height: i32,
    request: GraphicsBackend,
) -> Result<GpuBootstrap, ProbeReport> {
    bootstrap_graphics_engine_with(surface, width, height, request, |candidate| {
        try_create_gpu_recipe(candidate, surface, width, height)
    })
}

fn bootstrap_graphics_engine_with<F>(
    _surface: *mut c_void,
    width: i32,
    height: i32,
    request: GraphicsBackend,
    try_create: F,
) -> Result<GpuBootstrap, ProbeReport>
where
    F: FnMut(GraphicsRecipe) -> Result<Box<dyn IGraphicsContext>, Error>,
{
    bootstrap_graphics_engine_with_candidates(
        width,
        height,
        request,
        gpu_recipe_candidates(request),
        try_create,
    )
}

fn bootstrap_graphics_engine_with_candidates<F>(
    width: i32,
    height: i32,
    request: GraphicsBackend,
    candidates: Vec<GraphicsRecipe>,
    mut try_create: F,
) -> Result<GpuBootstrap, ProbeReport>
where
    F: FnMut(GraphicsRecipe) -> Result<Box<dyn IGraphicsContext>, Error>,
{
    let mut report = ProbeReport::default();

    if candidates.is_empty() {
        report.record_no_candidates(
            request,
            &Error::new(
                Errc::PlatformError,
                format!("Graphics bootstrap: no GPU backend candidates for {request}"),
            ),
        );
        return Err(report);
    }

    for candidate in candidates {
        crate::core::log::info_fn(format!("Graphics bootstrap: probing recipe {candidate}"));
        let context = match try_create(candidate) {
            Ok(context) => context,
            Err(err) => {
                crate::core::log::warn_fn(format!(
                    "Graphics bootstrap: recipe {candidate} unavailable: {}",
                    err.what()
                ));
                report.record_failure_at(candidate, ProbeStage::ContextCreate, None, &err);
                continue;
            }
        };
        let caps = context.caps();
        let selected = GraphicsRecipe::new(caps.backend, caps.raster, caps.present);

        let mut engine = match create_graphics_engine(context) {
            Ok(engine) => engine,
            Err(err) => {
                crate::core::log::warn_fn(format!(
                    "Graphics bootstrap: engine for recipe {selected} unavailable: {}",
                    err.what()
                ));
                report.record_failure_at(candidate, ProbeStage::EngineCreate, Some(selected), &err);
                continue;
            }
        };

        match engine.initialize(width, height) {
            Ok(()) => {
                crate::core::log::info_fn(format!(
                    "Graphics bootstrap: selected recipe {selected}"
                ));
                return Ok(GpuBootstrap {
                    engine,
                    selected: selected.backend,
                    selected_recipe: selected,
                    report,
                });
            }
            Err(err) => {
                crate::core::log::warn_fn(format!(
                    "Graphics bootstrap: engine init for recipe {selected} failed: {}",
                    err.what()
                ));
                engine.shutdown();
                report.record_failure_at(
                    candidate,
                    ProbeStage::EngineInitialize,
                    Some(selected),
                    &err,
                );
            }
        }
    }

    Err(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(feature = "d3d12", feature = "d3d11", feature = "opengles"))]
    use crate::native::traits::present::NativeRasterCaps;
    use crate::native::traits::present::{
        GraphicsContextCaps, IGraphicsContext, PresentDamage, PresentMode, RasterMode,
    };
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

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
                partial_present: false,
                device_pixel_ratio: 1.0,
            }
        }

        fn initialize(
            &mut self,
            _native_window: *mut c_void,
            _width: i32,
            _height: i32,
        ) -> Result<()> {
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

        fn shutdown(&mut self) {
            self.shutdown_called.set(true);
        }

        fn read_pixels(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) -> Vec<u32> {
            Vec::new()
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
            std::ptr::null_mut(),
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

    struct InitFailingContext {
        shutdown_called: Rc<Cell<bool>>,
    }

    impl IGraphicsContext for InitFailingContext {
        fn caps(&self) -> GraphicsContextCaps {
            GraphicsContextCaps::cpu_pixel_upload(GraphicsBackend::D3d11, 1.0)
        }

        fn initialize(
            &mut self,
            _native_window: *mut c_void,
            _width: i32,
            _height: i32,
        ) -> Result<()> {
            Err(Error::new(Errc::PlatformError, "init failed for test"))
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

        fn shutdown(&mut self) {
            self.shutdown_called.set(true);
        }

        fn read_pixels(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) -> Vec<u32> {
            Vec::new()
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
        match bootstrap_graphics_engine_with(
            std::ptr::null_mut(),
            640,
            480,
            GraphicsBackend::D3d11,
            |_candidate| {
                Ok(Box::new(InitFailingContext {
                    shutdown_called: Rc::clone(&shutdown_called),
                }) as Box<dyn IGraphicsContext>)
            },
        ) {
            Ok(_) => panic!("initialize failure must reject bootstrap"),
            Err(report) => {
                assert!(shutdown_called.get());
                let failure = report.failures.first().expect("init failure");
                assert_eq!(failure.backend, GraphicsBackend::D3d11);
                assert!(failure.message.contains("stage=engine_initialize"));
                assert!(failure
                    .message
                    .contains("selected=backend=d3d11; raster=cpu; present=pixel_upload"));
                assert!(failure.message.contains("init failed for test"));
            }
        }
    }

    #[cfg(all(feature = "d3d12", feature = "d3d11", feature = "opengles"))]
    struct BootstrapD3d12Context;

    #[cfg(all(feature = "d3d12", feature = "d3d11", feature = "opengles"))]
    impl IGraphicsContext for BootstrapD3d12Context {
        fn caps(&self) -> GraphicsContextCaps {
            GraphicsContextCaps::gpu_native_swapchain(GraphicsBackend::D3d12, false, 1.0)
        }

        fn native_raster_caps(&self) -> NativeRasterCaps {
            NativeRasterCaps {
                clear_target: true,
                soft_blit: true,
                solid_rects: true,
                ..NativeRasterCaps::default()
            }
        }

        fn initialize(
            &mut self,
            _native_window: *mut c_void,
            _width: i32,
            _height: i32,
        ) -> Result<()> {
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
        fn shutdown(&mut self) {}
        fn read_pixels(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) -> Vec<u32> {
            Vec::new()
        }
        fn width(&self) -> i32 {
            64
        }
        fn height(&self) -> i32 {
            48
        }
    }

    #[cfg(all(feature = "d3d12", feature = "d3d11", feature = "opengles"))]
    #[test]
    fn auto_falls_through_established_backends_to_low_priority_d3d12() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let calls_for_probe = Rc::clone(&calls);
        let mut bootstrap = bootstrap_graphics_engine_with(
            std::ptr::null_mut(),
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
        assert_eq!(
            calls.borrow().as_slice(),
            &[
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
            ]
        );
        assert_eq!(bootstrap.selected, GraphicsBackend::D3d12);
        assert_eq!(
            bootstrap.selected_recipe,
            GraphicsRecipe::new(
                GraphicsBackend::D3d12,
                RasterMode::GpuNative,
                PresentMode::Swapchain
            )
        );
        assert_eq!(bootstrap.report.failures.len(), 2);
        assert_eq!(bootstrap.report.failures[0].backend, GraphicsBackend::D3d11);
        assert_eq!(
            bootstrap.report.failures[1].backend,
            GraphicsBackend::OpenGlEs
        );
        assert!(bootstrap
            .report
            .failures
            .iter()
            .all(|failure| failure.message.contains("stage=context_create")));
        bootstrap.engine.shutdown();
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
        assert_eq!(bootstrap.report.failures.len(), 1);
        assert_eq!(bootstrap.report.failures[0].backend, GraphicsBackend::D3d11);
        assert!(bootstrap.report.failures[0]
            .message
            .contains("recipe=backend=d3d11; raster=gpu_native; present=swapchain"));
        bootstrap.engine.shutdown();
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
}
