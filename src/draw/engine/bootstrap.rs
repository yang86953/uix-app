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
        self.failures.push(ProbeFailure {
            backend,
            message: err.short_what(),
        });
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
        report.record_failure(
            request,
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
                    err.short_what()
                ));
                report.record_failure(candidate, &err);
                continue;
            }
        };
        let selected = context.graphics_backend();

        let mut engine = match create_graphics_engine(context) {
            Ok(engine) => engine,
            Err(err) => {
                crate::core::log::warn_fn(format!(
                    "Graphics bootstrap: engine for {selected} unavailable: {}",
                    err.short_what()
                ));
                report.record_failure(selected, &err);
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
                    err.short_what()
                ));
                engine.shutdown();
                report.record_failure(selected, &err);
            }
        }
    }

    Err(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::traits::present::{
        GraphicsContextCaps, IGraphicsContext, PresentDamage, RenderPipelineProfile,
    };
    use std::cell::Cell;
    use std::rc::Rc;

    struct ShutdownTrackingContext {
        shutdown_called: Rc<Cell<bool>>,
        pipeline: RenderPipelineProfile,
    }

    impl IGraphicsContext for ShutdownTrackingContext {
        fn caps(&self) -> GraphicsContextCaps {
            GraphicsContextCaps {
                backend: GraphicsBackend::D3d11,
                pipeline: self.pipeline,
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
    fn create_graphics_engine_shuts_down_context_on_cpu_presenter_profile() {
        let shutdown_called = Rc::new(Cell::new(false));
        let context = ShutdownTrackingContext {
            shutdown_called: Rc::clone(&shutdown_called),
            pipeline: RenderPipelineProfile::CpuPresenter,
        };

        let err = match create_graphics_engine(Box::new(context)) {
            Ok(_) => panic!("CpuPresenter profile must be rejected"),
            Err(err) => err,
        };

        assert!(err.message().contains("CpuPresenter"));
        assert!(shutdown_called.get());
    }

    #[test]
    fn caps_pipeline_forwards_to_legacy_supports_pixel_present() {
        let context = ShutdownTrackingContext {
            shutdown_called: Rc::new(Cell::new(false)),
            pipeline: RenderPipelineProfile::CpuUploadPresent,
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
                    pipeline: RenderPipelineProfile::CpuPresenter,
                }) as Box<dyn IGraphicsContext>)
            },
        ) {
            Ok(_) => panic!("CpuPresenter must fail engine creation"),
            Err(report) => {
                assert!(shutdown_called.get());
                assert!(!report.failures.is_empty());
            }
        }
    }

    struct InitFailingContext {
        shutdown_called: Rc<Cell<bool>>,
    }

    impl IGraphicsContext for InitFailingContext {
        fn caps(&self) -> GraphicsContextCaps {
            GraphicsContextCaps::cpu_upload_present(GraphicsBackend::D3d11, 1.0)
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
                assert!(!report.failures.is_empty());
            }
        }
    }
}
