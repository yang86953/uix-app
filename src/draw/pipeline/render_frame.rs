//! 帧渲染调度 — 从 UI event_loop 迁入的渲染段（Phase 3）。

use crate::core::{Error, Point, Rect};

use crate::core::DirtyRegion;
use crate::draw::backend::DamageRegion;
use crate::draw::compositor::{LayerTree, RenderObjectTree, ScenePaint};
use crate::draw::debug::DebugRenderService;
use crate::draw::font::font_service::FontService;
use crate::draw::font::text::TextRenderService;
use crate::draw::image::ImageService;
use crate::draw::painting::ThemeSnapshot;
use crate::draw::pipeline::{
    EncodedFrameExecution, InvalidationSource, RenderMetrics, frame_recording::FrameRecordingEngine,
};
use crate::draw::traits::{GraphicsEngine, UpdateStrategy};
use crate::draw::{Color, FontHandle, RenderOutcome};

/// 单帧渲染输入。
pub struct FrameRenderInput<'a> {
    pub rendered_first: bool,
    pub dirty_region: &'a DirtyRegion,
    pub tree_version: u64,
    pub scroll_move: Option<(Rect, f32, f32)>,
    pub theme: ThemeSnapshot<'a>,
    pub font: FontHandle,
    pub font_service: &'a FontService,
    pub image_service: &'a ImageService,
    pub debug_mode: bool,
    pub hover_pos: Option<Point>,
    pub metrics: Option<&'a RenderMetrics>,
}

/// 单帧渲染输出。
pub struct FrameRenderOutput {
    pub outcome: RenderOutcome,
    pub inv_source: InvalidationSource,
    pub tree_version: u64,
}

/// 帧渲染器 — 持有 LayerTree 与合成状态。
pub struct FrameRenderer {
    layer_tree: LayerTree,
    render_object_tree: RenderObjectTree,
    last_tree_version: u64,
    /// Private API-neutral producer for every scene path before the real
    /// backend consumes the one ordered main `FrameEncoder` (#181). It never
    /// owns a presentation surface and therefore cannot become a second
    /// submission boundary.
    recording_engine: FrameRecordingEngine,
    recording_extent: Option<(i32, i32)>,
}

impl FrameRenderer {
    pub fn new() -> Self {
        Self {
            layer_tree: LayerTree::new(),
            render_object_tree: RenderObjectTree::new(),
            last_tree_version: 0,
            recording_engine: FrameRecordingEngine::new(),
            recording_extent: None,
        }
    }

    pub fn render_object_tree(&self) -> &RenderObjectTree {
        &self.render_object_tree
    }

    pub fn layer_tree(&self) -> &LayerTree {
        &self.layer_tree
    }

    pub fn layer_tree_mut(&mut self) -> &mut LayerTree {
        &mut self.layer_tree
    }

