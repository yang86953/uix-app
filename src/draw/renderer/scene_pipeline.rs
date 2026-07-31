//! 帧渲染调度 — 从 UI event_loop 迁入的渲染段（Phase 3）。

use crate::core::{Errc, Error, Point, Rect};

use crate::core::DirtyRegion;

use crate::draw::backend::DamageRegion;
use crate::draw::command::{recorder::CommandRecorder, EncodedFrameExecution, FrameImage};
use crate::draw::debug::DebugRenderService;
use crate::draw::renderer::{InvalidationSource, RenderMetrics};
use crate::draw::resources::font::font_service::FontService;
use crate::draw::resources::font::text::TextRenderService;
use crate::draw::resources::image::ImageService;
use crate::draw::scene::{LayerTree, RenderObjectTree, ScenePaint};
use crate::draw::{Color, FontHandle, RenderOutcome};
use crate::draw::{RasterPipeline, RenderTarget, ScrollCopy, UpdateStrategy};

/// 单帧渲染输入。
pub struct FrameRenderInput<'a> {
    pub rendered_first: bool,
    pub dirty_region: &'a DirtyRegion,
    pub tree_version: u64,
    pub scroll_move: Option<Vec<(Rect, f32, f32)>>,
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
pub struct ScenePipeline {
    layer_tree: LayerTree,
    render_object_tree: RenderObjectTree,
    last_tree_version: u64,
    /// Private API-neutral producer for every scene path before the real
    /// backend consumes the one ordered main `FrameEncoder` (#181). It never
    /// owns a presentation surface and therefore cannot become a second
    /// submission boundary.
    recorder: CommandRecorder,
    recording_extent: Option<(i32, i32)>,
    /// Clean retained main surface captured immediately before the first
    /// root-level overlay frame clears it. Cloning FrameImage is cheap because
    /// its immutable pixels are shared.
    overlay_backdrop: Option<FrameImage>,
    /// Once the normal tree changes while an overlay is present, the current
    /// real surface already contains overlay pixels and can no longer become a
    /// clean backdrop. Wait for every overlay to leave before capturing again.
    overlay_backdrop_blocked: bool,
    /// Prevents Picture handles created by one raster owner from being reused
    /// after bounded recovery switches the live engine.
    raster_pipeline: Option<RasterPipeline>,
}

