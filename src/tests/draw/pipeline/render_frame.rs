use crate::draw::compositor::ScenePaint;
use crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
use crate::draw::pipeline::render_frame::*;
use crate::draw::pipeline::{InvalidationSource, RenderMetrics};
use crate::draw::traits::{Canvas2D, GraphicsCapabilities};
use crate::draw::traits::{GraphicsEngine, UpdateStrategy};
use crate::tests::common::*;

struct RecordingEngine {
    canvas: NoopCanvas2D,
    events: Vec<&'static str>,
    partial_redraw: bool,
    scroll_memmove: bool,
    begin_failure: Option<crate::draw::engine::GraphicsFailure>,
    begin_outcome: Option<RenderOutcome>,
    encoded_frame_failure: Option<Error>,
    encoded_frames: Vec<Vec<crate::draw::pipeline::FrameCommand>>,
    end_outcome: RenderOutcome,
    presentation_mode: crate::draw::traits::PresentationMode,
    last_begin_strategy: Option<UpdateStrategy>,
    end_damages: Vec<DamageRegion>,
}

impl RecordingEngine {
    fn new() -> Self {
        Self {
            canvas: NoopCanvas2D,
            events: Vec::new(),
            partial_redraw: true,
            scroll_memmove: true,
            begin_failure: None,
            begin_outcome: None,
            encoded_frame_failure: None,
            encoded_frames: Vec::new(),
            end_outcome: RenderOutcome::Present(DamageRegion::full()),
            presentation_mode: crate::draw::traits::PresentationMode::EngineManaged,
            last_begin_strategy: None,
            end_damages: Vec::new(),
        }
    }

    fn without_partial_redraw(mut self) -> Self {
        self.partial_redraw = false;
        self
    }

    fn without_scroll_memmove(mut self) -> Self {
        self.scroll_memmove = false;
        self
    }

    fn with_end_outcome(mut self, outcome: RenderOutcome) -> Self {
        self.end_outcome = outcome;
        self
    }

    fn with_begin_failure(mut self, failure: crate::draw::engine::GraphicsFailure) -> Self {
        self.begin_failure = Some(failure);
        self
    }

    fn with_begin_outcome(mut self, outcome: RenderOutcome) -> Self {
        self.begin_outcome = Some(outcome);
        self
    }

    fn with_external_presenter(mut self) -> Self {
        self.presentation_mode = crate::draw::traits::PresentationMode::ExternalPresenter;
        self
    }

    fn with_encoded_frame_failure(mut self, error: Error) -> Self {
        self.encoded_frame_failure = Some(error);
        self
    }
}

impl GraphicsEngine for RecordingEngine {
    fn initialize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
        Ok(())
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn resize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
        Ok(())
    }

    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        self.events.push("begin");
        self.last_begin_strategy = Some(strategy.clone());
        if let Some(failure) = &self.begin_failure {
            return RenderOutcome::Failed(failure.clone());
        }
        if let Some(outcome) = &self.begin_outcome {
            return outcome.clone();
        }
        match strategy {
            UpdateStrategy::FullRedraw => RenderOutcome::FrameReady(DamageRegion::full()),
            UpdateStrategy::DirtyRects(rects) => {
                RenderOutcome::FrameReady(DamageRegion::partial(rects))
            }
            UpdateStrategy::ScrollCopies { dirty_rects, .. } => {
                RenderOutcome::FrameReady(DamageRegion::partial(dirty_rects))
            }
        }
    }

    fn end_frame(&mut self, present_damage: &DamageRegion) -> RenderOutcome {
        self.events.push("end");
        self.end_damages.push(present_damage.clone());
        match &self.end_outcome {
            RenderOutcome::Idle => RenderOutcome::Idle,
            RenderOutcome::FrameReady(_) => RenderOutcome::FrameReady(present_damage.clone()),
            RenderOutcome::PresentPending(_) => {
                RenderOutcome::PresentPending(present_damage.clone())
            }
            RenderOutcome::Present(_) => RenderOutcome::Present(present_damage.clone()),
            RenderOutcome::Failed(error) => RenderOutcome::Failed(error.clone()),
        }
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.events.push("canvas");
        &mut self.canvas
    }

    fn capabilities(&self) -> GraphicsCapabilities {
        GraphicsCapabilities {
            presentation_mode: self.presentation_mode,
            partial_redraw: self.partial_redraw,
            offscreen: true,
            scroll_memmove: self.scroll_memmove,
        }
    }

    fn try_execute_encoded_frame(
        &mut self,
        encoder: &crate::draw::pipeline::FrameEncoder,
    ) -> Result<crate::draw::pipeline::EncodedFrameExecution, Error> {
        self.events.push("encoded_frame");
        self.encoded_frames.push(encoder.commands().to_vec());
        match &self.encoded_frame_failure {
            Some(error) => Err(error.clone()),
            None => Ok(crate::draw::pipeline::EncodedFrameExecution::Executed),
        }
    }

    fn create_offscreen(&mut self, _width: i32, _height: i32) -> Option<crate::draw::ImageHandle> {
        Some(crate::draw::ImageHandle(1))
    }

    fn offscreen_canvas(
        &mut self,
        _handle: &crate::draw::ImageHandle,
    ) -> Option<&mut dyn Canvas2D> {
        Some(&mut self.canvas)
    }
}

/// Captures the real WGL drawable immediately after the sole main
/// `FrameEncoder` execution. `SwapBuffers` does not promise that the next
/// back buffer remains readable, so post-present readback would test buffer
/// ownership rather than the producer-to-present contract.
#[cfg(feature = "opengles")]
struct CaptureBeforePresentEngine {
    inner: crate::draw::gpu_engine::GpuEngine,
    encoded_frames: Vec<Vec<u32>>,
}

#[cfg(feature = "opengles")]
impl GraphicsEngine for CaptureBeforePresentEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.initialize(width, height)
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.inner.try_shutdown()
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.resize(width, height)
    }

    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        self.inner.begin_frame(strategy)
    }

    fn end_frame(&mut self, damage: &DamageRegion) -> RenderOutcome {
        self.inner.end_frame(damage)
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.inner.canvas_2d()
    }

    fn capabilities(&self) -> GraphicsCapabilities {
        self.inner.capabilities()
    }

    fn try_execute_encoded_frame(
        &mut self,
        encoder: &crate::draw::pipeline::FrameEncoder,
    ) -> Result<crate::draw::pipeline::EncodedFrameExecution, Error> {
        let execution = self.inner.try_execute_encoded_frame(encoder)?;
        if matches!(
            execution,
            crate::draw::pipeline::EncodedFrameExecution::Executed
        ) {
            let pixels = self
                .inner
                .session_mut()
                .native_gpu_backend_mut()
                .expect("WGL must retain the native backend")
                .try_readback()?;
            self.encoded_frames.push(pixels);
        }
        Ok(execution)
    }
}

struct EncodedSoftwareEngine {
    inner: SoftwareEngine,
    encoded_picture_executions: usize,
    encoded_frame_executions: usize,
    encoded_frames: Vec<Vec<crate::draw::pipeline::FrameCommand>>,
}

impl EncodedSoftwareEngine {
    fn new() -> Self {
        Self {
            inner: SoftwareEngine::new(),
            encoded_picture_executions: 0,
            encoded_frame_executions: 0,
            encoded_frames: Vec::new(),
        }
    }
}

impl GraphicsEngine for EncodedSoftwareEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.initialize(width, height)
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.inner.try_shutdown()
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.resize(width, height)
    }

    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        self.inner.begin_frame(strategy)
    }

    fn end_frame(&mut self, damage: &DamageRegion) -> RenderOutcome {
        self.inner.end_frame(damage)
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.inner.canvas_2d()
    }

    fn capabilities(&self) -> GraphicsCapabilities {
        self.inner.capabilities()
    }

    fn copy_frame_pixels(&self) -> Option<(Vec<u32>, i32)> {
        self.inner.copy_frame_pixels()
    }

    fn create_offscreen(&mut self, width: i32, height: i32) -> Option<crate::draw::ImageHandle> {
        self.inner.create_offscreen(width, height)
    }

    fn destroy_offscreen(&mut self, handle: crate::draw::ImageHandle) {
        self.inner.destroy_offscreen(handle);
    }

    fn offscreen_canvas(&mut self, handle: &crate::draw::ImageHandle) -> Option<&mut dyn Canvas2D> {
        self.inner.offscreen_canvas(handle)
    }

    fn begin_offscreen_paint(&mut self, handle: &crate::draw::ImageHandle) -> bool {
        self.inner.begin_offscreen_paint(handle)
    }

    fn flush_offscreen_paint(&mut self, handle: &crate::draw::ImageHandle) {
        self.inner.flush_offscreen_paint(handle);
    }

    fn end_offscreen_paint(&mut self) {
        self.inner.end_offscreen_paint();
    }

    fn blit_offscreen_src(
        &mut self,
        handle: &crate::draw::ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
    ) {
        self.inner.blit_offscreen_src(handle, src_rect, dst_rect);
    }

    fn try_execute_encoded_picture(
        &mut self,
        handle: &crate::draw::ImageHandle,
        encoder: &crate::draw::pipeline::FrameEncoder,
    ) -> Result<crate::draw::pipeline::EncodedPictureExecution, Error> {
        let result = self.inner.try_execute_encoded_picture(handle, encoder)?;
        if matches!(
            result,
            crate::draw::pipeline::EncodedPictureExecution::Executed
        ) {
            self.encoded_picture_executions += 1;
        }
        Ok(result)
    }

    fn try_execute_encoded_frame(
        &mut self,
        encoder: &crate::draw::pipeline::FrameEncoder,
    ) -> Result<crate::draw::pipeline::EncodedFrameExecution, Error> {
        self.encoded_frames.push(encoder.commands().to_vec());
        let result = self.inner.try_execute_encoded_frame(encoder)?;
        if matches!(
            result,
            crate::draw::pipeline::EncodedFrameExecution::Executed
        ) {
            self.encoded_frame_executions += 1;
        }
        Ok(result)
    }
}

