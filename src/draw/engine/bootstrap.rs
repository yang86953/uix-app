//! GPU graphics bootstrap — sole owner of the init-time backend probe loop (P6.7 M3).

use crate::core::{Errc, Error, Result};
use crate::draw::engine::factory::create_graphics_engine;
use crate::draw::traits::GraphicsEngine;
use crate::native::factory::{
    describe_backend_availability, gpu_recipe_candidates, try_create_gpu_recipe, GraphicsRecipe,
};
use crate::native::traits::present::{
    GraphicsBackend, IGraphicsContext, NativeSurfaceHandle, PresentOcclusionSupport,
};

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

    pub(crate) fn record_failure_at(
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
pub(crate) enum ProbeStage {
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
    pub present_occlusion: PresentOcclusionSupport,
    pub report: ProbeReport,
}

/// Engine-assembly stages shared by initial probe and runtime recipe recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphicsEngineAssemblyStage {
    Create,
    Initialize,
}

impl GraphicsEngineAssemblyStage {
    fn probe_stage(self) -> ProbeStage {
        match self {
            Self::Create => ProbeStage::EngineCreate,
            Self::Initialize => ProbeStage::EngineInitialize,
        }
    }
}

/// A typed engine-assembly failure that preserves the lifecycle stage for
/// probe diagnostics while letting recovery return the underlying error.
pub(crate) struct GraphicsEngineAssemblyFailure {
    stage: GraphicsEngineAssemblyStage,
    error: Error,
}

impl GraphicsEngineAssemblyFailure {
    pub(crate) fn into_error(self) -> Error {
        self.error
    }
}

/// Assembles and starts an engine from one factory-created context.
///
/// Both bootstrap and runtime recovery use this one path so engine-creation
/// failures, startup failures, and their checked cleanup cannot drift.
pub(crate) fn assemble_graphics_engine(
    context: Box<dyn IGraphicsContext>,
    width: i32,
    height: i32,
) -> Result<Box<dyn GraphicsEngine>, GraphicsEngineAssemblyFailure> {
    let mut engine =
        create_graphics_engine(context).map_err(|error| GraphicsEngineAssemblyFailure {
            stage: GraphicsEngineAssemblyStage::Create,
            error,
        })?;
    if let Err(error) = engine.initialize(width, height) {
        let error = match engine.try_shutdown() {
            Ok(()) => error,
            Err(cleanup_error) => cleanup_error.with_source(error),
        };
        return Err(GraphicsEngineAssemblyFailure {
            stage: GraphicsEngineAssemblyStage::Initialize,
            error,
        });
    }
    Ok(engine)
}

/// Probes GPU backends in platform order and returns the first working engine.
///
/// CPU fallback (`SoftwareEngine`) stays in app; this function only handles GPU paths.
pub fn bootstrap_graphics_engine(
    surface: NativeSurfaceHandle,
    width: i32,
    height: i32,
    request: GraphicsBackend,
) -> Result<GpuBootstrap, ProbeReport> {
    bootstrap_graphics_engine_with(surface, width, height, request, |candidate| {
        try_create_gpu_recipe(candidate, surface, width, height)
    })
}

pub(crate) fn bootstrap_graphics_engine_with<F>(
    _surface: NativeSurfaceHandle,
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

pub(crate) fn bootstrap_graphics_engine_with_candidates<F>(
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
        let availability = describe_backend_availability(request)
            .map(|reason| format!(" ({reason})"))
            .unwrap_or_default();
        report.record_no_candidates(
            request,
            &Error::new(
                Errc::PlatformError,
                format!(
                    "Graphics bootstrap: no GPU backend candidates for {request}{availability}"
                ),
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
        let present_occlusion = caps.present_occlusion;

        let engine = match assemble_graphics_engine(context, width, height) {
            Ok(engine) => engine,
            Err(failure) => {
                crate::core::log::warn_fn(format!(
                    "Graphics bootstrap: engine assembly for recipe {selected} unavailable: {}",
                    failure.error.what()
                ));
                report.record_failure_at(
                    candidate,
                    failure.stage.probe_stage(),
                    Some(selected),
                    &failure.error,
                );
                continue;
            }
        };
        crate::core::log::info_fn(format!(
            "Graphics bootstrap: selected recipe {selected}; present_occlusion={present_occlusion}"
        ));
        return Ok(GpuBootstrap {
            engine,
            selected: selected.backend,
            selected_recipe: selected,
            present_occlusion,
            report,
        });
    }

    Err(report)
}