impl ScenePipeline {
    pub fn new() -> Self {
        Self {
            layer_tree: LayerTree::new(),
            render_object_tree: RenderObjectTree::new(),
            last_tree_version: 0,
            recorder: CommandRecorder::new(),
            recording_extent: None,
            overlay_backdrop: None,
            overlay_backdrop_blocked: false,
            raster_pipeline: None,
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
        engine: &mut dyn RenderTarget,
        scene: &S,
        input: FrameRenderInput<'_>,
    ) -> FrameRenderOutput {
        let cur_version = scene.tree_version();
        let has_overlay = scene
            .root_id()
            .is_some_and(|root| Self::scene_has_overlay(scene, root));
        if !has_overlay {
            self.overlay_backdrop = None;
            engine.release_overlay_backdrop();
            self.overlay_backdrop_blocked = false;
        }
        if input.rendered_first && input.dirty_region.is_empty() && input.scroll_move.is_none() {
            return FrameRenderOutput {
                outcome: RenderOutcome::Idle,
                inv_source: InvalidationSource::None,
                tree_version: cur_version,
            };
        }

        let frame_start_caps = engine.capabilities();
        let scroll_moves = input.scroll_move.as_deref().unwrap_or_default();
        // 即使上游只交付 scroll 参数，也由帧边界补齐 exposed strip；调用方仍零维护。
        let mut dirty_for_paint = input.dirty_region.clone();
        for &(viewport, dx, dy) in scroll_moves {
            if let Some(exposed) = scroll_exposed_rect(viewport, dx, dy) {
                dirty_for_paint.add_rect(exposed);
            }
        }
        let dirty_for_paint = dirty_for_paint.for_paint_clear();

        // 仅在保留缓冲明确支持重叠 memmove，且几何能无损映射到像素时启用。
        // 任一滚动不满足条件时，整批退回既有整视口重绘，避免同帧部分 copy。
        let use_scroll_copies = input.rendered_first
            && !input.dirty_region.full_frame
            && frame_start_caps.supports_scroll_memmove()
            && !scroll_moves.is_empty()
            && scroll_moves
                .iter()
                .all(|&(viewport, dx, dy)| valid_scroll_copy(viewport, dx, dy));
        let scroll_copies = use_scroll_copies.then(|| {
            scroll_moves
                .iter()
                .map(|&(viewport, dx, dy)| ScrollCopy::new(viewport, dx.round(), dy.round()))
                .collect::<Vec<_>>()
        });

        // present damage 覆盖所有实际变化像素：即使只重绘 exposed strip，滚动视口
        // 内的保留像素也发生了移动，外部 presenter 必须提交整个视口。
        let dirty_with_scroll = {
            let mut region = if scroll_moves.is_empty() {
                input.dirty_region.for_paint_clear()
            } else {
                input.dirty_region.clone()
            };
            for &(viewport, _, _) in scroll_moves {
                if valid_frame_rect(viewport) {
                    region.add_rect(viewport);
                }
            }
            region
        };

        let draw_full = !input.rendered_first
            || input.dirty_region.full_frame
            || dirty_for_paint.is_empty()
            || !frame_start_caps.supports_partial_redraw();
        let normal_tree_dirty = has_overlay
            && scene
                .root_id()
                .is_some_and(|root| Self::scene_normal_tree_dirty(scene, root));
        if has_overlay && normal_tree_dirty {
            self.overlay_backdrop = None;
            engine.release_overlay_backdrop();
            self.overlay_backdrop_blocked = true;
        } else if has_overlay
            && self.overlay_backdrop.is_none()
            && !engine.has_overlay_backdrop()
            && !self.overlay_backdrop_blocked
        {
            // This boundary still exposes the previous committed main surface;
            // after begin_frame/overlay paint it would already contain the mask.
            if input.rendered_first && !input.debug_mode {
                // CPU 可读路径优先；否则走保留色缓冲 GPU 纹理快照（无 readback）。
                self.overlay_backdrop = engine
                    .copy_frame_pixels()
                    .and_then(|(pixels, width)| Self::frame_image(pixels, width));
                if self.overlay_backdrop.is_none() {
                    let _ = engine.snapshot_overlay_backdrop();
                }
                self.overlay_backdrop_blocked =
                    self.overlay_backdrop.is_none() && !engine.has_overlay_backdrop();
            } else {
                self.overlay_backdrop_blocked = true;
            }
        }
        let requested_present_damage = compute_present_damage(
            &dirty_with_scroll,
            input.dirty_region.full_frame,
            input.rendered_first,
        );
        let requested_region = if draw_full {
            DirtyRegion::full()
        } else if use_scroll_copies {
            dirty_for_paint
        } else {
            dirty_with_scroll
        };

        let strategy = if draw_full {
            UpdateStrategy::FullRedraw
        } else if let Some(copies) = scroll_copies {
            UpdateStrategy::ScrollCopies {
                dirty_rects: requested_region.rects().to_vec(),
                copies,
            }
        } else {
            UpdateStrategy::DirtyRects(requested_region.rects().to_vec())
        };
        let begin_outcome = engine.begin_frame(strategy.clone());
        let begin_damage = match begin_outcome {
            RenderOutcome::FrameReady(damage) => damage,
            RenderOutcome::Present(_) => {
                return FrameRenderOutput {
                    outcome: RenderOutcome::Failed(
                        crate::draw::renderer::GraphicsFailure::from_error(
                            crate::core::Error::new(
                                crate::core::Errc::InvalidState,
                                "begin_frame reported a final presentation",
                            ),
                        ),
                    ),
                    inv_source: InvalidationSource::None,
                    tree_version: cur_version,
                };
            }
            RenderOutcome::PresentPending(_) => {
                return FrameRenderOutput {
                    outcome: RenderOutcome::Failed(
                        crate::draw::renderer::GraphicsFailure::from_error(
                            crate::core::Error::new(
                                crate::core::Errc::InvalidState,
                                "begin_frame reported an external presentation pending result",
                            ),
                        ),
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
        };
        let begin_promoted_full = !draw_full && begin_damage.full;
        let region = match resolve_frame_region(&requested_region, begin_damage) {
            Ok(region) => region,
            Err(error) => {
                return FrameRenderOutput {
                    outcome: RenderOutcome::Failed(
                        crate::draw::renderer::GraphicsFailure::from_error(error),
                    ),
                    inv_source: InvalidationSource::None,
                    tree_version: cur_version,
                };
            }
        };
        let damage = if begin_promoted_full {
            DamageRegion::full()
        } else {
            requested_present_damage
        };
        let strategy_full = region.full_frame as u8;

        // 恢复包装器可在 begin_frame 内切换到 Software，后续呈现协议须读取新引擎能力。
        let caps = engine.capabilities();
        let raster_pipeline = engine.raster_pipeline();
        if self.raster_pipeline != Some(raster_pipeline) {
            // The previous engine owns any native Picture handles. Its
            // shutdown reclaims them; never pass those opaque ids to the new
            // engine after recovery.
            self.layer_tree = LayerTree::new();
            self.render_object_tree = RenderObjectTree::new();
            self.last_tree_version = 0;
            self.overlay_backdrop = None;
            engine.release_overlay_backdrop();
            self.overlay_backdrop_blocked = has_overlay;
            self.raster_pipeline = Some(raster_pipeline);
        }

        let (recording_w, recording_h) = Self::reference_extent(engine, scene);
        if let Err(error) = self.ensure_recording_surface(recording_w, recording_h) {
            return FrameRenderOutput {
                outcome: RenderOutcome::Failed(crate::draw::renderer::GraphicsFailure::from_error(
                    error,
                )),
                inv_source: InvalidationSource::None,
                tree_version: cur_version,
            };
        }
        let backdrop_extent_matches = self
            .overlay_backdrop
            .as_ref()
            .is_some_and(|image| image.width() == recording_w && image.height() == recording_h);
        if self.overlay_backdrop.is_some() && !backdrop_extent_matches {
            self.overlay_backdrop = None;
            self.overlay_backdrop_blocked = true;
        }
        let overlay_backdrop = (has_overlay
            && region.full_frame
            && !input.debug_mode
            && !normal_tree_dirty
            && backdrop_extent_matches)
            .then(|| self.overlay_backdrop.clone())
            .flatten();
        let use_cpu_overlay_backdrop = overlay_backdrop.is_some();
        // GPU 快照由引擎持有；尺寸变化时 renderer 已释放，此处只查有效性。
        let use_gpu_overlay_backdrop = has_overlay
            && region.full_frame
            && !input.debug_mode
            && !normal_tree_dirty
            && engine.has_overlay_backdrop();
        let use_overlay_backdrop = use_cpu_overlay_backdrop || use_gpu_overlay_backdrop;

        let layer_t0 = std::time::Instant::now();
        if self.last_tree_version != cur_version || !self.layer_tree.is_ready() {
            self.layer_tree.build(
                scene,
                raster_pipeline == RasterPipeline::GpuNative || caps.supports_offscreen(),
            );
            let sweep_result = if raster_pipeline == RasterPipeline::GpuNative {
                self.layer_tree.sweep_orphaned_offscreens(engine)
            } else {
                self.layer_tree
                    .sweep_orphaned_offscreens(&mut self.recorder)
            };
            if let Err(error) = sweep_result {
                return FrameRenderOutput {
                    outcome: RenderOutcome::Failed(
                        crate::draw::renderer::GraphicsFailure::from_error(error),
                    ),
                    inv_source: InvalidationSource::None,
                    tree_version: cur_version,
                };
            }
            self.last_tree_version = cur_version;
        }
        self.layer_tree.update_dirty(scene);
        self.render_object_tree.sync(scene);
        let layer_build_us = layer_t0.elapsed().as_micros();

        if raster_pipeline == RasterPipeline::GpuNative {
            return self.render_gpu_native(
                engine,
                scene,
                input,
                cur_version,
                region,
                damage,
                strategy_full,
                layer_build_us,
                use_gpu_overlay_backdrop,
            );
        }

        // Paint prune uses the same region as begin_frame clear. Full frames
        // keep DirtyRegion::full(); dirty frames omit the recording Clear so
        // execute_into_pixels retains undamaged CPU pixels.
        let paint_region = region.clone();
        crate::core::perf_probe::begin_record_acc();
        let record_t0 = std::time::Instant::now();
        if let Err(error) = self.recorder.begin_recording(region.full_frame) {
            return FrameRenderOutput {
                outcome: RenderOutcome::Failed(crate::draw::renderer::GraphicsFailure::from_error(
                    error,
                )),
                inv_source: InvalidationSource::None,
                tree_version: cur_version,
            };
        }
        if let Some(image) = overlay_backdrop {
            if let Err(error) = self.recorder.record_main_image(image) {
                return FrameRenderOutput {
                    outcome: RenderOutcome::Failed(
                        crate::draw::renderer::GraphicsFailure::from_error(error),
                    ),
                    inv_source: InvalidationSource::None,
                    tree_version: cur_version,
                };
            }
        }
        // 多块脏区：逐矩形 clip + 以该矩形为 dirty 剪枝重绘，父背景只填当前洞，
        // 不污染空隙中的干净像素。单矩形仍走一次 clip（与 begin_frame 外层并集 clip 叠加）。
        let split_rects: Vec<Rect> = if !region.full_frame && region.rects().len() > 1 {
            region
                .rects()
                .iter()
                .copied()
                .filter(|r| r.w > 0.0 && r.h > 0.0)
                .collect()
        } else {
            Vec::new()
        };
        let render_result = if !split_rects.is_empty() {
            let mut first = true;
            let mut result = Ok(());
            for rect in &split_rects {
                self.recorder.canvas_2d().push_clip(*rect);
                let sub_region = DirtyRegion::area(*rect);
                let render_objects = if first && input.rendered_first {
                    first = false;
                    Some(&mut self.render_object_tree)
                } else {
                    first = false;
                    None
                };
                let pass = if use_overlay_backdrop {
                    self.layer_tree.render_overlays(
                        &mut self.recorder,
                        scene,
                        &sub_region,
                        input.font,
                        input.font_service,
                        input.image_service,
                        input.debug_mode,
                        input.hover_pos,
                        render_objects,
                    )
                } else {
                    self.layer_tree.render(
                        &mut self.recorder,
                        scene,
                        &sub_region,
                        input.font,
                        input.font_service,
                        input.image_service,
                        input.debug_mode,
                        input.hover_pos,
                        render_objects,
                    )
                };
                self.recorder.canvas_2d().pop_clip();
                if let Err(error) = pass {
                    result = Err(error);
                    break;
                }
            }
            result
        } else {
            // Dirty frames: clip recording to the damage AABB when a single hole
            // remains. begin_frame already cleared that rect on the retained CPU
            // canvas, but FrameEncoder execution bypasses the real surface clip.
            let damage_clip = (!region.full_frame)
                .then(|| region.bounds())
                .filter(|bounds| bounds.w > 0.0 && bounds.h > 0.0);
            if let Some(bounds) = damage_clip {
                self.recorder.canvas_2d().push_clip(bounds);
            }
            let render_objects = if input.rendered_first {
                Some(&mut self.render_object_tree)
            } else {
                None
            };
            let render_result = if use_overlay_backdrop {
                self.layer_tree.render_overlays(
                    &mut self.recorder,
                    scene,
                    &paint_region,
                    input.font,
                    input.font_service,
                    input.image_service,
                    input.debug_mode,
                    input.hover_pos,
                    render_objects,
                )
            } else {
                self.layer_tree.render(
                    &mut self.recorder,
                    scene,
                    &paint_region,
                    input.font,
                    input.font_service,
                    input.image_service,
                    input.debug_mode,
                    input.hover_pos,
                    render_objects,
                )
            };
            if damage_clip.is_some() {
                self.recorder.canvas_2d().pop_clip();
            }
            render_result
        };
        if let Err(error) = render_result {
            // An offscreen bind/flush/blit failure occurred while recording.
            // Do not call end_frame: that could submit a partial frame or turn
            // the failure into a final present. LayerTree leaves the affected
            // Picture dirty, and the caller retains invalidation for recovery.
            return FrameRenderOutput {
                outcome: RenderOutcome::Failed(crate::draw::renderer::GraphicsFailure::from_error(
                    error,
                )),
                inv_source: InvalidationSource::None,
                tree_version: cur_version,
            };
        }

        if input.debug_mode {
            draw_debug_telemetry(
                &mut self.recorder,
                input.metrics,
                input.font,
                input.font_service,
            );
        }

        let encoded_frame = match self.recorder.finish_recording() {
            Ok(encoder) => encoder,
            Err(error) => {
                self.layer_tree.invalidate();
                return FrameRenderOutput {
                    outcome: RenderOutcome::Failed(
                        crate::draw::renderer::GraphicsFailure::from_error(error),
                    ),
                    inv_source: InvalidationSource::None,
                    tree_version: cur_version,
                };
            }
        };
        let record_us = record_t0.elapsed().as_micros();
        let execute_t0 = std::time::Instant::now();
        match engine.try_execute_encoded_frame(&encoded_frame) {
            Ok(EncodedFrameExecution::Executed) => {}
            Ok(EncodedFrameExecution::Unsupported) => {
                self.layer_tree.invalidate();
                return FrameRenderOutput {
                    outcome: RenderOutcome::Failed(
                        crate::draw::renderer::GraphicsFailure::from_error(Error::new(
                            crate::core::Errc::InvalidState,
                            "render target does not execute the required main FrameEncoder",
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
                        crate::draw::renderer::GraphicsFailure::from_error(error),
                    ),
                    inv_source: InvalidationSource::None,
                    tree_version: cur_version,
                };
            }
        }
        let execute_us = execute_t0.elapsed().as_micros();

        let end_t0 = std::time::Instant::now();
        let end_outcome = engine.end_frame(&damage);
        let end_frame_us = end_t0.elapsed().as_micros();
        let mut paint_sample = crate::core::perf_probe::take_record_acc();
        paint_sample.layer_build_us = layer_build_us;
        paint_sample.record_us = record_us;
        paint_sample.execute_us = execute_us;
        paint_sample.end_frame_us = end_frame_us;
        paint_sample.strategy_full = strategy_full;
        paint_sample.backdrop_restore = u8::from(use_overlay_backdrop);
        crate::core::perf_probe::record_paint(paint_sample);
        let outcome = match end_outcome {
            RenderOutcome::Present(_) if caps.uses_external_presenter() => {
                RenderOutcome::PresentPending(damage)
            }
            RenderOutcome::Present(_) => RenderOutcome::Present(damage),
            RenderOutcome::PresentPending(_) if caps.uses_external_presenter() => {
                RenderOutcome::PresentPending(damage)
            }
            RenderOutcome::PresentPending(_) => RenderOutcome::Failed(
                crate::draw::renderer::GraphicsFailure::from_error(crate::core::Error::new(
                    crate::core::Errc::InvalidState,
                    "backend-managed backend returned an external presentation pending result",
                )),
            ),
            RenderOutcome::FrameReady(_) => RenderOutcome::Failed(
                crate::draw::renderer::GraphicsFailure::from_error(crate::core::Error::new(
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

    #[allow(clippy::too_many_arguments)]
    fn render_gpu_native<S: ScenePaint>(
        &mut self,
        engine: &mut dyn RenderTarget,
        scene: &S,
        input: FrameRenderInput<'_>,
        cur_version: u64,
        region: DirtyRegion,
        damage: DamageRegion,
        strategy_full: u8,
        layer_build_us: u128,
        use_overlay_backdrop: bool,
    ) -> FrameRenderOutput {
        let mut use_overlay_backdrop = use_overlay_backdrop;
        if use_overlay_backdrop && !engine.restore_overlay_backdrop() {
            // 恢复失败时退回整树重绘，并阻塞后续快照直至浮层离场。
            use_overlay_backdrop = false;
            engine.release_overlay_backdrop();
            self.overlay_backdrop_blocked = true;
        }

        let paint_region = region.clone();
        let split_rects: Vec<Rect> = if !region.full_frame && region.rects().len() > 1 {
            region
                .rects()
                .iter()
                .copied()
                .filter(|r| r.w > 0.0 && r.h > 0.0)
                .collect()
        } else {
            Vec::new()
        };
        crate::core::perf_probe::begin_record_acc();
        let execute_t0 = std::time::Instant::now();
        let render_result = if !split_rects.is_empty() {
            let mut first = true;
            let mut result = Ok(());
            for rect in &split_rects {
                engine.canvas_2d().push_clip(*rect);
                let sub_region = DirtyRegion::area(*rect);
                let render_objects = if first && input.rendered_first {
                    first = false;
                    Some(&mut self.render_object_tree)
                } else {
                    first = false;
                    None
                };
                let pass = if use_overlay_backdrop {
                    self.layer_tree.render_overlays(
                        engine,
                        scene,
                        &sub_region,
                        input.font,
                        input.font_service,
                        input.image_service,
                        input.debug_mode,
                        input.hover_pos,
                        render_objects,
                    )
                } else {
                    self.layer_tree.render(
                        engine,
                        scene,
                        &sub_region,
                        input.font,
                        input.font_service,
                        input.image_service,
                        input.debug_mode,
                        input.hover_pos,
                        render_objects,
                    )
                };
                engine.canvas_2d().pop_clip();
                if let Err(error) = pass {
                    result = Err(error);
                    break;
                }
            }
            result
        } else {
            let damage_clip = (!region.full_frame)
                .then(|| region.bounds())
                .filter(|bounds| bounds.w > 0.0 && bounds.h > 0.0);
            if let Some(bounds) = damage_clip {
                engine.canvas_2d().push_clip(bounds);
            }
            let render_objects = if input.rendered_first {
                Some(&mut self.render_object_tree)
            } else {
                None
            };
            let render_result = if use_overlay_backdrop {
                self.layer_tree.render_overlays(
                    engine,
                    scene,
                    &paint_region,
                    input.font,
                    input.font_service,
                    input.image_service,
                    input.debug_mode,
                    input.hover_pos,
                    render_objects,
                )
            } else {
                self.layer_tree.render(
                    engine,
                    scene,
                    &paint_region,
                    input.font,
                    input.font_service,
                    input.image_service,
                    input.debug_mode,
                    input.hover_pos,
                    render_objects,
                )
            };
            if damage_clip.is_some() {
                engine.canvas_2d().pop_clip();
            }
            render_result
        };
        if let Err(error) = render_result {
            self.layer_tree.invalidate();
            return FrameRenderOutput {
                outcome: RenderOutcome::Failed(crate::draw::renderer::GraphicsFailure::from_error(
                    error,
                )),
                inv_source: InvalidationSource::None,
                tree_version: cur_version,
            };
        }
        if input.debug_mode {
            draw_debug_telemetry(engine, input.metrics, input.font, input.font_service);
        }
        let execute_us = execute_t0.elapsed().as_micros();

        let end_t0 = std::time::Instant::now();
        let end_outcome = engine.end_frame(&damage);
        let end_frame_us = end_t0.elapsed().as_micros();
        let mut paint_sample = crate::core::perf_probe::take_record_acc();
        paint_sample.layer_build_us = layer_build_us;
        paint_sample.record_us = 0;
        paint_sample.execute_us = execute_us;
        paint_sample.end_frame_us = end_frame_us;
        paint_sample.strategy_full = strategy_full;
        paint_sample.backdrop_restore = u8::from(use_overlay_backdrop);
        crate::core::perf_probe::record_paint(paint_sample);

        let caps = engine.capabilities();
        let outcome = normalize_end_outcome(end_outcome, caps, damage);
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

    fn scene_has_overlay(scene: &impl ScenePaint, id: crate::draw::renderer::NodeId) -> bool {
        if !scene.node_visible(id) {
            return false;
        }
        scene.node_is_overlay(id)
            || scene
                .node_children(id)
                .iter()
                .copied()
                .any(|child| Self::scene_has_overlay(scene, child))
    }

    fn scene_normal_tree_dirty(scene: &impl ScenePaint, id: crate::draw::renderer::NodeId) -> bool {
        if !scene.node_visible(id) || scene.node_is_overlay(id) {
            return false;
        }
        scene.node_dirty(id)
            || scene
                .node_children(id)
                .iter()
                .copied()
                .any(|child| Self::scene_normal_tree_dirty(scene, child))
    }

    fn frame_image(pixels: Vec<u32>, width: i32) -> Option<FrameImage> {
        let width_usize = usize::try_from(width).ok().filter(|width| *width > 0)?;
        if pixels.is_empty() || !pixels.len().is_multiple_of(width_usize) {
            return None;
        }
        let height = i32::try_from(pixels.len() / width_usize).ok()?;
        FrameImage::new(width, height, pixels).ok()
    }

    fn reference_extent<S: ScenePaint>(engine: &mut dyn RenderTarget, scene: &S) -> (i32, i32) {
        let (canvas_w, canvas_h) = {
            let canvas = engine.canvas_2d();
            (canvas.width(), canvas.height())
        };
        if canvas_w > 0 && canvas_h > 0 {
            return (canvas_w, canvas_h);
        }

        // The test backend may intentionally expose no real canvas. Its
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
                self.recorder.resize(extent.0, extent.1)?;
                self.recording_extent = Some(extent);
                Ok(())
            }
            None => {
                self.recorder.initialize(extent.0, extent.1)?;
                self.recording_extent = Some(extent);
                Ok(())
            }
        }
    }
}

impl Default for ScenePipeline {
    fn default() -> Self {
        Self::new()
    }
}

fn draw_debug_telemetry(
    engine: &mut dyn RenderTarget,
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

fn normalize_end_outcome(
    end_outcome: RenderOutcome,
    caps: crate::draw::GraphicsCapabilities,
    damage: DamageRegion,
) -> RenderOutcome {
    match end_outcome {
        RenderOutcome::Present(_) if caps.uses_external_presenter() => {
            RenderOutcome::PresentPending(damage)
        }
        RenderOutcome::Present(_) => RenderOutcome::Present(damage),
        RenderOutcome::PresentPending(_) if caps.uses_external_presenter() => {
            RenderOutcome::PresentPending(damage)
        }
        RenderOutcome::PresentPending(_) => RenderOutcome::Failed(
            crate::draw::renderer::GraphicsFailure::from_error(Error::new(
                Errc::InvalidState,
                "backend-managed backend returned an external presentation pending result",
            )),
        ),
        RenderOutcome::FrameReady(_) => RenderOutcome::Failed(
            crate::draw::renderer::GraphicsFailure::from_error(Error::new(
                Errc::InvalidState,
                "end_frame returned FrameReady instead of final presentation",
            )),
        ),
        RenderOutcome::Idle => RenderOutcome::Idle,
        RenderOutcome::Failed(error) => RenderOutcome::Failed(error),
    }
}

fn compute_present_damage(
    dirty: &DirtyRegion,
    full_frame: bool,
    rendered_first: bool,
) -> DamageRegion {
    if !rendered_first || full_frame {
        return DamageRegion::full();
    }
    // scroll 视口已并入 dirty（见 render_frame 的 dirty_with_scroll 构造）。
    let rects: Vec<Rect> = dirty
        .rects()
        .iter()
        .filter(|r| r.w > 0.0 && r.h > 0.0)
        .map(pad_damage_rect)
        .collect();
    if rects.is_empty() {
        DamageRegion::full()
    } else {
        DamageRegion::partial(rects)
    }
}

fn valid_scroll_copy(viewport: Rect, dx: f32, dy: f32) -> bool {
    if !valid_frame_rect(viewport)
        || ![viewport.x, viewport.y, viewport.w, viewport.h]
            .into_iter()
            .all(|value| value == value.round())
        || !dx.is_finite()
        || !dy.is_finite()
    {
        return false;
    }
    let dx = dx.round();
    let dy = dy.round();
    (dx != 0.0 || dy != 0.0)
        && (dx == 0.0 || dx.abs() < viewport.w)
        && (dy == 0.0 || dy.abs() < viewport.h)
}

fn scroll_exposed_rect(viewport: Rect, dx: f32, dy: f32) -> Option<Rect> {
    if !valid_frame_rect(viewport) || !dx.is_finite() || !dy.is_finite() {
        return None;
    }
    let dx = dx.round();
    let dy = dy.round();
    let horizontal = (dx != 0.0).then(|| {
        let width = dx.abs().min(viewport.w);
        let x = if dx > 0.0 {
            viewport.x + viewport.w - width
        } else {
            viewport.x
        };
        Rect::new(x, viewport.y, width, viewport.h)
    });
    let vertical = (dy != 0.0).then(|| {
        let height = dy.abs().min(viewport.h);
        let y = if dy > 0.0 {
            viewport.y + viewport.h - height
        } else {
            viewport.y
        };
        Rect::new(viewport.x, y, viewport.w, height)
    });
    match (horizontal, vertical) {
        (Some(horizontal), Some(vertical)) => Some(horizontal.union(&vertical)),
        (Some(rect), None) | (None, Some(rect)) => Some(rect),
        (None, None) => None,
    }
}

/// 以 `begin_frame` 返回的实际清区为权威绘制区，并验证它覆盖请求区。
fn resolve_frame_region(
    requested: &DirtyRegion,
    actual: DamageRegion,
) -> Result<DirtyRegion, Error> {
    if actual.full {
        return Ok(DirtyRegion::full());
    }
    if requested.full_frame {
        return Err(Error::new(
            Errc::InvalidState,
            "begin_frame returned partial damage for a full redraw request",
        ));
    }

    let mut actual_region = DirtyRegion::empty();
    for rect in actual.rects {
        if !valid_frame_rect(rect) {
            return Err(Error::new(
                Errc::InvalidState,
                "begin_frame returned an invalid partial damage rectangle",
            ));
        }
        actual_region.add_rect(rect);
    }
    let actual_region = actual_region.for_paint_clear();
    if actual_region.is_empty() || !rect_covers(actual_region.bounds(), requested.bounds()) {
        return Err(Error::new(
            Errc::InvalidState,
            "begin_frame damage does not cover the requested paint region",
        ));
    }
    Ok(actual_region)
}

fn valid_frame_rect(rect: Rect) -> bool {
    rect.x.is_finite()
        && rect.y.is_finite()
        && rect.w.is_finite()
        && rect.h.is_finite()
        && rect.w > 0.0
        && rect.h > 0.0
}

fn rect_covers(outer: Rect, inner: Rect) -> bool {
    outer.x <= inner.x
        && outer.y <= inner.y
        && outer.x + outer.w >= inner.x + inner.w
        && outer.y + outer.h >= inner.y + inner.h
}

fn pad_damage_rect(r: &Rect) -> Rect {
    Rect::new(
        (r.x - 1.0).max(0.0),
        (r.y - 1.0).max(0.0),
        r.w + 2.0,
        r.h + 2.0,
    )
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