#[test]
fn frame_renderer_stops_before_paint_or_end_when_begin_frame_fails() {
    let failure = crate::draw::engine::GraphicsFailure::SurfaceLost(Error::new(
        crate::core::error::Errc::GraphicsSurfaceLost,
        "injected begin failure",
    ));
    let mut renderer = FrameRenderer::new();
    let mut engine = RecordingEngine::new().with_begin_failure(failure);
    let scene = EmptyScene::new();
    let tokens = MockTokens;
    let fonts = FontService::new();
    let images = ImageService::new();

    let output = renderer.render_frame(
        &mut engine,
        &scene,
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &DirtyRegion::full(),
            tree_version: 0,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    assert!(matches!(output.outcome, RenderOutcome::Failed(_)));
    assert_eq!(output.inv_source, InvalidationSource::None);
    assert_eq!(engine.events, ["begin"]);
}

#[test]
fn frame_renderer_does_not_report_present_when_engine_declines_frame_completion() {
    let mut renderer = FrameRenderer::new();
    let mut engine = RecordingEngine::new().with_end_outcome(RenderOutcome::Idle);
    let scene = EmptyScene::new();
    let tokens = MockTokens;
    let fonts = FontService::new();
    let images = ImageService::new();

    let output = renderer.render_frame(
        &mut engine,
        &scene,
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &DirtyRegion::full(),
            tree_version: 0,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    assert_eq!(output.outcome, RenderOutcome::Idle);
    assert_eq!(engine.events.last(), Some(&"end"));
}

#[test]
fn frame_renderer_rejects_final_presentation_from_begin_frame() {
    let mut renderer = FrameRenderer::new();
    let mut engine =
        RecordingEngine::new().with_begin_outcome(RenderOutcome::Present(DamageRegion::full()));
    let scene = EmptyScene::new();
    let tokens = MockTokens;
    let fonts = FontService::new();
    let images = ImageService::new();

    let output = renderer.render_frame(
        &mut engine,
        &scene,
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &DirtyRegion::full(),
            tree_version: 0,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    assert!(matches!(output.outcome, RenderOutcome::Failed(_)));
    assert_eq!(engine.events, ["begin"]);
}

#[test]
fn frame_renderer_rejects_ready_result_from_end_frame() {
    let mut renderer = FrameRenderer::new();
    let mut engine =
        RecordingEngine::new().with_end_outcome(RenderOutcome::FrameReady(DamageRegion::full()));
    let scene = EmptyScene::new();
    let tokens = MockTokens;
    let fonts = FontService::new();
    let images = ImageService::new();

    let output = renderer.render_frame(
        &mut engine,
        &scene,
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &DirtyRegion::full(),
            tree_version: 0,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    assert!(matches!(output.outcome, RenderOutcome::Failed(_)));
    assert_eq!(engine.events.last(), Some(&"end"));
}

#[test]
fn frame_renderer_preserves_typed_graphics_failure() {
    let failure = crate::draw::engine::GraphicsFailure::SurfaceLost(Error::new(
        crate::core::error::Errc::GraphicsSurfaceLost,
        "injected surface loss",
    ));
    let mut renderer = FrameRenderer::new();
    let mut engine = RecordingEngine::new().with_end_outcome(RenderOutcome::Failed(failure));
    let scene = EmptyScene::new();
    let tokens = MockTokens;
    let fonts = FontService::new();
    let images = ImageService::new();

    let output = renderer.render_frame(
        &mut engine,
        &scene,
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &DirtyRegion::full(),
            tree_version: 0,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    assert!(matches!(
        output.outcome,
        RenderOutcome::Failed(crate::draw::engine::GraphicsFailure::SurfaceLost(error))
            if error.code() == crate::core::error::Errc::GraphicsSurfaceLost
    ));
    assert_eq!(output.inv_source, InvalidationSource::None);
}

#[test]
fn root_and_direct_scene_execute_one_encoded_frame_before_final_present() {
    let mut renderer = FrameRenderer::new();
    let mut engine = RecordingEngine::new();
    let scene = EligiblePictureScene::direct();
    let tokens = MockTokens;
    let fonts = FontService::new();
    let images = ImageService::new();

    let output = renderer.render_frame(
        &mut engine,
        &scene,
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &DirtyRegion::full(),
            tree_version: 1,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    assert!(matches!(output.outcome, RenderOutcome::Present(_)));
    let encoded = engine
        .events
        .iter()
        .position(|event| *event == "encoded_frame")
        .expect("root/direct scene must reach the main FrameEncoder executor");
    let end = engine
        .events
        .iter()
        .position(|event| *event == "end")
        .expect("one final presentation boundary");
    assert!(
        encoded < end,
        "FrameEncoder must execute before final present"
    );
    let commands = engine
        .encoded_frames
        .last()
        .expect("main producer must record one command stream");
    assert!(matches!(
        commands.as_slice(),
        [
            crate::draw::pipeline::FrameCommand::Clear { .. },
            crate::draw::pipeline::FrameCommand::Native { .. },
            crate::draw::pipeline::FrameCommand::Native { .. },
        ]
    ));
}

#[cfg(feature = "d3d11")]
#[test]
fn frame_renderer_d3d11_warp_executes_direct_and_picture_recordings_before_present() {
    use crate::draw::backend::NativeGpuBackend;
    use crate::draw::gpu_engine::GpuEngine;

    if !crate::native::factory::d3d11_warp_test_context_available() {
        return;
    }

    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("FrameRenderer WARP recording test", 300, 300)
        .expect("window");
    let context = crate::native::factory::create_d3d11_warp_test_context(
        window.native_surface_ptr(),
        300,
        300,
    )
    .expect("D3D11 WARP context");
    let mut engine = GpuEngine::new(context).expect("native WARP engine");
    engine
        .initialize(300, 300)
        .expect("initialize native WARP engine");
    let tokens = MockTokens;
    let fonts = FontService::new();
    let images = ImageService::new();

    let mut direct_renderer = FrameRenderer::new();
    let direct_scene = EligiblePictureScene::mixed_direct_overlay();
    let direct = direct_renderer.render_frame(
        &mut engine,
        &direct_scene,
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &DirtyRegion::full(),
            tree_version: 1,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );
    assert!(matches!(direct.outcome, RenderOutcome::Present(_)));
    let direct_pixels = engine
        .session_mut()
        .backend_mut()
        .as_any_mut()
        .downcast_mut::<NativeGpuBackend>()
        .expect("D3D11 WARP must retain the native backend")
        .try_readback()
        .expect("read direct WARP frame");
    assert_eq!(
        direct_pixels[0],
        Color::from_rgb(160, 20, 20).premultiplied(),
        "direct root must reach the real FrameEncoder executor"
    );
    assert_eq!(
        direct_pixels[8 * 300 + 8],
        Color::from_rgb(20, 40, 220).premultiplied(),
        "direct child must retain painter order on the real target"
    );
    assert_eq!(
        direct_pixels[150 * 300 + 150],
        Color::from_rgb(30, 180, 80).premultiplied(),
        "direct root-level overlay must reach the real FrameEncoder executor"
    );

    let mut picture_renderer = FrameRenderer::new();
    let picture_scene = EligiblePictureScene::mixed();
    let picture = picture_renderer.render_frame(
        &mut engine,
        &picture_scene,
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &DirtyRegion::full(),
            tree_version: 1,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );
    assert!(matches!(picture.outcome, RenderOutcome::Present(_)));
    let picture_pixels = engine
        .session_mut()
        .backend_mut()
        .as_any_mut()
        .downcast_mut::<NativeGpuBackend>()
        .expect("D3D11 WARP must retain the native backend")
        .try_readback()
        .expect("read Picture WARP frame");
    assert_eq!(
        picture_pixels[0],
        Color::from_rgb(160, 20, 20).premultiplied(),
        "Picture root must reach the real FrameEncoder executor"
    );
    assert_eq!(
        picture_pixels[8 * 300 + 8],
        Color::from_rgb(20, 40, 220).premultiplied(),
        "Picture child must retain painter order on the real target"
    );
    assert_eq!(
        picture_pixels[150 * 300 + 150],
        Color::from_rgb(30, 180, 80).premultiplied(),
        "Picture CPU fallback must retain painter order on the real target"
    );

    engine.try_shutdown().expect("checked shutdown");
    window.close().expect("close WARP window");
}

#[test]
#[ignore = "legacy WGL readback fixture; production frame rendering uses wgpu"]
fn frame_renderer_wgl_executes_direct_and_picture_recordings_before_present() {
    use crate::draw::gpu_engine::GpuEngine;

    if std::env::consts::OS != "windows" {
        return;
    }

    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("FrameRenderer WGL recording test", 300, 300)
        .expect("window");
    let context = crate::native::factory::create_gpu_context_with_backend(
        window.native_surface_ptr(),
        300,
        300,
        GraphicsBackend::OpenGlEs,
    )
    .expect("WGL context");
    let mut engine = CaptureBeforePresentEngine {
        inner: GpuEngine::new(context).expect("native WGL engine"),
        encoded_frames: Vec::new(),
    };
    engine
        .initialize(300, 300)
        .expect("initialize native WGL engine");
    let tokens = MockTokens;
    let fonts = FontService::new();
    let images = ImageService::new();
    let rgba_to_aarrggbb = |pixel: u32| {
        (pixel & 0xFF00_0000)
            | ((pixel & 0x0000_00FF) << 16)
            | (pixel & 0x0000_FF00)
            | ((pixel & 0x00FF_0000) >> 16)
    };
    let logical_pixel = |pixels: &[u32], x: usize, y: usize| {
        // WGL readback has a bottom-left origin; FrameEncoder coordinates are
        // top-left logical pixels.
        pixels[(299 - y) * 300 + x]
    };

    let mut direct_renderer = FrameRenderer::new();
    let direct_scene = EligiblePictureScene::mixed_direct_overlay();
    let direct = direct_renderer.render_frame(
        &mut engine,
        &direct_scene,
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &DirtyRegion::full(),
            tree_version: 1,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );
    assert!(matches!(direct.outcome, RenderOutcome::Present(_)));
    let direct_pixels = engine
        .encoded_frames
        .last()
        .expect("capture direct WGL frame before present");
    assert_eq!(
        rgba_to_aarrggbb(logical_pixel(direct_pixels, 0, 0)),
        Color::from_rgb(160, 20, 20).premultiplied(),
        "direct root must reach the real FrameEncoder executor"
    );
    assert_eq!(
        rgba_to_aarrggbb(logical_pixel(direct_pixels, 8, 8)),
        Color::from_rgb(20, 40, 220).premultiplied(),
        "direct child must retain painter order on the real target"
    );
    assert_eq!(
        rgba_to_aarrggbb(logical_pixel(direct_pixels, 150, 150)),
        Color::from_rgb(30, 180, 80).premultiplied(),
        "direct root-level overlay must reach the real FrameEncoder executor"
    );

    let mut picture_renderer = FrameRenderer::new();
    let picture_scene = EligiblePictureScene::mixed();
    let picture = picture_renderer.render_frame(
        &mut engine,
        &picture_scene,
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &DirtyRegion::full(),
            tree_version: 1,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );
    assert!(matches!(picture.outcome, RenderOutcome::Present(_)));
    let picture_pixels = engine
        .encoded_frames
        .last()
        .expect("capture Picture WGL frame before present");
    assert_eq!(
        rgba_to_aarrggbb(logical_pixel(picture_pixels, 0, 0)),
        Color::from_rgb(160, 20, 20).premultiplied(),
        "Picture root must reach the real FrameEncoder executor"
    );
    assert_eq!(
        rgba_to_aarrggbb(logical_pixel(picture_pixels, 8, 8)),
        Color::from_rgb(20, 40, 220).premultiplied(),
        "Picture child must retain painter order on the real target"
    );
    assert_eq!(
        rgba_to_aarrggbb(logical_pixel(picture_pixels, 150, 150)),
        Color::from_rgb(30, 180, 80).premultiplied(),
        "Picture CPU fallback must retain painter order on the real target"
    );

    engine.try_shutdown().expect("checked shutdown");
    window.close().expect("close WGL window");
}

#[test]
fn main_frame_encoder_failure_skips_final_present_and_preserves_retry() {
    let mut renderer = FrameRenderer::new();
    let mut engine = RecordingEngine::new().with_encoded_frame_failure(Error::new(
        crate::core::Errc::GraphicsSurfaceLost,
        "injected main FrameEncoder failure",
    ));
    let scene = EligiblePictureScene::direct();
    let tokens = MockTokens;
    let fonts = FontService::new();
    let images = ImageService::new();

    let output = renderer.render_frame(
        &mut engine,
        &scene,
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &DirtyRegion::full(),
            tree_version: 1,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    assert!(matches!(output.outcome, RenderOutcome::Failed(_)));
    assert!(engine.events.contains(&"encoded_frame"));
    assert!(
        !engine.events.contains(&"end"),
        "failed main FrameEncoder execution cannot reach final present"
    );

    engine.encoded_frame_failure = None;
    let retry = renderer.render_frame(
        &mut engine,
        &scene,
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &DirtyRegion::full(),
            tree_version: 1,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );
    assert!(matches!(retry.outcome, RenderOutcome::Present(_)));
    assert_eq!(
        engine
            .events
            .iter()
            .filter(|event| **event == "encoded_frame")
            .count(),
        2,
        "retry must re-execute the dirty main FrameEncoder rather than reuse a failed frame"
    );
}

#[test]
fn cached_picture_is_consumed_by_the_main_frame_encoder_before_present() {
    let mut renderer = FrameRenderer::new();
    let mut engine = EncodedSoftwareEngine::new();
    engine.initialize(300, 300).expect("software init");
    let scene = EligiblePictureScene::new();
    let tokens = MockTokens;
    let fonts = FontService::new();
    let images = ImageService::new();

    let first = renderer.render_frame(
        &mut engine,
        &scene,
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &DirtyRegion::full(),
            tree_version: 1,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );
    assert!(matches!(first.outcome, RenderOutcome::PresentPending(_)));
    assert_eq!(engine.encoded_picture_executions, 0);
    assert_eq!(engine.encoded_frame_executions, 1);

    // The reference compositor may reuse a cached Picture, but its result is
    // still consumed only through the one main-surface FrameEncoder.
    scene.root_dirty.set(false);
    scene.child_dirty.set(true);
    let second = renderer.render_frame(
        &mut engine,
        &scene,
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &DirtyRegion::full(),
            tree_version: 1,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    assert!(matches!(second.outcome, RenderOutcome::PresentPending(_)));
    assert_eq!(engine.encoded_picture_executions, 0);
    assert_eq!(engine.encoded_frame_executions, 2);
    let pixels = engine
        .inner
        .session()
        .cpu_backend()
        .expect("CPU backend")
        .pixels();
    assert_eq!(pixels[0], Color::from_rgb(160, 20, 20).premultiplied());
    assert_eq!(
        pixels[8 * 300 + 8],
        Color::from_rgb(20, 40, 220).premultiplied()
    );
}

struct EmptyScene {
    region: DirtyRegion,
}

impl EmptyScene {
    fn new() -> Self {
        Self {
            region: DirtyRegion::empty(),
        }
    }
}

impl ScenePaint for EmptyScene {
    fn root_id(&self) -> Option<crate::draw::pipeline::NodeId> {
        None
    }
    fn tree_version(&self) -> u64 {
        0
    }
    fn dirty_region(&self) -> DirtyRegion {
        self.region.clone()
    }
    fn node_visible(&self, _: crate::draw::pipeline::NodeId) -> bool {
        false
    }
    fn node_frame(&self, _: crate::draw::pipeline::NodeId) -> Rect {
        Rect::zero()
    }
    fn node_dirty(&self, _: crate::draw::pipeline::NodeId) -> bool {
        false
    }
    fn node_z_index(&self, _: crate::draw::pipeline::NodeId) -> i32 {
        0
    }
    fn node_children(&self, _: crate::draw::pipeline::NodeId) -> &[crate::draw::pipeline::NodeId] {
        &[]
    }
    fn children_clip(&self, _: crate::draw::pipeline::NodeId, _: Rect) -> Option<Rect> {
        None
    }
    fn dirty_rect(&self, _: crate::draw::pipeline::NodeId, frame: Rect) -> Rect {
        frame
    }
    fn scroll_offset(&self, _: crate::draw::pipeline::NodeId) -> Option<(f32, f32)> {
        None
    }
    fn focused_node(&self) -> Option<crate::draw::pipeline::NodeId> {
        None
    }
    fn node_focusable(&self, _: crate::draw::pipeline::NodeId) -> bool {
        false
    }
    fn hit_test(&self, _: Point) -> Option<crate::draw::pipeline::NodeId> {
        None
    }
    fn parent(&self, _: crate::draw::pipeline::NodeId) -> Option<crate::draw::pipeline::NodeId> {
        None
    }
    fn paint(&self, _: crate::draw::pipeline::NodeId, _: Rect, _: &mut PaintContext<'_>) {}
}

struct EligiblePictureScene {
    root_dirty: Cell<bool>,
    child_dirty: Cell<bool>,
    include_cpu_fallback: bool,
    cpu_fallback_is_overlay: bool,
    picture_policy: crate::draw::compositor::PicturePolicy,
}

impl EligiblePictureScene {
    fn new() -> Self {
        Self {
            root_dirty: Cell::new(true),
            child_dirty: Cell::new(false),
            include_cpu_fallback: false,
            cpu_fallback_is_overlay: false,
            picture_policy: crate::draw::compositor::PicturePolicy::Eligible,
        }
    }

    fn direct() -> Self {
        Self {
            picture_policy: crate::draw::compositor::PicturePolicy::Never,
            ..Self::new()
        }
    }

    #[cfg(any(feature = "d3d11", feature = "opengles"))]
    fn mixed() -> Self {
        Self {
            include_cpu_fallback: true,
            ..Self::new()
        }
    }

    #[cfg(any(feature = "d3d11", feature = "opengles"))]
    fn mixed_direct() -> Self {
        Self {
            picture_policy: crate::draw::compositor::PicturePolicy::Never,
            ..Self::mixed()
        }
    }

    #[cfg(any(feature = "d3d11", feature = "opengles"))]
    fn mixed_direct_overlay() -> Self {
        Self {
            cpu_fallback_is_overlay: true,
            ..Self::mixed_direct()
        }
    }
}

impl ScenePaint for EligiblePictureScene {
    fn root_id(&self) -> Option<crate::draw::pipeline::NodeId> {
        Some(crate::draw::pipeline::NodeId::new(1))
    }

    fn tree_version(&self) -> u64 {
        1
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::full()
    }

    fn node_visible(&self, id: crate::draw::pipeline::NodeId) -> bool {
        (1..=8).any(|slot| id == crate::draw::pipeline::NodeId::new(slot))
    }

    fn node_is_overlay(&self, id: crate::draw::pipeline::NodeId) -> bool {
        self.cpu_fallback_is_overlay && id == crate::draw::pipeline::NodeId::new(3)
    }

    fn node_frame(&self, _: crate::draw::pipeline::NodeId) -> Rect {
        Rect::new(0.0, 0.0, 300.0, 300.0)
    }

    fn node_dirty(&self, id: crate::draw::pipeline::NodeId) -> bool {
        if id == crate::draw::pipeline::NodeId::new(1) {
            self.root_dirty.get()
        } else if id == crate::draw::pipeline::NodeId::new(2) {
            self.child_dirty.get()
        } else {
            false
        }
    }

    fn node_z_index(&self, _: crate::draw::pipeline::NodeId) -> i32 {
        0
    }

    fn node_children(&self, id: crate::draw::pipeline::NodeId) -> &[crate::draw::pipeline::NodeId] {
        static CHILDREN: [crate::draw::pipeline::NodeId; 7] = [
            crate::draw::pipeline::NodeId::new(2),
            crate::draw::pipeline::NodeId::new(3),
            crate::draw::pipeline::NodeId::new(4),
            crate::draw::pipeline::NodeId::new(5),
            crate::draw::pipeline::NodeId::new(6),
            crate::draw::pipeline::NodeId::new(7),
            crate::draw::pipeline::NodeId::new(8),
        ];
        if id == crate::draw::pipeline::NodeId::new(1) {
            &CHILDREN
        } else {
            &[]
        }
    }

    fn node_picture_policy(
        &self,
        _: crate::draw::pipeline::NodeId,
    ) -> crate::draw::compositor::PicturePolicy {
        self.picture_policy
    }

    fn children_clip(&self, _: crate::draw::pipeline::NodeId, _: Rect) -> Option<Rect> {
        None
    }

    fn dirty_rect(&self, _: crate::draw::pipeline::NodeId, frame: Rect) -> Rect {
        frame
    }

    fn scroll_offset(&self, _: crate::draw::pipeline::NodeId) -> Option<(f32, f32)> {
        None
    }

    fn focused_node(&self) -> Option<crate::draw::pipeline::NodeId> {
        None
    }

    fn node_focusable(&self, _: crate::draw::pipeline::NodeId) -> bool {
        false
    }

    fn hit_test(&self, _: Point) -> Option<crate::draw::pipeline::NodeId> {
        None
    }

    fn parent(&self, _: crate::draw::pipeline::NodeId) -> Option<crate::draw::pipeline::NodeId> {
        None
    }

    fn paint(&self, id: crate::draw::pipeline::NodeId, frame: Rect, ctx: &mut PaintContext<'_>) {
        if id == crate::draw::pipeline::NodeId::new(1) {
            ctx.fill_rect(frame, Color::from_rgb(160, 20, 20), None);
        } else if id == crate::draw::pipeline::NodeId::new(2) {
            ctx.fill_rect(
                Rect::new(8.0, 8.0, 16.0, 16.0),
                Color::from_rgb(20, 40, 220),
                None,
            );
        } else if id == crate::draw::pipeline::NodeId::new(3) && self.include_cpu_fallback {
            ctx.fill_circle(150.0, 150.0, 24.0, Color::from_rgb(30, 180, 80));
        }
    }
}

struct MockTokens;

impl crate::draw::painting::IColorTokens for MockTokens {
    fn color_primary(&self) -> Color {
        Color::blue()
    }
    fn color_primary_hover(&self) -> Color {
        Color::blue()
    }
    fn color_primary_active(&self) -> Color {
        Color::blue()
    }
    fn color_primary_bg(&self) -> Color {
        Color::blue()
    }
    fn color_primary_border(&self) -> Color {
        Color::blue()
    }
    fn color_bg_container(&self) -> Color {
        Color::white()
    }
    fn color_bg_elevated(&self) -> Color {
        Color::white()
    }
    fn color_bg_raised(&self) -> Color {
        Color::white()
    }
    fn color_bg_overlay(&self) -> Color {
        Color::white()
    }
    fn color_bg_layout(&self) -> Color {
        Color::white()
    }
    fn color_bg_spotlight(&self) -> Color {
        Color::white()
    }
    fn color_bg_mask(&self) -> Color {
        Color::white()
    }
    fn color_border(&self) -> Color {
        Color::black()
    }
    fn color_border_secondary(&self) -> Color {
        Color::black()
    }
    fn color_fill(&self) -> Color {
        Color::black()
    }
    fn color_fill_secondary(&self) -> Color {
        Color::black()
    }
    fn color_fill_tertiary(&self) -> Color {
        Color::black()
    }
    fn color_fill_quaternary(&self) -> Color {
        Color::black()
    }
    fn color_text(&self) -> Color {
        Color::black()
    }
    fn color_text_secondary(&self) -> Color {
        Color::black()
    }
    fn color_text_tertiary(&self) -> Color {
        Color::black()
    }
    fn color_text_quaternary(&self) -> Color {
        Color::black()
    }
    fn color_white(&self) -> Color {
        Color::white()
    }
    fn color_black(&self) -> Color {
        Color::black()
    }
    fn color_shadow(&self) -> Color {
        Color::black()
    }
    fn color_shadow_secondary(&self) -> Color {
        Color::black()
    }
    fn color_success(&self) -> Color {
        Color::green()
    }
    fn color_success_bg(&self) -> Color {
        Color::green()
    }
    fn color_success_border(&self) -> Color {
        Color::green()
    }
    fn color_warning(&self) -> Color {
        Color::from_rgb(255, 255, 0)
    }
    fn color_warning_bg(&self) -> Color {
        Color::from_rgb(255, 255, 0)
    }
    fn color_warning_border(&self) -> Color {
        Color::from_rgb(255, 255, 0)
    }
    fn color_error(&self) -> Color {
        Color::red()
    }
    fn color_error_bg(&self) -> Color {
        Color::red()
    }
    fn color_error_border(&self) -> Color {
        Color::red()
    }
    fn color_info(&self) -> Color {
        Color::blue()
    }
    fn color_info_bg(&self) -> Color {
        Color::blue()
    }
    fn color_info_border(&self) -> Color {
        Color::blue()
    }
    fn color_link(&self) -> Color {
        Color::blue()
    }
    fn color_link_hover(&self) -> Color {
        Color::blue()
    }
    fn color_link_active(&self) -> Color {
        Color::blue()
    }
}

impl crate::draw::painting::ITypographyTokens for MockTokens {
    fn font_family(&self) -> &str {
        "sans"
    }
}

impl crate::draw::painting::IBoxShadowTokens for MockTokens {
    fn box_shadow(&self) -> crate::draw::painting::ShadowToken {
        crate::draw::painting::ShadowToken::none()
    }
    fn box_shadow_secondary(&self) -> crate::draw::painting::ShadowToken {
        crate::draw::painting::ShadowToken::none()
    }
}

impl crate::draw::painting::ISpacingTokens for MockTokens {}

impl crate::draw::painting::ThemeTokens for MockTokens {}

#[test]
fn first_frame_expands_partial_dirty_to_full_paint_region() {
    struct PartialDirtyScene {
        region: DirtyRegion,
    }

    impl PartialDirtyScene {
        fn new() -> Self {
            let mut region = DirtyRegion::empty();
            region.add_rect(Rect::new(0.0, 0.0, 50.0, 50.0));
            Self { region }
        }
    }

    impl ScenePaint for PartialDirtyScene {
        fn root_id(&self) -> Option<crate::draw::pipeline::NodeId> {
            Some(crate::draw::pipeline::NodeId::new(1))
        }
        fn tree_version(&self) -> u64 {
            1
        }
        fn dirty_region(&self) -> DirtyRegion {
            self.region.clone()
        }
        fn node_visible(&self, _: crate::draw::pipeline::NodeId) -> bool {
            true
        }
        fn node_frame(&self, id: crate::draw::pipeline::NodeId) -> Rect {
            match id {
                id if id == crate::draw::pipeline::NodeId::new(1) => {
                    Rect::new(0.0, 0.0, 100.0, 100.0)
                }
                id if id == crate::draw::pipeline::NodeId::new(2) => {
                    Rect::new(200.0, 200.0, 30.0, 30.0)
                }
                _ => Rect::zero(),
            }
        }
        fn node_dirty(&self, _: crate::draw::pipeline::NodeId) -> bool {
            false
        }
        fn node_z_index(&self, _: crate::draw::pipeline::NodeId) -> i32 {
            0
        }
        fn node_children(
            &self,
            id: crate::draw::pipeline::NodeId,
        ) -> &[crate::draw::pipeline::NodeId] {
            static ROOT_KIDS: [crate::draw::pipeline::NodeId; 1] =
                [crate::draw::pipeline::NodeId::new(2)];
            static EMPTY: [crate::draw::pipeline::NodeId; 0] = [];
            if id == crate::draw::pipeline::NodeId::new(1) {
                &ROOT_KIDS
            } else {
                &EMPTY
            }
        }
        fn children_clip(&self, _: crate::draw::pipeline::NodeId, _: Rect) -> Option<Rect> {
            None
        }
        fn dirty_rect(&self, _: crate::draw::pipeline::NodeId, frame: Rect) -> Rect {
            frame
        }
        fn scroll_offset(&self, _: crate::draw::pipeline::NodeId) -> Option<(f32, f32)> {
            None
        }
        fn focused_node(&self) -> Option<crate::draw::pipeline::NodeId> {
            None
        }
        fn node_focusable(&self, _: crate::draw::pipeline::NodeId) -> bool {
            false
        }
        fn hit_test(&self, _: Point) -> Option<crate::draw::pipeline::NodeId> {
            None
        }
        fn parent(
            &self,
            id: crate::draw::pipeline::NodeId,
        ) -> Option<crate::draw::pipeline::NodeId> {
            if id == crate::draw::pipeline::NodeId::new(2) {
                Some(crate::draw::pipeline::NodeId::new(1))
            } else {
                None
            }
        }
        fn paint(&self, _: crate::draw::pipeline::NodeId, _: Rect, _: &mut PaintContext<'_>) {}
    }

    let mut renderer = FrameRenderer::new();
    let mut engine = NullEngine::new();
    let _ = engine.initialize(256, 256);
    let tokens = MockTokens;
    let theme = ThemeSnapshot::new(&tokens);
    let fs = FontService::new();
    let img = ImageService::new();
    let scene = PartialDirtyScene::new();
    let partial = scene.dirty_region();
    assert!(!partial.full_frame);
    // 子节点在 scene 的 partial dirty 之外
    assert!(!partial.intersects(Rect::new(200.0, 200.0, 30.0, 30.0)));

    let out = renderer.render_frame(
        &mut engine,
        &scene,
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &partial,
            tree_version: 1,
            scroll_move: None,
            theme,
            font: FontHandle::default(),
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );
    assert_eq!(
        out.outcome,
        RenderOutcome::PresentPending(DamageRegion::full())
    );
}

#[test]
fn mock_scene_render_frame_does_not_panic() {
    let mut renderer = FrameRenderer::new();
    let mut engine = NullEngine::new();
    let _ = engine.initialize(64, 64);
    let tokens = MockTokens;
    let theme = ThemeSnapshot::new(&tokens);
    let fs = FontService::new();
    let img = ImageService::new();
    let region = DirtyRegion::full();
    let out = renderer.render_frame(
        &mut engine,
        &EmptyScene::new(),
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &region,
            tree_version: 0,
            scroll_move: None,
            theme,
            font: FontHandle::default(),
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );
    assert_eq!(
        out.outcome,
        RenderOutcome::PresentPending(DamageRegion::full())
    );
}

#[test]
fn first_frame_builds_layer_tree_when_scene_version_is_zero() {
    struct ZeroVersionScene {
        painted: std::cell::Cell<usize>,
    }

    impl ScenePaint for ZeroVersionScene {
        fn root_id(&self) -> Option<crate::draw::pipeline::NodeId> {
            Some(crate::draw::pipeline::NodeId::new(1))
        }
        fn tree_version(&self) -> u64 {
            0
        }
        fn dirty_region(&self) -> DirtyRegion {
            DirtyRegion::full()
        }
        fn node_visible(&self, _: crate::draw::pipeline::NodeId) -> bool {
            true
        }
        fn node_frame(&self, _: crate::draw::pipeline::NodeId) -> Rect {
            Rect::new(0.0, 0.0, 64.0, 64.0)
        }
        fn node_dirty(&self, _: crate::draw::pipeline::NodeId) -> bool {
            true
        }
        fn node_z_index(&self, _: crate::draw::pipeline::NodeId) -> i32 {
            0
        }
        fn node_children(
            &self,
            _: crate::draw::pipeline::NodeId,
        ) -> &[crate::draw::pipeline::NodeId] {
            &[]
        }
        fn children_clip(&self, _: crate::draw::pipeline::NodeId, _: Rect) -> Option<Rect> {
            None
        }
        fn dirty_rect(&self, _: crate::draw::pipeline::NodeId, frame: Rect) -> Rect {
            frame
        }
        fn scroll_offset(&self, _: crate::draw::pipeline::NodeId) -> Option<(f32, f32)> {
            None
        }
        fn focused_node(&self) -> Option<crate::draw::pipeline::NodeId> {
            None
        }
        fn node_focusable(&self, _: crate::draw::pipeline::NodeId) -> bool {
            false
        }
        fn hit_test(&self, _: Point) -> Option<crate::draw::pipeline::NodeId> {
            None
        }
        fn parent(
            &self,
            _: crate::draw::pipeline::NodeId,
        ) -> Option<crate::draw::pipeline::NodeId> {
            None
        }
        fn paint(&self, _: crate::draw::pipeline::NodeId, _: Rect, _: &mut PaintContext<'_>) {
            self.painted.set(self.painted.get() + 1);
        }
    }

    let mut renderer = FrameRenderer::new();
    let mut engine = NullEngine::new();
    let _ = engine.initialize(64, 64);
    let tokens = MockTokens;
    let theme = ThemeSnapshot::new(&tokens);
    let font_service = FontService::new();
    let image_service = ImageService::new();
    let scene = ZeroVersionScene {
        painted: std::cell::Cell::new(0),
    };
    let region = DirtyRegion::full();

    let out = renderer.render_frame(
        &mut engine,
        &scene,
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &region,
            tree_version: 0,
            scroll_move: None,
            theme,
            font: FontHandle::default(),
            font_service: &font_service,
            image_service: &image_service,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    assert_eq!(
        out.outcome,
        RenderOutcome::PresentPending(DamageRegion::full())
    );
    assert_eq!(scene.painted.get(), 1);
    assert!(renderer.layer_tree().is_ready());
}

#[test]
fn rendered_frame_with_empty_dirty_region_is_idle() {
    let mut renderer = FrameRenderer::new();
    let mut engine = NullEngine::new();
    let _ = engine.initialize(64, 64);
    let tokens = MockTokens;
    let theme = ThemeSnapshot::new(&tokens);
    let fs = FontService::new();
    let img = ImageService::new();
    let region = DirtyRegion::empty();
    let out = renderer.render_frame(
        &mut engine,
        &EmptyScene::new(),
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &region,
            tree_version: 0,
            scroll_move: None,
            theme,
            font: FontHandle::default(),
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    assert_eq!(out.outcome, RenderOutcome::Idle);
    assert_eq!(out.inv_source, InvalidationSource::None);
}

#[test]
fn rendered_frame_with_partial_dirty_outputs_padded_partial_damage() {
    let mut renderer = FrameRenderer::new();
    let mut engine = NullEngine::new();
    let _ = engine.initialize(64, 64);
    let tokens = MockTokens;
    let theme = ThemeSnapshot::new(&tokens);
    let fs = FontService::new();
    let img = ImageService::new();
    let mut region = DirtyRegion::empty();
    region.add_rect(Rect::new(3.0, 4.0, 5.0, 6.0));

    let out = renderer.render_frame(
        &mut engine,
        &EmptyScene::new(),
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &region,
            tree_version: 0,
            scroll_move: None,
            theme,
            font: FontHandle::default(),
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    assert_eq!(
        out.outcome,
        RenderOutcome::PresentPending(DamageRegion::partial(vec![Rect::new(2.0, 3.0, 7.0, 8.0)]))
    );
    assert_eq!(out.inv_source, InvalidationSource::DirtyRegion);
}

#[test]
fn multi_rect_dirty_expands_to_union_for_paint_and_damage() {
    let mut renderer = FrameRenderer::new();
    let mut engine = NullEngine::new();
    let _ = engine.initialize(64, 64);
    let tokens = MockTokens;
    let theme = ThemeSnapshot::new(&tokens);
    let fs = FontService::new();
    let img = ImageService::new();
    let mut region = DirtyRegion::empty();
    // 悬停项 + 远处定时器标脏 → 并集须覆盖中间侧栏项
    region.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0));
    region.add_rect(Rect::new(0.0, 80.0, 20.0, 20.0));

    let out = renderer.render_frame(
        &mut engine,
        &EmptyScene::new(),
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &region,
            tree_version: 0,
            scroll_move: None,
            theme,
            font: FontHandle::default(),
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    // for_paint_clear → [0,0,20,100]，再 pad ±1
    assert_eq!(
        out.outcome,
        RenderOutcome::PresentPending(DamageRegion::partial(vec![Rect::new(
            0.0, 0.0, 22.0, 102.0
        )]))
    );
}

#[test]
fn rendered_frame_with_scroll_move_adds_scroll_frame_to_partial_damage() {
    let mut renderer = FrameRenderer::new();
    let mut engine = NullEngine::new();
    let _ = engine.initialize(128, 128);
    let tokens = MockTokens;
    let theme = ThemeSnapshot::new(&tokens);
    let fs = FontService::new();
    let img = ImageService::new();
    let mut region = DirtyRegion::empty();
    region.add_rect(Rect::new(10.0, 20.0, 4.0, 5.0));

    let out = renderer.render_frame(
        &mut engine,
        &EmptyScene::new(),
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &region,
            tree_version: 0,
            scroll_move: Some(vec![(Rect::new(30.0, 40.0, 50.0, 60.0), 0.0, -12.0)]),
            theme,
            font: FontHandle::default(),
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    assert_eq!(
        out.outcome,
        RenderOutcome::PresentPending(DamageRegion::partial(vec![
            Rect::new(9.0, 19.0, 6.0, 7.0),
            Rect::new(29.0, 39.0, 52.0, 62.0),
        ]))
    );
    assert_eq!(out.inv_source, InvalidationSource::DirtyRegion);
}

#[test]
fn rendered_frame_keeps_every_same_frame_scroll_viewport_narrow() {
    let mut renderer = FrameRenderer::new();
    let mut engine = NullEngine::new();
    let _ = engine.initialize(320, 160);
    let tokens = MockTokens;
    let theme = ThemeSnapshot::new(&tokens);
    let fs = FontService::new();
    let img = ImageService::new();
    let region = DirtyRegion::empty();

    let out = renderer.render_frame(
        &mut engine,
        &EmptyScene::new(),
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &region,
            tree_version: 0,
            scroll_move: Some(vec![
                (Rect::new(10.0, 20.0, 80.0, 60.0), 0.0, 12.0),
                (Rect::new(200.0, 30.0, 70.0, 50.0), -8.0, 0.0),
            ]),
            theme,
            font: FontHandle::default(),
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    assert_eq!(
        out.outcome,
        RenderOutcome::PresentPending(DamageRegion::partial(vec![
            Rect::new(9.0, 19.0, 82.0, 62.0),
            Rect::new(199.0, 29.0, 72.0, 52.0),
        ]))
    );
}

/// GPU 主路径（!partial_redraw）绘制全帧：strategy=FullRedraw、不裁剪绘制，
/// 但 present damage 仍按真实 dirty 窄区提交（与绘制区解耦）。
#[test]
fn engine_without_partial_redraw_draws_full_frame_but_presents_narrow_damage() {
    let mut renderer = FrameRenderer::new();
    let mut engine = RecordingEngine::new().without_partial_redraw();
    let _ = engine.initialize(64, 64);
    let tokens = MockTokens;
    let theme = ThemeSnapshot::new(&tokens);
    let fs = FontService::new();
    let img = ImageService::new();
    let mut region = DirtyRegion::empty();
    region.add_rect(Rect::new(8.0, 8.0, 12.0, 12.0));

    let out = renderer.render_frame(
        &mut engine,
        &EmptyScene::new(),
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &region,
            tree_version: 0,
            scroll_move: None,
            theme,
            font: FontHandle::default(),
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    assert!(
        matches!(engine.last_begin_strategy, Some(UpdateStrategy::FullRedraw)),
        "GPU 主路径绘制全帧"
    );
    assert_eq!(
        out.outcome,
        RenderOutcome::Present(DamageRegion::partial(vec![Rect::new(7.0, 7.0, 14.0, 14.0)])),
        "present damage 仍按真实 dirty 窄区（pad ±1）"
    );
    assert_eq!(out.inv_source, InvalidationSource::DirtyRegion);
}

#[test]
fn first_frame_with_scroll_move_still_uses_full_damage() {
    let mut renderer = FrameRenderer::new();
    let mut engine = NullEngine::new();
    let _ = engine.initialize(128, 128);
    let tokens = MockTokens;
    let theme = ThemeSnapshot::new(&tokens);
    let fs = FontService::new();
    let img = ImageService::new();
    let mut region = DirtyRegion::empty();
    region.add_rect(Rect::new(1.0, 2.0, 3.0, 4.0));

    let out = renderer.render_frame(
        &mut engine,
        &EmptyScene::new(),
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &region,
            tree_version: 0,
            scroll_move: Some(vec![(Rect::new(10.0, 12.0, 30.0, 40.0), 0.0, -8.0)]),
            theme,
            font: FontHandle::default(),
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    assert_eq!(
        out.outcome,
        RenderOutcome::PresentPending(DamageRegion::full())
    );
    assert_eq!(out.inv_source, InvalidationSource::FirstFrame);
}

#[test]
fn debug_telemetry_draws_before_end_frame() {
    let mut renderer = FrameRenderer::new();
    let mut engine = RecordingEngine::new();
    let _ = engine.initialize(64, 64);
    let tokens = MockTokens;
    let theme = ThemeSnapshot::new(&tokens);
    let fs = FontService::new();
    let img = ImageService::new();
    let region = DirtyRegion::full();
    let metrics = RenderMetrics::default();

    let out = renderer.render_frame(
        &mut engine,
        &EmptyScene::new(),
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &region,
            tree_version: 0,
            scroll_move: None,
            theme,
            font: FontHandle::default(),
            font_service: &fs,
            image_service: &img,
            debug_mode: true,
            hover_pos: None,
            metrics: Some(&metrics),
        },
    );

    let first_canvas = engine
        .events
        .iter()
        .position(|event| *event == "canvas")
        .expect("debug telemetry should draw to canvas");
    let end = engine
        .events
        .iter()
        .position(|event| *event == "end")
        .expect("frame should end");
    assert!(
        first_canvas < end,
        "debug HUD must be drawn before end_frame"
    );
    assert_eq!(out.outcome, RenderOutcome::Present(DamageRegion::full()));
}

#[test]
fn frame_renderer_marks_external_presenter_output_as_pending() {
    let mut renderer = FrameRenderer::new();
    let mut engine = RecordingEngine::new().with_external_presenter();
    let tokens = MockTokens;
    let fonts = FontService::new();
    let images = ImageService::new();

    let output = renderer.render_frame(
        &mut engine,
        &EmptyScene::new(),
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &DirtyRegion::full(),
            tree_version: 0,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    assert_eq!(
        output.outcome,
        RenderOutcome::PresentPending(DamageRegion::full())
    );
    assert_eq!(output.inv_source, InvalidationSource::FirstFrame);
}

#[test]
fn partial_dirty_uses_dirty_rects_strategy_and_padded_damage() {
    let mut renderer = FrameRenderer::new();
    let mut engine = RecordingEngine::new();
    let _ = engine.initialize(64, 64);
    let tokens = MockTokens;
    let theme = ThemeSnapshot::new(&tokens);
    let fs = FontService::new();
    let img = ImageService::new();
    let mut region = DirtyRegion::empty();
    region.add_rect(Rect::new(10.0, 12.0, 8.0, 6.0));

    let out = renderer.render_frame(
        &mut engine,
        &EmptyScene::new(),
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &region,
            tree_version: 0,
            scroll_move: None,
            theme,
            font: FontHandle::default(),
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    match engine.last_begin_strategy.as_ref() {
        Some(UpdateStrategy::DirtyRects(rects)) => {
            assert_eq!(rects, &[Rect::new(10.0, 12.0, 8.0, 6.0)]);
        }
        other => panic!("expected DirtyRects, got {other:?}"),
    }
    assert_eq!(
        engine.end_damages.last(),
        Some(&DamageRegion::partial(vec![Rect::new(
            9.0, 11.0, 10.0, 8.0
        )]))
    );
    assert_eq!(
        out.outcome,
        RenderOutcome::Present(DamageRegion::partial(vec![Rect::new(9.0, 11.0, 10.0, 8.0)]))
    );
}

#[test]
fn dirty_frame_paint_is_faster_than_full_frame_on_dense_scene() {
    use crate::draw::SoftwareEngine;
    use std::time::Instant;

    struct DenseScene {
        dirty: DirtyRegion,
        version: u64,
    }

    impl ScenePaint for DenseScene {
        fn root_id(&self) -> Option<crate::draw::pipeline::NodeId> {
            Some(crate::draw::pipeline::NodeId::new(1))
        }
        fn tree_version(&self) -> u64 {
            self.version
        }
        fn dirty_region(&self) -> DirtyRegion {
            self.dirty.clone()
        }
        fn node_visible(&self, id: crate::draw::pipeline::NodeId) -> bool {
            let slot = id.slot();
            (1..=101).contains(&slot)
        }
        fn node_frame(&self, id: crate::draw::pipeline::NodeId) -> Rect {
            if id.slot() == 1 {
                return Rect::new(0.0, 0.0, 400.0, 400.0);
            }
            let i = (id.slot() - 2) as f32;
            let col = i % 10.0;
            let row = (i / 10.0).floor();
            Rect::new(col * 40.0, row * 40.0, 36.0, 36.0)
        }
        fn node_dirty(&self, id: crate::draw::pipeline::NodeId) -> bool {
            self.dirty.intersects(self.node_frame(id))
        }
        fn node_z_index(&self, _: crate::draw::pipeline::NodeId) -> i32 {
            0
        }
        fn node_children(
            &self,
            id: crate::draw::pipeline::NodeId,
        ) -> &[crate::draw::pipeline::NodeId] {
            static CHILDREN: [crate::draw::pipeline::NodeId; 100] = {
                let mut kids = [crate::draw::pipeline::NodeId::new(0); 100];
                let mut i = 0;
                while i < 100 {
                    kids[i] = crate::draw::pipeline::NodeId::new(i + 2);
                    i += 1;
                }
                kids
            };
            if id.slot() == 1 {
                &CHILDREN
            } else {
                &[]
            }
        }
        fn node_picture_policy(
            &self,
            _: crate::draw::pipeline::NodeId,
        ) -> crate::draw::compositor::PicturePolicy {
            crate::draw::compositor::PicturePolicy::Never
        }
        fn children_clip(&self, _: crate::draw::pipeline::NodeId, _: Rect) -> Option<Rect> {
            None
        }
        fn dirty_rect(&self, _: crate::draw::pipeline::NodeId, frame: Rect) -> Rect {
            frame
        }
        fn scroll_offset(&self, _: crate::draw::pipeline::NodeId) -> Option<(f32, f32)> {
            None
        }
        fn focused_node(&self) -> Option<crate::draw::pipeline::NodeId> {
            None
        }
        fn node_focusable(&self, _: crate::draw::pipeline::NodeId) -> bool {
            false
        }
        fn hit_test(&self, _: Point) -> Option<crate::draw::pipeline::NodeId> {
            None
        }
        fn parent(
            &self,
            id: crate::draw::pipeline::NodeId,
        ) -> Option<crate::draw::pipeline::NodeId> {
            if id.slot() == 1 {
                None
            } else {
                Some(crate::draw::pipeline::NodeId::new(1))
            }
        }
        fn paint(
            &self,
            id: crate::draw::pipeline::NodeId,
            frame: Rect,
            ctx: &mut PaintContext<'_>,
        ) {
            if id.slot() == 1 {
                return;
            }
            let s = id.slot() as u32;
            ctx.fill_rect(
                frame,
                Color::from_rgba(
                    ((s * 17) % 255) as u8,
                    ((s * 29) % 255) as u8,
                    ((s * 41) % 255) as u8,
                    255,
                ),
                None,
            );
        }
    }

    let tokens = MockTokens;
    let fs = FontService::new();
    let img = ImageService::new();

    let mut engine = SoftwareEngine::new();
    engine.initialize(400, 400).expect("init");
    let mut renderer = FrameRenderer::new();

    let full = DirtyRegion::full();
    let scene_full = DenseScene {
        dirty: full.clone(),
        version: 1,
    };
    let _ = renderer.render_frame(
        &mut engine,
        &scene_full,
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &full,
            tree_version: 1,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    let mut full_ms = 0u128;
    for _ in 0..5 {
        let t0 = Instant::now();
        let out = renderer.render_frame(
            &mut engine,
            &scene_full,
            FrameRenderInput {
                rendered_first: true,
                dirty_region: &full,
                tree_version: 1,
                scroll_move: None,
                theme: ThemeSnapshot::new(&tokens),
                font: FontHandle::default(),
                font_service: &fs,
                image_service: &img,
                debug_mode: false,
                hover_pos: None,
                metrics: None,
            },
        );
        full_ms += t0.elapsed().as_millis();
        assert!(matches!(
            out.outcome,
            RenderOutcome::PresentPending(DamageRegion { full: true, .. })
                | RenderOutcome::Present(DamageRegion { full: true, .. })
        ));
    }

    let mut dirty = DirtyRegion::empty();
    dirty.add_rect(Rect::new(0.0, 0.0, 40.0, 40.0));
    let scene_dirty = DenseScene {
        dirty: dirty.clone(),
        version: 1,
    };
    let mut dirty_ms = 0u128;
    for _ in 0..5 {
        let t0 = Instant::now();
        let out = renderer.render_frame(
            &mut engine,
            &scene_dirty,
            FrameRenderInput {
                rendered_first: true,
                dirty_region: &dirty,
                tree_version: 1,
                scroll_move: None,
                theme: ThemeSnapshot::new(&tokens),
                font: FontHandle::default(),
                font_service: &fs,
                image_service: &img,
                debug_mode: false,
                hover_pos: None,
                metrics: None,
            },
        );
        dirty_ms += t0.elapsed().as_millis();
        match &out.outcome {
            RenderOutcome::PresentPending(d) | RenderOutcome::Present(d) => {
                assert!(
                    !d.full,
                    "dirty frame must not expand to full present damage"
                );
            }
            other => panic!("expected present, got {other:?}"),
        }
    }

    eprintln!(
        "L4 evidence: full_paint_5x={}ms dirty_paint_5x={}ms (dirty should be <= full)",
        full_ms, dirty_ms
    );
    assert!(
        dirty_ms <= full_ms,
        "dirty paint should not exceed full paint cost: dirty={dirty_ms}ms full={full_ms}ms"
    );
}

/// 悬停窄标脏时父背景不得盖住未重绘的兄弟节点（移动鼠标后整页空白的根因）。
///
/// FrameEncoder 执行绕过 begin_frame 的 surface clip；若 recording 也不裁到
/// damage AABB，父 FillRect 会写满自身 frame，兄弟像素被擦掉且不再绘制。
#[test]
fn dirty_frame_parent_bg_must_not_wipe_undamaged_sibling() {
    use crate::draw::painting::PaintContext;
    use crate::draw::SoftwareEngine;
    use std::cell::Cell;

    struct SiblingScene {
        dirty: DirtyRegion,
        /// 仅左子标脏（模拟 hover）；右子保持干净。
        left_dirty: Cell<bool>,
    }

    impl ScenePaint for SiblingScene {
        fn root_id(&self) -> Option<crate::draw::pipeline::NodeId> {
            Some(crate::draw::pipeline::NodeId::new(1))
        }
        fn tree_version(&self) -> u64 {
            1
        }
        fn dirty_region(&self) -> DirtyRegion {
            self.dirty.clone()
        }
        fn node_visible(&self, id: crate::draw::pipeline::NodeId) -> bool {
            (1..=3).contains(&id.slot())
        }
        fn node_frame(&self, id: crate::draw::pipeline::NodeId) -> Rect {
            match id.slot() {
                1 => Rect::new(0.0, 0.0, 100.0, 40.0),
                2 => Rect::new(0.0, 0.0, 40.0, 40.0),
                3 => Rect::new(60.0, 0.0, 40.0, 40.0),
                _ => Rect::zero(),
            }
        }
        fn node_dirty(&self, id: crate::draw::pipeline::NodeId) -> bool {
            match id.slot() {
                2 => self.left_dirty.get(),
                _ => false,
            }
        }
        fn node_z_index(&self, _: crate::draw::pipeline::NodeId) -> i32 {
            0
        }
        fn node_children(
            &self,
            id: crate::draw::pipeline::NodeId,
        ) -> &[crate::draw::pipeline::NodeId] {
            static KIDS: [crate::draw::pipeline::NodeId; 2] = [
                crate::draw::pipeline::NodeId::new(2),
                crate::draw::pipeline::NodeId::new(3),
            ];
            if id.slot() == 1 {
                &KIDS
            } else {
                &[]
            }
        }
        fn children_clip(&self, _: crate::draw::pipeline::NodeId, _: Rect) -> Option<Rect> {
            None
        }
        fn dirty_rect(&self, _: crate::draw::pipeline::NodeId, frame: Rect) -> Rect {
            frame
        }
        fn scroll_offset(&self, _: crate::draw::pipeline::NodeId) -> Option<(f32, f32)> {
            None
        }
        fn focused_node(&self) -> Option<crate::draw::pipeline::NodeId> {
            None
        }
        fn node_focusable(&self, _: crate::draw::pipeline::NodeId) -> bool {
            false
        }
        fn hit_test(&self, _: Point) -> Option<crate::draw::pipeline::NodeId> {
            None
        }
        fn parent(
            &self,
            id: crate::draw::pipeline::NodeId,
        ) -> Option<crate::draw::pipeline::NodeId> {
            if id.slot() == 1 {
                None
            } else {
                Some(crate::draw::pipeline::NodeId::new(1))
            }
        }
        fn paint(
            &self,
            id: crate::draw::pipeline::NodeId,
            frame: Rect,
            ctx: &mut PaintContext<'_>,
        ) {
            match id.slot() {
                // 父背景：灰。若 dirty 帧未裁剪，会盖住右子。
                1 => ctx.fill_rect(frame, Color::from_rgba(200, 200, 200, 255), None),
                2 => ctx.fill_rect(frame, Color::from_rgba(255, 0, 0, 255), None),
                3 => ctx.fill_rect(frame, Color::from_rgba(0, 0, 255, 255), None),
                _ => {}
            }
        }
    }

    let tokens = MockTokens;
    let fs = FontService::new();
    let img = ImageService::new();
    let mut engine = SoftwareEngine::new();
    engine.initialize(100, 40).expect("init");
    let mut renderer = FrameRenderer::new();

    let full = DirtyRegion::full();
    let scene = SiblingScene {
        dirty: full.clone(),
        left_dirty: Cell::new(true),
    };
    let out = renderer.render_frame(
        &mut engine,
        &scene,
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &full,
            tree_version: 1,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );
    assert!(
        matches!(
            out.outcome,
            RenderOutcome::PresentPending(_) | RenderOutcome::Present(_)
        ),
        "first frame present: {:?}",
        out.outcome
    );

    let blue = Color::from_rgba(0, 0, 255, 255).premultiplied();
    let right_px = {
        let pixels = engine.canvas_2d().pixels_mut();
        pixels[20 * 100 + 80] // center of right sibling
    };
    assert_eq!(right_px, blue, "baseline: right sibling must be blue");

    // 仅左子脏：父仍会因 dirty 相交被绘制；右子不得被父背景擦成灰。
    let mut dirty = DirtyRegion::empty();
    dirty.add_rect(Rect::new(0.0, 0.0, 40.0, 40.0));
    scene.left_dirty.set(true);
    let out = renderer.render_frame(
        &mut engine,
        &scene,
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &dirty,
            tree_version: 1,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );
    assert!(
        matches!(
            out.outcome,
            RenderOutcome::PresentPending(_) | RenderOutcome::Present(_)
        ),
        "dirty frame present: {:?}",
        out.outcome
    );

    let right_px = {
        let pixels = engine.canvas_2d().pixels_mut();
        pixels[20 * 100 + 80]
    };
    assert_eq!(
        right_px, blue,
        "after hover dirty paint, undamaged sibling must stay blue (not parent gray)"
    );
}

/// 滚动时整窗视口必须并入清/绘区，由 paint 重新生成偏移后的内容。
///
/// 只清重绘 exposed strip 时，视口内已偏移的内容保持上一帧的旧像素，
/// 与新绘的 strip 叠加 → 滚动渲染错乱（标题/按钮/分割线互相重叠）。
#[test]
fn scroll_move_shifts_viewport_content_and_repaints_exposed_strip() {
    use crate::draw::painting::PaintContext;
    use std::cell::Cell;

    struct ScrollScene {
        scroll_y: Cell<f32>,
        /// 全帧标脏（首帧）；后续帧由 dirty_region 驱动。
        root_dirty: Cell<bool>,
    }

    impl ScrollScene {
        fn band_color(y: f32) -> Color {
            // 50px 一段，依次红/绿/蓝/黄…
            match (y as i32) / 50 {
                0 => Color::from_rgba(200, 0, 0, 255),
                1 => Color::from_rgba(0, 200, 0, 255),
                2 => Color::from_rgba(0, 0, 200, 255),
                3 => Color::from_rgba(200, 200, 0, 255),
                _ => Color::from_rgba(40, 40, 40, 255),
            }
        }
    }

    impl ScenePaint for ScrollScene {
        fn root_id(&self) -> Option<NodeId> {
            Some(NodeId::new(1))
        }
        fn tree_version(&self) -> u64 {
            1
        }
        fn dirty_region(&self) -> DirtyRegion {
            DirtyRegion::full()
        }
        fn node_visible(&self, id: NodeId) -> bool {
            id.slot() == 1 || id.slot() == 2
        }
        fn node_frame(&self, id: NodeId) -> Rect {
            match id.slot() {
                1 => Rect::new(0.0, 0.0, 100.0, 100.0),
                2 => Rect::new(0.0, 0.0, 100.0, 300.0),
                _ => Rect::zero(),
            }
        }
        fn node_dirty(&self, id: NodeId) -> bool {
            id.slot() == 1 && self.root_dirty.get()
        }
        fn node_z_index(&self, _: NodeId) -> i32 {
            0
        }
        fn node_children(&self, id: NodeId) -> &[NodeId] {
            static KIDS: [NodeId; 1] = [NodeId::new(2)];
            static EMPTY: [NodeId; 0] = [];
            if id.slot() == 1 {
                &KIDS
            } else {
                &EMPTY
            }
        }
        fn children_clip(&self, id: NodeId, frame: Rect) -> Option<Rect> {
            if id.slot() == 1 {
                Some(frame)
            } else {
                None
            }
        }
        fn dirty_rect(&self, _: NodeId, frame: Rect) -> Rect {
            frame
        }
        fn scroll_offset(&self, id: NodeId) -> Option<(f32, f32)> {
            if id.slot() == 1 {
                Some((0.0, self.scroll_y.get()))
            } else {
                None
            }
        }
        fn focused_node(&self) -> Option<NodeId> {
            None
        }
        fn node_focusable(&self, _: NodeId) -> bool {
            false
        }
        fn hit_test(&self, _: Point) -> Option<NodeId> {
            None
        }
        fn parent(&self, id: NodeId) -> Option<NodeId> {
            if id.slot() == 2 {
                Some(NodeId::new(1))
            } else {
                None
            }
        }
        fn paint(&self, id: NodeId, frame: Rect, ctx: &mut PaintContext<'_>) {
            if id.slot() == 2 {
                // 按 50px 分段填色；帧处于 content 坐标 (0,0,100,300)
                let mut y = frame.y;
                while y < frame.y + frame.h {
                    let band_h: f32 = 50.0;
                    let top = y;
                    let h = band_h.min(frame.y + frame.h - top);
                    ctx.fill_rect(
                        Rect::new(frame.x, top, frame.w, h),
                        Self::band_color(top),
                        None,
                    );
                    y += band_h;
                }
            }
        }
    }

    let tokens = MockTokens;
    let _theme = ThemeSnapshot::new(&tokens);
    let fs = FontService::new();
    let img = ImageService::new();
    let mut engine = SoftwareEngine::new();
    engine.initialize(100, 100).expect("init");
    let mut renderer = FrameRenderer::new();

    let scene = ScrollScene {
        scroll_y: Cell::new(0.0),
        root_dirty: Cell::new(true),
    };

    // 首帧：scroll_y=0，视口显示 红(0..50) + 绿(50..100)
    let full = DirtyRegion::full();
    let out = renderer.render_frame(
        &mut engine,
        &scene,
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &full,
            tree_version: 1,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );
    assert!(matches!(
        out.outcome,
        RenderOutcome::PresentPending(_) | RenderOutcome::Present(_)
    ));
    let red = Color::from_rgba(200, 0, 0, 255).premultiplied();
    let green = Color::from_rgba(0, 200, 0, 255).premultiplied();
    let blue = Color::from_rgba(0, 0, 200, 255).premultiplied();
    let px = |engine: &SoftwareEngine, x: usize, y: usize| {
        // canvas_2d() 借用不便；直接通过 session 读 CPU 后端像素
        engine
            .session()
            .cpu_backend()
            .expect("CPU backend")
            .pixels()[y * 100 + x]
    };
    assert_eq!(px(&engine, 50, 25), red, "baseline: top band is red");
    assert_eq!(px(&engine, 50, 75), green, "baseline: second band is green");

    // 滚动 50px：scroll_y=50。exposed strip = 底部 50px (0,50,100,50)
    scene.scroll_y.set(50.0);
    scene.root_dirty.set(false);
    let mut dirty = DirtyRegion::empty();
    dirty.add_rect(Rect::new(0.0, 50.0, 100.0, 50.0));
    let out = renderer.render_frame(
        &mut engine,
        &scene,
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &dirty,
            tree_version: 1,
            scroll_move: Some(vec![(Rect::new(0.0, 0.0, 100.0, 100.0), 0.0, 50.0)]),
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );
    assert!(matches!(
        out.outcome,
        RenderOutcome::PresentPending(_) | RenderOutcome::Present(_)
    ));

    // 视口整窗重绘后：顶部 50px 应为绿（content y=50..100 重绘到视口 0..50）
    assert_eq!(
        px(&engine, 50, 25),
        green,
        "after scroll, top band must be green (repainted content y=50..100), not stale red"
    );
    // exposed strip 重绘：底部 50px 应为蓝（content y=100..150）
    assert_eq!(
        px(&engine, 50, 75),
        blue,
        "exposed strip must be repainted with blue (content y=100..150)"
    );
}

/// 滚动帧的 begin_frame 策略必须把滚动视口并入清/绘区。
///
/// dirty_region 只覆盖 exposed strip 时，begin_frame 只清这一条；
/// 视口内已偏移的内容保留上一帧旧像素 → 滚动错乱。并入整窗后
/// begin_frame 清空并重绘整个视口，所有偏移内容由 paint 重新生成。
#[test]
fn scroll_move_uses_copy_strategy_with_exposed_strip_as_paint_region() {
    let mut renderer = FrameRenderer::new();
    let mut engine = RecordingEngine::new();
    let _ = engine.initialize(128, 128);
    let tokens = MockTokens;
    let theme = ThemeSnapshot::new(&tokens);
    let fs = FontService::new();
    let img = ImageService::new();
    // dirty 只覆盖 exposed strip（底部 40px），与视口 (0,0,100,100) 不重合顶部
    let mut dirty = DirtyRegion::empty();
    dirty.add_rect(Rect::new(0.0, 60.0, 100.0, 40.0));

    let _ = renderer.render_frame(
        &mut engine,
        &EmptyScene::new(),
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &dirty,
            tree_version: 0,
            scroll_move: Some(vec![(Rect::new(0.0, 0.0, 100.0, 100.0), 0.0, 40.0)]),
            theme,
            font: FontHandle::default(),
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    let strategy = engine
        .last_begin_strategy
        .as_ref()
        .expect("begin_frame must be called");
    match strategy {
        UpdateStrategy::ScrollCopies {
            dirty_rects,
            copies,
        } => {
            assert_eq!(dirty_rects, &[Rect::new(0.0, 60.0, 100.0, 40.0)]);
            assert_eq!(
                copies,
                &[crate::draw::ScrollCopy::new(
                    Rect::new(0.0, 0.0, 100.0, 100.0),
                    0.0,
                    40.0,
                )]
            );
        }
        UpdateStrategy::DirtyRects(_) | UpdateStrategy::FullRedraw => {
            panic!("eligible scroll must use ScrollCopies")
        }
    }
}

/// 滚动 copy 属于原子 begin_frame 设置，主 FrameEncoder 只记录其后的条带重绘，
/// 避免 copy 与 clear 被拆到两个可独立失败的提交边界。
#[test]
fn scroll_move_keeps_copy_outside_the_main_paint_encoder() {
    let mut renderer = FrameRenderer::new();
    let mut engine = RecordingEngine::new();
    let _ = engine.initialize(128, 128);
    let tokens = MockTokens;
    let theme = ThemeSnapshot::new(&tokens);
    let fs = FontService::new();
    let img = ImageService::new();
    let mut dirty = DirtyRegion::empty();
    dirty.add_rect(Rect::new(0.0, 60.0, 100.0, 40.0));

    let _ = renderer.render_frame(
        &mut engine,
        &EmptyScene::new(),
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &dirty,
            tree_version: 0,
            scroll_move: Some(vec![(Rect::new(0.0, 0.0, 100.0, 100.0), 0.0, 40.0)]),
            theme,
            font: FontHandle::default(),
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    let commands = engine
        .encoded_frames
        .last()
        .expect("scroll frame must produce an encoded frame");
    let has_scroll_copy = commands.iter().any(|cmd| {
        matches!(
            cmd,
            crate::draw::pipeline::FrameCommand::Native {
                operation: crate::draw::pipeline::FrameRasterOp::ScrollCopy { .. }
            }
        )
    });
    assert!(
        !has_scroll_copy,
        "scroll copy must stay in the atomic begin_frame strategy"
    );
}

#[test]
fn scroll_move_without_memmove_capability_repaints_the_viewport() {
    let mut renderer = FrameRenderer::new();
    let mut engine = RecordingEngine::new().without_scroll_memmove();
    let _ = engine.initialize(128, 128);
    let tokens = MockTokens;
    let fs = FontService::new();
    let img = ImageService::new();
    let mut dirty = DirtyRegion::empty();
    dirty.add_rect(Rect::new(0.0, 60.0, 100.0, 40.0));

    let _ = renderer.render_frame(
        &mut engine,
        &EmptyScene::new(),
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &dirty,
            tree_version: 0,
            scroll_move: Some(vec![(Rect::new(0.0, 0.0, 100.0, 100.0), 0.0, 40.0)]),
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    match engine.last_begin_strategy.as_ref() {
        Some(UpdateStrategy::DirtyRects(rects)) => {
            let bounds = rects
                .iter()
                .copied()
                .reduce(|bounds, rect| bounds.union(&rect))
                .expect("fallback dirty region");
            assert_eq!(bounds, Rect::new(0.0, 0.0, 100.0, 100.0));
        }
        _ => panic!("unsupported scroll copy must fall back to viewport DirtyRects"),
    }
}

const BACKDROP_ROOT: crate::draw::pipeline::NodeId = crate::draw::pipeline::NodeId::new(1);
const BACKDROP_OVERLAY: crate::draw::pipeline::NodeId = crate::draw::pipeline::NodeId::new(2);

struct OverlayBackdropScene {
    children: [crate::draw::pipeline::NodeId; 1],
    version: Cell<u64>,
    overlay_visible: Cell<bool>,
    root_dirty: Cell<bool>,
    overlay_dirty: Cell<bool>,
    root_paints: Cell<usize>,
    overlay_paints: Cell<usize>,
    root_color: Cell<Color>,
    overlay_alpha: Cell<u8>,
}

impl OverlayBackdropScene {
    fn new() -> Self {
        Self {
            children: [BACKDROP_OVERLAY],
            version: Cell::new(1),
            overlay_visible: Cell::new(false),
            root_dirty: Cell::new(true),
            overlay_dirty: Cell::new(false),
            root_paints: Cell::new(0),
            overlay_paints: Cell::new(0),
            root_color: Cell::new(Color::from_rgb(20, 80, 180)),
            overlay_alpha: Cell::new(128),
        }
    }

    fn show_overlay(&self) {
        self.version.set(self.version.get() + 1);
        self.overlay_visible.set(true);
        self.overlay_dirty.set(true);
    }
}

impl ScenePaint for OverlayBackdropScene {
    fn root_id(&self) -> Option<crate::draw::pipeline::NodeId> {
        Some(BACKDROP_ROOT)
    }

    fn tree_version(&self) -> u64 {
        self.version.get()
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::full()
    }

    fn node_visible(&self, id: crate::draw::pipeline::NodeId) -> bool {
        id == BACKDROP_ROOT || (id == BACKDROP_OVERLAY && self.overlay_visible.get())
    }

    fn node_frame(&self, _: crate::draw::pipeline::NodeId) -> Rect {
        Rect::new(0.0, 0.0, 4.0, 4.0)
    }

    fn node_dirty(&self, id: crate::draw::pipeline::NodeId) -> bool {
        match id {
            BACKDROP_ROOT => self.root_dirty.get(),
            BACKDROP_OVERLAY => self.overlay_dirty.get(),
            _ => false,
        }
    }

    fn node_z_index(&self, id: crate::draw::pipeline::NodeId) -> i32 {
        if id == BACKDROP_OVERLAY {
            1_000
        } else {
            0
        }
    }

    fn node_children(&self, id: crate::draw::pipeline::NodeId) -> &[crate::draw::pipeline::NodeId] {
        if id == BACKDROP_ROOT {
            &self.children
        } else {
            &[]
        }
    }

    fn node_is_overlay(&self, id: crate::draw::pipeline::NodeId) -> bool {
        id == BACKDROP_OVERLAY && self.overlay_visible.get()
    }

    fn children_clip(&self, _: crate::draw::pipeline::NodeId, _: Rect) -> Option<Rect> {
        None
    }

    fn dirty_rect(&self, _: crate::draw::pipeline::NodeId, frame: Rect) -> Rect {
        frame
    }

    fn scroll_offset(&self, _: crate::draw::pipeline::NodeId) -> Option<(f32, f32)> {
        None
    }

    fn focused_node(&self) -> Option<crate::draw::pipeline::NodeId> {
        None
    }

    fn node_focusable(&self, _: crate::draw::pipeline::NodeId) -> bool {
        false
    }

    fn hit_test(&self, _: Point) -> Option<crate::draw::pipeline::NodeId> {
        None
    }

    fn parent(&self, id: crate::draw::pipeline::NodeId) -> Option<crate::draw::pipeline::NodeId> {
        (id == BACKDROP_OVERLAY).then_some(BACKDROP_ROOT)
    }

    fn paint(&self, id: crate::draw::pipeline::NodeId, frame: Rect, ctx: &mut PaintContext<'_>) {
        if id == BACKDROP_ROOT {
            self.root_paints.set(self.root_paints.get() + 1);
            ctx.fill_rect(frame, self.root_color.get(), None);
        } else if id == BACKDROP_OVERLAY {
            self.overlay_paints.set(self.overlay_paints.get() + 1);
            ctx.fill_rect(
                frame,
                Color::from_rgba(0, 0, 0, self.overlay_alpha.get()),
                None,
            );
        }
    }
}

fn render_overlay_backdrop_test_frame(
    renderer: &mut FrameRenderer,
    engine: &mut dyn GraphicsEngine,
    scene: &OverlayBackdropScene,
    rendered_first: bool,
    tokens: &MockTokens,
    fonts: &FontService,
    images: &ImageService,
) -> FrameRenderOutput {
    let dirty = DirtyRegion::full();
    renderer.render_frame(
        engine,
        scene,
        FrameRenderInput {
            rendered_first,
            dirty_region: &dirty,
            tree_version: scene.version.get(),
            scroll_move: None,
            theme: ThemeSnapshot::new(tokens),
            font: FontHandle::default(),
            font_service: fonts,
            image_service: images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    )
}

fn expected_masked_pixel(background: Color, alpha: u8) -> u32 {
    use crate::draw::pipeline::{FrameEncoder, FrameRasterOp, FrameRect};

    let mut encoder = FrameEncoder::new(1, 1).expect("reference encoder");
    encoder.clear(background);
    encoder.native(FrameRasterOp::FillRect {
        rect: FrameRect::new(0, 0, 1, 1),
        color: Color::from_rgba(0, 0, 0, alpha),
    });
    encoder
        .render_reference()
        .pixel(0, 0)
        .expect("reference pixel")
}

#[test]
fn full_overlay_frames_restore_clean_backdrop_without_repainting_normal_tree() {
    let tokens = MockTokens;
    let fonts = FontService::new();
    let images = ImageService::new();
    let scene = OverlayBackdropScene::new();
    let mut renderer = FrameRenderer::new();
    let mut engine = EncodedSoftwareEngine::new();
    engine.initialize(4, 4).expect("software init");

    let first = render_overlay_backdrop_test_frame(
        &mut renderer,
        &mut engine,
        &scene,
        false,
        &tokens,
        &fonts,
        &images,
    );
    assert!(matches!(first.outcome, RenderOutcome::PresentPending(_)));
    assert_eq!(scene.root_paints.get(), 1);

    scene.root_dirty.set(false);
    scene.show_overlay();
    let opened = render_overlay_backdrop_test_frame(
        &mut renderer,
        &mut engine,
        &scene,
        true,
        &tokens,
        &fonts,
        &images,
    );
    assert!(matches!(opened.outcome, RenderOutcome::PresentPending(_)));
    assert_eq!(
        scene.root_paints.get(),
        1,
        "opening must reuse the backdrop"
    );
    assert_eq!(scene.overlay_paints.get(), 1);
    assert!(engine.encoded_frames.last().is_some_and(|commands| {
        commands.iter().any(|command| {
            matches!(
                command,
                crate::draw::pipeline::FrameCommand::PictureBlit { .. }
            )
        })
    }));
    assert_eq!(
        engine
            .inner
            .session()
            .cpu_backend()
            .expect("CPU backend")
            .pixels()[0],
        expected_masked_pixel(scene.root_color.get(), 128)
    );

    scene.overlay_alpha.set(64);
    let animated = render_overlay_backdrop_test_frame(
        &mut renderer,
        &mut engine,
        &scene,
        true,
        &tokens,
        &fonts,
        &images,
    );
    assert!(matches!(animated.outcome, RenderOutcome::PresentPending(_)));
    assert_eq!(scene.root_paints.get(), 1);
    assert_eq!(scene.overlay_paints.get(), 2);
    assert_eq!(
        engine
            .inner
            .session()
            .cpu_backend()
            .expect("CPU backend")
            .pixels()[0],
        expected_masked_pixel(scene.root_color.get(), 64),
        "each animation frame must compose from the original clean backdrop"
    );
}

#[test]
fn dirty_normal_tree_blocks_overlay_backdrop_until_overlays_leave() {
    let tokens = MockTokens;
    let fonts = FontService::new();
    let images = ImageService::new();
    let scene = OverlayBackdropScene::new();
    let mut renderer = FrameRenderer::new();
    let mut engine = EncodedSoftwareEngine::new();
    engine.initialize(4, 4).expect("software init");

    let _ = render_overlay_backdrop_test_frame(
        &mut renderer,
        &mut engine,
        &scene,
        false,
        &tokens,
        &fonts,
        &images,
    );
    scene.show_overlay();
    scene.root_color.set(Color::from_rgb(20, 180, 80));
    let changed = render_overlay_backdrop_test_frame(
        &mut renderer,
        &mut engine,
        &scene,
        true,
        &tokens,
        &fonts,
        &images,
    );
    assert!(matches!(changed.outcome, RenderOutcome::PresentPending(_)));
    assert_eq!(scene.root_paints.get(), 2, "dirty base must be repainted");
    assert!(engine.encoded_frames.last().is_some_and(|commands| {
        !commands.iter().any(|command| {
            matches!(
                command,
                crate::draw::pipeline::FrameCommand::PictureBlit { .. }
            )
        })
    }));
    assert_eq!(
        engine
            .inner
            .session()
            .cpu_backend()
            .expect("CPU backend")
            .pixels()[0],
        expected_masked_pixel(scene.root_color.get(), 128)
    );

    scene.root_dirty.set(false);
    scene.overlay_alpha.set(64);
    let next = render_overlay_backdrop_test_frame(
        &mut renderer,
        &mut engine,
        &scene,
        true,
        &tokens,
        &fonts,
        &images,
    );
    assert!(matches!(next.outcome, RenderOutcome::PresentPending(_)));
    assert!(
        engine.encoded_frames.last().is_some_and(|commands| {
            !commands.iter().any(|command| {
                matches!(
                    command,
                    crate::draw::pipeline::FrameCommand::PictureBlit { .. }
                )
            })
        }),
        "a surface that already contains overlay pixels cannot become a later backdrop"
    );
    assert_eq!(
        engine
            .inner
            .session()
            .cpu_backend()
            .expect("CPU backend")
            .pixels()[0],
        expected_masked_pixel(scene.root_color.get(), 64)
    );
}

#[test]
fn engine_without_readable_frame_pixels_keeps_full_overlay_repaint() {
    let tokens = MockTokens;
    let fonts = FontService::new();
    let images = ImageService::new();
    let scene = OverlayBackdropScene::new();
    let mut renderer = FrameRenderer::new();
    let mut engine = RecordingEngine::new();

    let _ = render_overlay_backdrop_test_frame(
        &mut renderer,
        &mut engine,
        &scene,
        false,
        &tokens,
        &fonts,
        &images,
    );
    scene.root_dirty.set(false);
    scene.show_overlay();
    let _ = render_overlay_backdrop_test_frame(
        &mut renderer,
        &mut engine,
        &scene,
        true,
        &tokens,
        &fonts,
        &images,
    );

    assert_eq!(
        scene.root_paints.get(),
        2,
        "unsupported engines must preserve the established full-redraw path"
    );
    assert_eq!(scene.overlay_paints.get(), 1);
}
