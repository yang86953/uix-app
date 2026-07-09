//! GPU graphics bootstrap — sole owner of the init-time backend probe loop (P6.7 M3).

use std::ffi::c_void;

use crate::core::{Errc, Error, Result};
use crate::draw::engine::factory::create_graphics_engine;
use crate::draw::traits::GraphicsEngine;
use crate::native::factory::{gpu_probe_candidates, try_create_gpu_context};
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
    pub fn record_failure(&mut self, backend: GraphicsBackend, err: &Error) {
        self.record_failure_at(backend, ProbeStage::Unspecified, Some(backend), err);
    }

    fn record_failure_at(
        &mut self,
        candidate: GraphicsBackend,
        stage: ProbeStage,
        selected: Option<GraphicsBackend>,
        err: &Error,
    ) {
        let selected = selected.map(GraphicsBackend::as_str).unwrap_or("none");
        self.failures.push(ProbeFailure {
            backend: candidate,
            message: format!(
                "stage={}; selected={selected}; error={}",
                stage.as_str(),
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
        try_create_gpu_context(candidate, surface, width, height)
    })
}

fn bootstrap_graphics_engine_with<F>(
    _surface: *mut c_void,
    width: i32,
    height: i32,
    request: GraphicsBackend,
    mut try_create: F,
) -> Result<GpuBootstrap, ProbeReport>
where
    F: FnMut(GraphicsBackend) -> Result<Box<dyn IGraphicsContext>, Error>,
{
    let candidates = gpu_probe_candidates(request);
    let mut report = ProbeReport::default();

    if candidates.is_empty() {
        report.record_failure_at(
            request,
            ProbeStage::CandidateSelection,
            None,
            &Error::new(
                Errc::PlatformError,
                format!("Graphics bootstrap: no GPU backend candidates for {request}"),
            ),
        );
        return Err(report);
    }

    for candidate in candidates {
        crate::core::log::info_fn(format!("Graphics bootstrap: probing {candidate}"));
        let context = match try_create(candidate) {
            Ok(context) => context,
            Err(err) => {
                crate::core::log::warn_fn(format!(
                    "Graphics bootstrap: {candidate} unavailable: {}",
                    err.what()
                ));
                report.record_failure_at(candidate, ProbeStage::ContextCreate, None, &err);
                continue;
            }
        };
        let selected = context.graphics_backend();

        let mut engine = match create_graphics_engine(context) {
            Ok(engine) => engine,
            Err(err) => {
                crate::core::log::warn_fn(format!(
                    "Graphics bootstrap: engine for {selected} unavailable: {}",
                    err.what()
                ));
                report.record_failure_at(candidate, ProbeStage::EngineCreate, Some(selected), &err);
                continue;
            }
        };

        match engine.initialize(width, height) {
            Ok(()) => {
                crate::core::log::info_fn(format!("Graphics bootstrap: selected {selected}"));
                return Ok(GpuBootstrap {
                    engine,
                    selected,
                    report,
                });
            }
            Err(err) => {
                crate::core::log::warn_fn(format!(
                    "Graphics bootstrap: engine init for {selected} failed: {}",
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
    use crate::native::traits::present::{
        GraphicsContextCaps, IGraphicsContext, PresentDamage, PresentMode, RasterMode,
    };
    use std::cell::Cell;
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

        fn resize(&mut self, _width: i32, _height: i32) {}

        fn make_current(&mut self) {}

        fn swap_buffers(&mut self, _damage: PresentDamage) {}

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
                assert!(failure.message.contains("selected=d3d11"));
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

        fn resize(&mut self, _width: i32, _height: i32) {}

        fn make_current(&mut self) {}

        fn swap_buffers(&mut self, _damage: PresentDamage) {}

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
                assert!(failure.message.contains("selected=d3d11"));
                assert!(failure.message.contains("init failed for test"));
            }
        }
    }

    #[test]
    fn probe_report_preserves_context_create_stage_and_message() {
        let mut report = ProbeReport::default();
        report.record_failure_at(
            GraphicsBackend::OpenGlEs,
            ProbeStage::ContextCreate,
            None,
            &Error::new(Errc::PlatformError, "WGL context creation failed"),
        );

        let failure = report.failures.first().expect("context failure");
        assert_eq!(failure.backend, GraphicsBackend::OpenGlEs);
        assert!(failure.message.contains("stage=context_create"));
        assert!(failure.message.contains("selected=none"));
        assert!(failure.message.contains("WGL context creation failed"));
    }
}
