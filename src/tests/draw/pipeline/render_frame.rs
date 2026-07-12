use super::*;
use crate::core::Error;
use crate::draw::SoftwareEngine;
use crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
use crate::draw::null_engine::NullEngine;
use crate::draw::painting::PaintContext;
use crate::draw::traits::{Canvas2D, GraphicsCapabilities, GraphicsEngine, UpdateStrategy};
use std::cell::Cell;

struct RecordingEngine {
    canvas: NoopCanvas2D,
    events: Vec<&'static str>,
    partial_redraw: bool,
    begin_failure: Option<crate::draw::engine::GraphicsFailure>,
    begin_outcome: Option<RenderOutcome>,
    encoded_frame_failure: Option<Error>,
    end_outcome: RenderOutcome,
    presentation_mode: crate::draw::traits::PresentationMode,
}

impl RecordingEngine {
    fn new() -> Self {
        Self {
            canvas: NoopCanvas2D,
            events: Vec::new(),
            partial_redraw: true,
            begin_failure: None,
            begin_outcome: None,
            encoded_frame_failure: None,
            end_outcome: RenderOutcome::Present(DamageRegion::full()),
            presentation_mode: crate::draw::traits::PresentationMode::EngineManaged,
        }
    }

    fn without_partial_redraw(mut self) -> Self {
        self.partial_redraw = false;
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

    fn shutdown(&mut self) {}

    fn resize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
        Ok(())
    }

    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        self.events.push("begin");
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
        }
    }

    fn end_frame(&mut self, present_damage: &DamageRegion) -> RenderOutcome {
        self.events.push("end");
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
        }
    }

    fn try_execute_encoded_frame(
        &mut self,
        _encoder: &crate::draw::pipeline::FrameEncoder,
    ) -> Result<crate::draw::pipeline::EncodedFrameExecution, Error> {
        self.events.push("encoded_frame");
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

struct EncodedSoftwareEngine {
    inner: SoftwareEngine,
    encoded_picture_executions: usize,
    encoded_frame_executions: usize,
}

impl EncodedSoftwareEngine {
    fn new() -> Self {
        Self {
            inner: SoftwareEngine::new(),
            encoded_picture_executions: 0,
            encoded_frame_executions: 0,
        }
    }
}

impl GraphicsEngine for EncodedSoftwareEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.initialize(width, height)
    }

    fn shutdown(&mut self) {
        self.inner.shutdown();
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
    picture_policy: crate::draw::compositor::PicturePolicy,
}

impl EligiblePictureScene {
    fn new() -> Self {
        Self {
            root_dirty: Cell::new(true),
            child_dirty: Cell::new(false),
            picture_policy: crate::draw::compositor::PicturePolicy::Eligible,
        }
    }

    fn direct() -> Self {
        Self {
            picture_policy: crate::draw::compositor::PicturePolicy::Never,
            ..Self::new()
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
            scroll_move: Some((Rect::new(30.0, 40.0, 50.0, 60.0), 0.0, -12.0)),
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
fn engine_without_partial_redraw_expands_dirty_region_to_full_damage() {
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

    assert_eq!(out.outcome, RenderOutcome::Present(DamageRegion::full()));
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
            scroll_move: Some((Rect::new(10.0, 12.0, 30.0, 40.0), 0.0, -8.0)),
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