    /// 执行单 Pass 渲染（Content + AfterChildren）；返回 Present damage 与 invalidation 来源。
    pub fn render_frame<S: ScenePaint>(
        &mut self,
        engine: &mut dyn GraphicsEngine,
        scene: &S,
        input: FrameRenderInput<'_>,
    ) -> FrameRenderOutput {
        let cur_version = scene.tree_version();
        if input.rendered_first && input.dirty_region.is_empty() && input.scroll_move.is_none() {
            return FrameRenderOutput {
                outcome: RenderOutcome::Idle,
                inv_source: InvalidationSource::None,
                tree_version: cur_version,
            };
        }

        let caps = engine.capabilities();
        // FrameRecordingEngine rebuilds a complete ordered command stream and
        // clears/replaces the whole target. Until R6 provides retained,
        // damage-aware recording, neither begin-frame clipping nor a partial
        // final present can truthfully describe that work (#181, #197).
        let region = DirtyRegion::full();
        let damage = DamageRegion::full();
        let strategy = UpdateStrategy::FullRedraw;
        let begin_outcome = engine.begin_frame(strategy.clone());
        match begin_outcome {
            RenderOutcome::FrameReady(_) => {}
            RenderOutcome::Present(_) => {
                return FrameRenderOutput {
                    outcome: RenderOutcome::Failed(
                        crate::draw::engine::GraphicsFailure::from_error(crate::core::Error::new(
                            crate::core::Errc::InvalidState,
                            "begin_frame reported a final presentation",
                        )),
                    ),
                    inv_source: InvalidationSource::None,
                    tree_version: cur_version,
                };
            }
            RenderOutcome::PresentPending(_) => {
                return FrameRenderOutput {
                    outcome: RenderOutcome::Failed(
                        crate::draw::engine::GraphicsFailure::from_error(crate::core::Error::new(
                            crate::core::Errc::InvalidState,
                            "begin_frame reported an external presentation pending result",
                        )),
                    ),
                    inv_source: InvalidationSource::None,
                    tree_version: cur_version,
                };
            }
            RenderOutcome::Idle => {
                return FrameRenderOutput {
                    outcome: RenderOutcome::Idle,
                    inv_source: InvalidationSource::None,
                    tree_version: cur_version,
                };
            }
            RenderOutcome::Failed(error) => {
                // A failed begin has no valid recording target.  Continuing
                // into scene paint/end_frame can clear dirty state or report a
                // later success for a frame that never began.
                return FrameRenderOutput {
                    outcome: RenderOutcome::Failed(error),
                    inv_source: InvalidationSource::None,
                    tree_version: cur_version,
                };
            }
        }

        let (recording_w, recording_h) = Self::reference_extent(engine, scene);
        if let Err(error) = self.ensure_recording_surface(recording_w, recording_h) {
            return FrameRenderOutput {
                outcome: RenderOutcome::Failed(crate::draw::engine::GraphicsFailure::from_error(
                    error,
                )),
                inv_source: InvalidationSource::None,
                tree_version: cur_version,
            };
        }

        if self.last_tree_version != cur_version || !self.layer_tree.is_ready() {
            // The private producer owns CPU Picture targets, so cached and
            // direct paths are both included before the one real main-surface
            // FrameEncoder is executed.
            self.layer_tree.build(scene, true);
            if let Err(error) = self
                .layer_tree
                .sweep_orphaned_offscreens(&mut self.recording_engine)
            {
                return FrameRenderOutput {
                    outcome: RenderOutcome::Failed(
                        crate::draw::engine::GraphicsFailure::from_error(error),
                    ),
                    inv_source: InvalidationSource::None,
                    tree_version: cur_version,
                };
            }
            self.last_tree_version = cur_version;
        }
        self.layer_tree.update_dirty(scene);
        self.render_object_tree.sync(scene);

        // The producer rebuilds the complete ordered command stream. Until R6
        // adds retained/damage-aware recording, a partial stream could omit
        // an unchanged ancestor or overlay before final execution.
        let reference_region = DirtyRegion::full();
        if let Err(error) = self.recording_engine.begin_recording() {
            return FrameRenderOutput {
                outcome: RenderOutcome::Failed(crate::draw::engine::GraphicsFailure::from_error(
                    error,
                )),
                inv_source: InvalidationSource::None,
                tree_version: cur_version,
            };
        }
        // 首帧绕过 DisplayList 缓存，避免空缓存重放导致侧栏等节点漏绘
        let render_objects = if input.rendered_first {
            Some(&mut self.render_object_tree)
        } else {
            None
        };
        if let Err(error) = self.layer_tree.render(
            &mut self.recording_engine,
            scene,
            &reference_region,
            &input.theme,
            input.font,
            input.font_service,
            input.image_service,
            input.debug_mode,
            input.hover_pos,
            render_objects,
        ) {
            // An offscreen bind/flush/blit failure occurred while recording.
            // Do not call end_frame: that could submit a partial frame or turn
            // the failure into a final present. LayerTree leaves the affected
            // Picture dirty, and the caller retains invalidation for recovery.
            return FrameRenderOutput {
                outcome: RenderOutcome::Failed(crate::draw::engine::GraphicsFailure::from_error(
                    error,
                )),
                inv_source: InvalidationSource::None,
                tree_version: cur_version,
            };
        }

        if input.debug_mode {
            draw_debug_telemetry(
                &mut self.recording_engine,
                input.metrics,
                input.font,
                input.font_service,
            );
        }

        let encoded_frame = match self.recording_engine.finish_recording() {
            Ok(encoder) => encoder,
            Err(error) => {
                self.layer_tree.invalidate();
                return FrameRenderOutput {
                    outcome: RenderOutcome::Failed(
                        crate::draw::engine::GraphicsFailure::from_error(error),
                    ),
                    inv_source: InvalidationSource::None,
                    tree_version: cur_version,
                };
            }
        };
        match engine.try_execute_encoded_frame(&encoded_frame) {
            Ok(EncodedFrameExecution::Executed) => {}
            Ok(EncodedFrameExecution::Unsupported) => {
                self.layer_tree.invalidate();
                return FrameRenderOutput {
                    outcome: RenderOutcome::Failed(
                        crate::draw::engine::GraphicsFailure::from_error(Error::new(
                            crate::core::Errc::InvalidState,
                            "graphics engine does not execute the required main FrameEncoder",
                        )),
                    ),
                    inv_source: InvalidationSource::None,
                    tree_version: cur_version,
                };
            }
            Err(error) => {
                // The reference image is private and no final target has been
                // presented. Keep Picture caches dirty and retain caller
                // invalidation for the typed recovery path.
                self.layer_tree.invalidate();
                return FrameRenderOutput {
                    outcome: RenderOutcome::Failed(
                        crate::draw::engine::GraphicsFailure::from_error(error),
                    ),
                    inv_source: InvalidationSource::None,
                    tree_version: cur_version,
                };
            }
        }

        let end_outcome = engine.end_frame(&damage);
        let outcome = match end_outcome {
            RenderOutcome::Present(_) if caps.uses_external_presenter() => {
                RenderOutcome::PresentPending(damage)
            }
            RenderOutcome::Present(_) => RenderOutcome::Present(damage),
            RenderOutcome::PresentPending(_) if caps.uses_external_presenter() => {
                RenderOutcome::PresentPending(damage)
            }
            RenderOutcome::PresentPending(_) => RenderOutcome::Failed(
                crate::draw::engine::GraphicsFailure::from_error(crate::core::Error::new(
                    crate::core::Errc::InvalidState,
                    "engine-managed backend returned an external presentation pending result",
                )),
            ),
            RenderOutcome::FrameReady(_) => RenderOutcome::Failed(
                crate::draw::engine::GraphicsFailure::from_error(crate::core::Error::new(
                    crate::core::Errc::InvalidState,
                    "end_frame returned FrameReady instead of final presentation",
                )),
            ),
            RenderOutcome::Idle => RenderOutcome::Idle,
            RenderOutcome::Failed(error) => RenderOutcome::Failed(error),
        };
        let inv_source = match outcome {
            RenderOutcome::Present(_) | RenderOutcome::PresentPending(_) => {
                classify_invalidation(input.rendered_first, &region)
            }
            RenderOutcome::Idle | RenderOutcome::FrameReady(_) | RenderOutcome::Failed(_) => {
                InvalidationSource::None
            }
        };

        FrameRenderOutput {
            outcome,
            inv_source,
            tree_version: cur_version,
        }
    }

