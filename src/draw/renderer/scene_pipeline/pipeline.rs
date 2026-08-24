use super::*;

impl ScenePipeline {
    /// 创建一个尚未记录场景帧的空管线。
    pub fn new() -> Self {
        Self {
            layer_tree: LayerTree::new(),
            render_object_tree: RenderObjectTree::new(),
            last_tree_version: 0,
            recorder: CommandRecorder::new(),
            recording_extent: None,
            overlay_backdrop: None,
            overlay_backdrop_blocked: false,
            // 初始没有 overlay effect 计划。
            overlay_backdrop_effect: None,
            raster_pipeline: None,
            // 帧诊断阶段耗时从零开始。
            last_record_us: Duration::ZERO,
            last_submit_us: Duration::ZERO,
        }
    }

    /// 返回当前由场景同步得到的渲染对象树。
    pub fn render_object_tree(&self) -> &RenderObjectTree {
        &self.render_object_tree
    }

    /// 返回当前用于合成的图层树。
    pub fn layer_tree(&self) -> &LayerTree {
        &self.layer_tree
    }

    /// 返回当前图层树的可变引用。
    pub fn layer_tree_mut(&mut self) -> &mut LayerTree {
        &mut self.layer_tree
    }

    /// 执行单 Pass 渲染（Content + AfterChildren）；返回 Present damage 与 invalidation 来源。
    pub fn render_frame<S: ScenePaint>(
        &mut self,
        engine: &mut dyn RenderTarget,
        scene: &S,
        mut input: FrameRenderInput<'_>,
    ) -> FrameRenderOutput {
        let cur_version = scene.tree_version();
        let has_backdrop_overlay = scene
            .root_id()
            .is_some_and(|root| Self::scene_has_backdrop_overlay(scene, root));
        // UI 已把 Theme、区域策略与多 overlay 聚合为单一 typed 计划。
        let requested_backdrop_effect = scene.overlay_backdrop_effect();
        // 策略、半径或逻辑区域任一变化都属于 effect 失效。
        let backdrop_effect_changed = self.overlay_backdrop_effect != requested_backdrop_effect;
        if !has_backdrop_overlay {
            self.overlay_backdrop = None;
            // 需要背景快照的浮层离场时，资源释放失败必须在任何新帧动作前进入恢复路径。
            if let Err(error) = engine.release_overlay_backdrop() {
                // 不再继续 idle、begin_frame、paint 或 present。
                return Self::failed_backdrop_frame(error, cur_version);
            }
            self.overlay_backdrop_blocked = false;
            // 浮层离场同时清除已解析计划。
            self.overlay_backdrop_effect = None;
        }
        if input.rendered_first
            && input.dirty_region.is_empty()
            && input.scroll_move.is_none()
            && !backdrop_effect_changed
        {
            return FrameRenderOutput {
                outcome: RenderOutcome::Idle,
                inv_source: InvalidationSource::None,
                tree_version: cur_version,
            };
        }

        let frame_start_caps = engine.capabilities();
        let scroll_moves = input.scroll_move.as_deref().unwrap_or_default();
        let input_dirty_full = input.dirty_region.full_frame;
        // 接管调用方本帧快照；普通局部帧沿后端生命周期复用同一 Vec 分配。
        let mut paint_region = std::mem::take(&mut input.dirty_region);
        // effect 变化必须把旧、新逻辑区域同时送入区域失效。
        if backdrop_effect_changed {
            // 旧区域需要清除上一策略的像素影响。
            if let Some(effect) = self.overlay_backdrop_effect {
                // DirtyRegion 负责后续合并与裁剪。
                paint_region.add_rect(effect.region());
            }
            // 新区域需要绘制新的 backdrop 结果。
            if let Some(effect) = requested_backdrop_effect {
                // DPR 只由 backend blur boundary 应用一次。
                paint_region.add_rect(effect.region());
            }
        }
        for &(viewport, dx, dy) in scroll_moves {
            if let Some(exposed) = scroll_exposed_rect(viewport, dx, dy) {
                paint_region.add_rect(exposed);
            }
        }
        // 仅在保留缓冲明确支持重叠 memmove，且几何能无损映射到像素时启用。
        // 任一滚动不满足条件时，整批退回既有整视口重绘，避免同帧部分 copy。
        let use_scroll_copies = input.rendered_first
            && !input_dirty_full
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
        let mut present_region = if scroll_moves.is_empty() {
            None
        } else {
            let mut region = paint_region.clone();
            for &(viewport, _, _) in scroll_moves {
                if valid_frame_rect(viewport) {
                    region.add_rect(viewport);
                }
            }
            // present damage 同样包含旧、新 effect 区域。
            if backdrop_effect_changed {
                // 清除旧策略影响。
                if let Some(effect) = self.overlay_backdrop_effect {
                    // 记录旧逻辑区域。
                    region.add_rect(effect.region());
                }
                // 呈现新策略结果。
                if let Some(effect) = requested_backdrop_effect {
                    // 记录新逻辑区域。
                    region.add_rect(effect.region());
                }
            }
            Some(region)
        };
        // 页面切换等结构更新常产生多块高度重叠的脏区；若包围盒额外面积受控，
        // 扩大实际清理与 present damage，避免按矩形重复遍历和编码整棵场景。
        // 滚动搬移仍保留独立 viewport/条带契约，不参与该收敛。
        if scroll_moves.is_empty() {
            paint_region = coalesce_dense_dirty_region(paint_region);
        }

        // 判断正常树变化是否会污染现有 overlay backdrop。
        let normal_tree_dirty = has_backdrop_overlay
            && scene
                .root_id()
                .is_some_and(|root| Self::scene_normal_tree_dirty(scene, root));
        // GPU-native 可以在同一最终 present 前提交正常树并重建 clean snapshot。
        let refresh_overlay_backdrop = (normal_tree_dirty
            // 首帧没有历史 snapshot；只要请求了 effect 就必须主动建立 clean source。
            || (!input.rendered_first && requested_backdrop_effect.is_some()))
            // 首帧与后续正常树变化都必须先建立无 overlay 的 clean source。
            // debug overlay 不参与可复用 backdrop。
            && !input.debug_mode
            // 中间 FrameEncoder 提交只对 retained GPU 路径开放。
            && engine.raster_pipeline() == RasterPipeline::GpuNative
            // 本次 overlay 生命周期已证明正常树不可纯 GPU 编码时禁止重试优化。
            && !self.overlay_backdrop_blocked;
        // refresh 必须从完整正常树建立确定的 clean source。
        let draw_full = input.debug_mode
            || refresh_overlay_backdrop
            || !input.rendered_first
            || input_dirty_full
            || paint_region.is_empty()
            || !frame_start_caps.supports_partial_redraw();
        if refresh_overlay_backdrop {
            // CPU backdrop 不跨 GPU 中间提交复用。
            self.overlay_backdrop = None;
            // 新事务将在 begin_frame 后从完整正常树重建，不进入安全阻塞。
            self.overlay_backdrop_blocked = false;
        } else if has_backdrop_overlay && normal_tree_dirty {
            self.overlay_backdrop = None;
            // 正常树变化会使快照失效；销毁失败不能被整树重绘掩盖。
            if let Err(error) = engine.release_overlay_backdrop() {
                // 保留资源 owner，交由有界 recovery 或 shutdown 重试。
                return Self::failed_backdrop_frame(error, cur_version);
            }
            self.overlay_backdrop_blocked = true;
        } else if has_backdrop_overlay
            && backdrop_effect_changed
            && engine.has_overlay_backdrop()
            && !self.overlay_backdrop_blocked
        {
            // 已有独立干净快照时，策略变化只重新派生 effect，不重复快照。
            let blur_request = if engine.supports_backdrop_blur() {
                // Some 应用新效果；None 以 0 半径释放派生效果。
                requested_backdrop_effect
                    // 新请求直接携带区域与半径。
                    .map(|effect| (effect.region(), effect.radius()))
                    // 离场开始或关闭效果时复用旧区域发出 reset 事务。
                    .or_else(|| {
                        self.overlay_backdrop_effect
                            .map(|effect| (effect.region(), 0.0))
                    })
            } else {
                // 能力丢失时也必须清除旧派生结果，显式降级为 mask-only。
                self.overlay_backdrop_effect
                    // 仅在过去确实有 blur 时发送 reset。
                    .map(|effect| (effect.region(), 0.0))
            };
            // 无旧、新 effect 时不触碰底层资源。
            if let Some((region, radius)) = blur_request {
                // typed failure 必须中止当前帧，不能伪装为 fallback 成功。
                if let Err(error) = engine.blur_overlay_backdrop(region, radius) {
                    // 保留 dirty 状态，交给有界 recovery。
                    return Self::failed_backdrop_frame(error, cur_version);
                }
            }
        } else if has_backdrop_overlay
            && self.overlay_backdrop.is_none()
            && !engine.has_overlay_backdrop()
            && !self.overlay_backdrop_blocked
            && !refresh_overlay_backdrop
        {
            // This boundary still exposes the previous committed main surface;
            // after begin_frame/overlay paint it would already contain the mask.
            if input.rendered_first && !input.debug_mode {
                // CPU 可读路径优先；否则走保留色缓冲 GPU 纹理快照（无 readback）。
                self.overlay_backdrop = engine
                    .copy_frame_pixels()
                    .and_then(|(pixels, width)| Self::frame_image(pixels, width));
                if self.overlay_backdrop.is_none() {
                    // GPU 快照的真实资源错误必须越过场景边界进入 recovery。
                    if let Err(error) = engine.snapshot_overlay_backdrop() {
                        // 快照失败后不允许继续 begin_frame 或绘制浮层。
                        return Self::failed_backdrop_frame(error, cur_version);
                    }
                    // 真实 GPU 能力存在时，在独立干净快照上应用唯一效果计划。
                    if engine.has_overlay_backdrop() && engine.supports_backdrop_blur() {
                        // 默认关闭时不创建派生纹理。
                        if let Some(effect) = requested_backdrop_effect {
                            // blur 只执行 device submit，不重复 acquire/present。
                            match engine.blur_overlay_backdrop(effect.region(), effect.radius()) {
                                // 已执行后继续 overlay 帧。
                                Ok(true) => {}
                                // 能力在事务间失效时明确降级为 mask-only。
                                Ok(false) => {}
                                // 真实资源或提交失败进入 typed recovery。
                                Err(error) => {
                                    // 失败后禁止 begin_frame、paint 与 present。
                                    return Self::failed_backdrop_frame(error, cur_version);
                                }
                            }
                        }
                    }
                }
                self.overlay_backdrop_blocked =
                    self.overlay_backdrop.is_none() && !engine.has_overlay_backdrop();
            } else {
                self.overlay_backdrop_blocked = true;
            }
        }
        // 只有同步/降级事务未失败时才消费本帧请求计划。
        self.overlay_backdrop_effect = requested_backdrop_effect;
        let present_damage_region = present_region.as_ref().unwrap_or(&paint_region);
        let requested_present_damage =
            compute_present_damage(present_damage_region, draw_full, input.rendered_first);
        let requested_region = if draw_full {
            DirtyRegion::full()
        } else if use_scroll_copies {
            paint_region
        } else {
            present_region.take().unwrap_or(paint_region)
        };

        // backdrop refresh 必须在任何最终 begin_frame 之前完成正常树中间提交。
        if refresh_overlay_backdrop {
            // 两阶段事务强制全幅重建，并只在函数内部开始一次最终帧。
            return self.prepare_gpu_backdrop_refresh(
                // 传入唯一 retained target。
                engine,
                // 读取同一场景事实。
                scene,
                // 移交本帧输入。
                input,
                // 保留场景代际。
                cur_version,
                // refresh 重放完整正常树与 overlay。
                DirtyRegion::full(),
                // 完整恢复会改变全部 retained 像素，最终提交全幅 damage。
                DamageRegion::full(),
                // 传递当前 typed effect。
                requested_backdrop_effect,
            );
        }

        let requested_full_frame = requested_region.full_frame;
        let requested_bounds = requested_region.bounds();
        let strategy = if draw_full {
            UpdateStrategy::FullRedraw
        } else if let Some(copies) = scroll_copies {
            UpdateStrategy::ScrollCopies {
                dirty_rects: requested_region.into_rects(),
                copies,
            }
        } else {
            UpdateStrategy::DirtyRects(requested_region.into_rects())
        };
        // 策略只提交一次，直接移交其矩形与滚动所有权。
        let begin_outcome = engine.begin_frame(strategy);
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
        let region =
            match resolve_frame_region(requested_full_frame, requested_bounds, begin_damage) {
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
            // raster pipeline 切换前先检查式回收旧引擎的 backdrop owner。
            if let Err(error) = engine.release_overlay_backdrop() {
                // begin_frame 已开始，但失败后仍禁止 paint、end_frame 与 present。
                return Self::failed_backdrop_frame(error, cur_version);
            }
            self.overlay_backdrop_blocked = has_backdrop_overlay;
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
        let overlay_backdrop = (has_backdrop_overlay
            && region.full_frame
            && !input.debug_mode
            && !normal_tree_dirty
            && backdrop_extent_matches)
            .then(|| self.overlay_backdrop.clone())
            .flatten();
        let use_cpu_overlay_backdrop = overlay_backdrop.is_some();
        // GPU 快照由引擎持有；尺寸变化时 renderer 已释放，此处只查有效性。
        let use_gpu_overlay_backdrop = has_backdrop_overlay
            && region.full_frame
            && !input.debug_mode
            && !normal_tree_dirty
            && engine.has_overlay_backdrop();
        let use_overlay_backdrop = use_cpu_overlay_backdrop || use_gpu_overlay_backdrop;

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

        if raster_pipeline == RasterPipeline::GpuNative {
            // refresh 已在 begin_frame 前分流，此处只执行普通 GPU 路径。
            return self.render_gpu_native(
                engine,
                scene,
                input,
                cur_version,
                region,
                damage,
                use_gpu_overlay_backdrop,
            );
        }

        // Paint prune uses the same region as begin_frame clear. Full frames
        // keep DirtyRegion::full(); dirty frames omit the recording Clear so
        // execute_into_pixels retains undamaged CPU pixels.
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
        let render_result = if should_split_dirty_rects(&region) {
            let mut first = true;
            let mut result = Ok(());
            // 直接借用原矩形切片，避免每帧复制临时 Vec。
            for rect in region
                .rects()
                .iter()
                .copied()
                .filter(|rect| positive_dirty_rect(*rect))
            {
                self.recorder.canvas_2d().push_clip(rect);
                let sub_region = DirtyRegion::area(rect);
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
                        false,
                        None,
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
                        false,
                        None,
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
                    &region,
                    input.font,
                    input.font_service,
                    input.image_service,
                    false,
                    None,
                    render_objects,
                )
            } else {
                self.layer_tree.render(
                    &mut self.recorder,
                    scene,
                    &region,
                    input.font,
                    input.font_service,
                    input.image_service,
                    false,
                    None,
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
            draw_debug_overlay(
                &mut self.recorder,
                scene,
                input.hover_pos,
                input.debug_frame,
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
                classify_invalidation(input.rendered_first, &region, input.invalidation_source)
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
    pub(super) fn render_gpu_native<S: ScenePaint>(
        &mut self,
        engine: &mut dyn RenderTarget,
        scene: &S,
        input: FrameRenderInput<'_>,
        cur_version: u64,
        region: DirtyRegion,
        damage: DamageRegion,
        use_overlay_backdrop: bool,
    ) -> FrameRenderOutput {
        // 帧诊断：GPU 路径记录阶段起点（含场景遍历与绘制编码）。
        let stage_start = Instant::now();
        let mut use_overlay_backdrop = use_overlay_backdrop;
        if use_overlay_backdrop {
            // 不支持或代际不匹配仍可退回整树重绘，真实 typed failure 则终止本帧。
            match engine.restore_overlay_backdrop() {
                // 完整恢复后只重绘 overlay。
                Ok(true) => {}
                // 普通不可用保留既有整树重绘降级。
                Ok(false) => {
                    // 禁止后续路径继续使用失效快照。
                    use_overlay_backdrop = false;
                    // 清理失败同样必须传播，不能被 fallback 覆盖。
                    if let Err(error) = engine.release_overlay_backdrop() {
                        // begin_frame 后的失败不再进入 paint 或 end_frame。
                        return Self::failed_backdrop_frame(error, cur_version);
                    }
                    // 浮层离场前不再重复尝试捕获当前受污染表面。
                    self.overlay_backdrop_blocked = true;
                }
                // copy/submit/device maintenance 的 typed failure 直接进入 recovery。
                Err(error) => return Self::failed_backdrop_frame(error, cur_version),
            }
        }

        let render_result = if should_split_dirty_rects(&region) {
            let mut first = true;
            let mut result = Ok(());
            // GPU 路径同样直接遍历原矩形，保持与 CPU 录制路径一致。
            for rect in region
                .rects()
                .iter()
                .copied()
                .filter(|rect| positive_dirty_rect(*rect))
            {
                engine.canvas_2d().push_clip(rect);
                let sub_region = DirtyRegion::area(rect);
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
                        false,
                        None,
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
                        false,
                        None,
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
                    &region,
                    input.font,
                    input.font_service,
                    input.image_service,
                    false,
                    None,
                    render_objects,
                )
            } else {
                self.layer_tree.render(
                    engine,
                    scene,
                    &region,
                    input.font,
                    input.font_service,
                    input.image_service,
                    false,
                    None,
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
        // 帧诊断：记录阶段耗时（场景遍历 + 绘制编码）。
        self.last_record_us = stage_start.elapsed();
        if input.debug_mode {
            draw_debug_overlay(
                engine,
                scene,
                input.hover_pos,
                input.debug_frame,
                input.metrics,
                input.font,
                input.font_service,
            );
        }
        // 帧诊断：提交阶段起点（end_frame 含 GPU 提交与 present 等待）。
        let submit_start = Instant::now();
        let end_outcome = engine.end_frame(&damage);
        // 帧诊断：提交阶段耗时。
        self.last_submit_us = submit_start.elapsed();

        let caps = engine.capabilities();
        let outcome = normalize_end_outcome(end_outcome, caps, damage);
        let inv_source = match outcome {
            RenderOutcome::Present(_) | RenderOutcome::PresentPending(_) => {
                classify_invalidation(input.rendered_first, &region, input.invalidation_source)
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

    fn scene_has_backdrop_overlay(scene: &impl ScenePaint, id: crate::draw::scene::NodeId) -> bool {
        if !scene.node_visible(id) {
            return false;
        }
        scene.node_requires_overlay_backdrop(id)
            || scene
                .node_children(id)
                .iter()
                .copied()
                .any(|child| Self::scene_has_backdrop_overlay(scene, child))
    }

    fn scene_normal_tree_dirty(scene: &impl ScenePaint, id: crate::draw::scene::NodeId) -> bool {
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
}
