//! GPU graphics bootstrap — sole owner of the init-time backend probe loop (P6.7 M3).

use crate::core::{Errc, Error, Result};
use crate::diagnostics::PendingFailureQueue;
use crate::draw::renderer::factory::create_renderer;
use crate::draw::renderer::RenderTarget;
use crate::draw::renderer::Renderer;
use crate::native::factory::{
    describe_backend_availability, gpu_recipe_candidates, try_create_gpu_recipe_with_queue,
    GraphicsRecipe,
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
    RendererCreate,
    RendererInitialize,
}

impl ProbeStage {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Unspecified => "unspecified",
            Self::CandidateSelection => "candidate_selection",
            Self::ContextCreate => "context_create",
            Self::RendererCreate => "renderer_create",
            Self::RendererInitialize => "renderer_initialize",
        }
    }
}

/// Successful GPU bootstrap result.
pub struct GpuBootstrap {
    pub renderer: Renderer,
    pub selected: GraphicsBackend,
    pub selected_recipe: GraphicsRecipe,
    pub present_occlusion: PresentOcclusionSupport,
    pub report: ProbeReport,
}

/// Renderer-assembly stages shared by initial probe and runtime recipe recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RendererAssemblyStage {
    Create,
    Initialize,
}

impl RendererAssemblyStage {
    fn probe_stage(self) -> ProbeStage {
        match self {
            Self::Create => ProbeStage::RendererCreate,
            Self::Initialize => ProbeStage::RendererInitialize,
        }
    }
}

/// A typed renderer-assembly failure that preserves the lifecycle stage for
/// probe diagnostics while letting recovery return the underlying error.
pub(crate) struct RendererAssemblyFailure {
    stage: RendererAssemblyStage,
    error: Error,
}

impl RendererAssemblyFailure {
    pub(crate) fn into_error(self) -> Error {
        self.error
    }
}

/// Assembles and starts the unique renderer from one factory-created context.
///
/// Both bootstrap and runtime recovery use this one path so renderer-creation
/// failures, startup failures, and their checked cleanup cannot drift.
pub(crate) fn assemble_renderer(
    context: Box<dyn IGraphicsContext>,
    width: i32,
    height: i32,
) -> Result<Renderer, RendererAssemblyFailure> {
    let mut renderer = create_renderer(context).map_err(|error| RendererAssemblyFailure {
        stage: RendererAssemblyStage::Create,
        error,
    })?;
    if let Err(error) = renderer.initialize(width, height) {
        let error = match renderer.try_shutdown() {
            Ok(()) => error,
            Err(cleanup_error) => cleanup_error.with_source(error),
        };
        return Err(RendererAssemblyFailure {
            stage: RendererAssemblyStage::Initialize,
            error,
        });
    }
    Ok(renderer)
}

/// Probes GPU backends in platform order and returns the first working renderer.
///
/// CPU fallback (`Renderer::cpu`) stays in app; this function only handles native contexts.
pub fn bootstrap_renderer(
    surface: NativeSurfaceHandle,
    width: i32,
    height: i32,
    request: GraphicsBackend,
) -> Result<GpuBootstrap, ProbeReport> {
    bootstrap_renderer_with_pending(surface, width, height, request, PendingFailureQueue::new())
}

/// Bootstrap entry used by an application runtime that owns callback failure
/// delivery for every graphics context it creates.
pub(crate) fn bootstrap_renderer_with_pending(
    surface: NativeSurfaceHandle,
    width: i32,
    height: i32,
    request: GraphicsBackend,
    pending_failures: PendingFailureQueue,
) -> Result<GpuBootstrap, ProbeReport> {
    bootstrap_renderer_with(surface, width, height, request, |candidate| {
        try_create_gpu_recipe_with_queue(
            candidate,
            surface,
            width,
            height,
            pending_failures.clone(),
        )
    })
}

pub(crate) fn bootstrap_renderer_with<F>(
    _surface: NativeSurfaceHandle,
    width: i32,
    height: i32,
    request: GraphicsBackend,
    try_create: F,
) -> Result<GpuBootstrap, ProbeReport>
where
    F: FnMut(GraphicsRecipe) -> Result<Box<dyn IGraphicsContext>, Error>,
{
    bootstrap_renderer_with_candidates(
        width,
        height,
        request,
        gpu_recipe_candidates(request),
        try_create,
    )
}

pub(crate) fn bootstrap_renderer_with_candidates<F>(
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
        crate::core::log::info_fn(format_args!(
            "Graphics bootstrap: probing recipe {candidate}"
        ));
        let context = match try_create(candidate) {
            Ok(context) => context,
            Err(err) => {
                crate::core::log::warn_fn(format_args!(
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

        let renderer = match assemble_renderer(context, width, height) {
            Ok(renderer) => renderer,
            Err(failure) => {
                crate::core::log::warn_fn(format_args!(
                    "Graphics bootstrap: renderer assembly for recipe {selected} unavailable: {}",
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
        crate::core::log::info_fn(format_args!(
            "Graphics bootstrap: selected recipe {selected}; present_occlusion={present_occlusion}"
        ));
        return Ok(GpuBootstrap {
            renderer,
            selected: selected.backend,
            selected_recipe: selected,
            present_occlusion,
            report,
        });
    }

    Err(report)
}