    fn reference_extent<S: ScenePaint>(engine: &mut dyn GraphicsEngine, scene: &S) -> (i32, i32) {
        let (canvas_w, canvas_h) = {
            let canvas = engine.canvas_2d();
            (canvas.width(), canvas.height())
        };
        if canvas_w > 0 && canvas_h > 0 {
            return (canvas_w, canvas_h);
        }

        // Test/null engines may intentionally expose no real canvas. Their
        // root frame still provides a deterministic private recording extent.
        scene
            .root_id()
            .map(|id| {
                let frame = scene.node_frame(id);
                (
                    frame.w.ceil().max(1.0) as i32,
                    frame.h.ceil().max(1.0) as i32,
                )
            })
            .unwrap_or((1, 1))
    }

    fn ensure_recording_surface(&mut self, width: i32, height: i32) -> Result<(), Error> {
        let extent = (width.max(1), height.max(1));
        match self.recording_extent {
            Some(current) if current == extent => Ok(()),
            Some(_) => {
                self.recording_engine.resize(extent.0, extent.1)?;
                self.recording_extent = Some(extent);
                Ok(())
            }
            None => {
                self.recording_engine.initialize(extent.0, extent.1)?;
                self.recording_extent = Some(extent);
                Ok(())
            }
        }
    }
}

impl Default for FrameRenderer {
    fn default() -> Self {
        Self::new()
    }
}

fn draw_debug_telemetry(
    engine: &mut dyn GraphicsEngine,
    metrics: Option<&RenderMetrics>,
    font: FontHandle,
    font_service: &FontService,
) {
    let Some(m) = metrics else {
        return;
    };
    let canvas = engine.canvas_2d();
    let sw = canvas.width();
    let hud = DebugRenderService::new(true);
    hud.draw_telemetry_hud(canvas, m, sw);
    let lines = DebugRenderService::telemetry_hud_lines(m);
    let mut text_svc = TextRenderService::new(font, font_service, 300.0);
    // 文字在色条右侧，与 HUD 面板几何对齐。
    let text_x = DebugRenderService::hud_panel_x(sw) + DebugRenderService::HUD_PAD_X + 42.0;
    let text_top = DebugRenderService::HUD_MARGIN + DebugRenderService::HUD_TEXT_TOP;
    for (i, line) in lines.iter().enumerate() {
        text_svc.draw_text(
            canvas,
            line,
            Point::new(text_x, text_top + i as f32 * DebugRenderService::HUD_LINE_H),
            Color::from_rgba(220, 220, 220, 255),
            11.0,
        );
    }
}

fn classify_invalidation(rendered_first: bool, dirty_region: &DirtyRegion) -> InvalidationSource {
    if !rendered_first {
        return InvalidationSource::FirstFrame;
    }
    if !dirty_region.is_empty() {
        return InvalidationSource::DirtyRegion;
    }
    InvalidationSource::None
}

