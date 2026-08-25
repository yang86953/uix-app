//! Overlay backdrop 的 clean refresh 事务。

// 引入 ScenePipeline 私有状态与同模块渲染契约。
use super::*;

impl ScenePipeline {
    // 将 backdrop 资源事务失败统一转换为不可提交的帧结果。
    pub(super) fn failed_backdrop_frame(error: Error, tree_version: u64) -> FrameRenderOutput {
        // 保留底层错误分类，供 RecoveryDriver 选择 surface 或 device 恢复动作。
        FrameRenderOutput {
            // 禁止把资源事务失败伪装成普通整树重绘。
            outcome: RenderOutcome::Failed(
                // 使用统一图形错误映射保留 DeviceLost、SurfaceLost 与 OOM。
                crate::draw::renderer::GraphicsFailure::from_error(error),
            ),
            // 失败帧不得消费任何 invalidation 来源。
            inv_source: InvalidationSource::None,
            // 回传当前场景代际，调用方仍可保留对应 dirty 状态。
            tree_version,
        }
    }

    /// 在最终 begin_frame 之前准备并执行 clean snapshot 两阶段事务。
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare_gpu_backdrop_refresh<S: ScenePaint>(
        // 借用当前唯一 GPU target。
        &mut self,
        // 借用当前场景。
        engine: &mut dyn RenderTarget,
        // 借用当前场景事实。
        scene: &S,
        // 移交本帧输入。
        input: FrameRenderInput<'_>,
        // 保存场景代际。
        tree_version: u64,
        // 保存本帧绘制区域。
        region: DirtyRegion,
        // 保存最终 present damage。
        damage: DamageRegion,
        // 保存当前 typed effect。
        effect: Option<crate::draw::OverlayBackdropEffect>,
    ) -> FrameRenderOutput {
        // refresh 只接受调用点已经确认的 GPU-native target。
        if engine.raster_pipeline() != RasterPipeline::GpuNative {
            // 能力在分流前丢失时保持场景 dirty，并交给恢复层重试。
            return Self::failed_backdrop_frame(
                // 返回稳定的状态错误而不是进入平行软件路径。
                Error::new(
                    // 当前时序不再满足 retained GPU 前置条件。
                    Errc::InvalidState,
                    // 说明 refresh 必须在 GPU-native owner 上执行。
                    "overlay backdrop refresh lost the retained GPU pipeline",
                ),
                // 保留当前场景代际。
                tree_version,
            );
        }
        // 切换进入 GPU-native 时先清除旧 pipeline 的不透明缓存身份。
        if self.raster_pipeline != Some(RasterPipeline::GpuNative) {
            // 丢弃只属于旧 adapter 的 Layer 资源。
            self.layer_tree = LayerTree::new();
            // 丢弃旧渲染对象索引。
            self.render_object_tree = RenderObjectTree::new();
            // 强制按当前代际重建。
            self.last_tree_version = 0;
            // 清除 CPU backdrop 副本。
            self.overlay_backdrop = None;
            // 检查式释放旧引擎可能持有的 GPU backdrop。
            if let Err(error) = engine.release_overlay_backdrop() {
                // 资源失败禁止开始最终帧。
                return Self::failed_backdrop_frame(error, tree_version);
            }
            // refresh 将立即建立新 clean source。
            self.overlay_backdrop_blocked = false;
            // 登记当前 pipeline 身份。
            self.raster_pipeline = Some(RasterPipeline::GpuNative);
        }
        // 录制器尺寸必须与当前逻辑 surface 一致。
        let (recording_w, recording_h) = Self::reference_extent(engine, scene);
        // 在任何中间提交前准备 API-neutral recorder surface。
        if let Err(error) = self.ensure_recording_surface(recording_w, recording_h) {
            // 失败时不触碰最终帧。
            return Self::failed_backdrop_frame(error, tree_version);
        }
        // 场景代际变化或首次进入时重建 LayerTree。
        if self.last_tree_version != tree_version || !self.layer_tree.is_ready() {
            // GPU-native 层树允许创建 retained offscreen 计划。
            self.layer_tree.build(scene, true);
            // 清理当前场景不再引用的旧离屏资源。
            if let Err(error) = self.layer_tree.sweep_orphaned_offscreens(engine) {
                // 清理失败保持 typed owner 错误。
                return Self::failed_backdrop_frame(error, tree_version);
            }
            // 记录已完成结构同步的代际。
            self.last_tree_version = tree_version;
        }
        // 合并当前场景 dirty 标记。
        self.layer_tree.update_dirty(scene);
        // 同步正常树与 overlay 的渲染对象索引。
        self.render_object_tree.sync(scene);
        // 进入严格的 normal→snapshot→blur→begin→restore→overlay→end 顺序。
        self.render_gpu_backdrop_refresh(
            // 传递唯一 target。
            engine,
            // 传递同一场景。
            scene,
            // 移交输入。
            input,
            // 传递代际。
            tree_version,
            // 传递完整区域。
            region,
            // 传递最终 damage。
            damage,
            // 传递 typed effect。
            effect,
        )
    }

    /// 在一次最终 present 内提交正常树、重建 clean/effect，再绘制 overlay。
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_gpu_backdrop_refresh<S: ScenePaint>(
        // 借用当前唯一 retained GPU target。
        &mut self,
        // 借用 renderer/recovery 包装器。
        engine: &mut dyn RenderTarget,
        // 借用当前 UI 场景。
        scene: &S,
        // 移交本帧服务与调试输入。
        input: FrameRenderInput<'_>,
        // 保存当前场景代际。
        tree_version: u64,
        // refresh 强制使用完整逻辑区域。
        region: DirtyRegion,
        // 保存唯一最终 present damage。
        damage: DamageRegion,
        // 接收 UI 已解析的唯一 effect。
        effect: Option<crate::draw::OverlayBackdropEffect>,
    ) -> FrameRenderOutput {
        // 正常树必须从全幅 clear 开始，不能把旧 overlay 像素带入 clean source。
        if let Err(error) = self.recorder.begin_recording(true) {
            // 录制失败保持 typed frame failure。
            return Self::failed_backdrop_frame(error, tree_version);
        }
        // 第一阶段只绘制正常根树，overlay 留到 snapshot/blur 之后。
        if let Err(error) = self.layer_tree.render_content(
            // 使用 API-neutral recorder 形成可中间提交的 FrameEncoder。
            &mut self.recorder,
            // 读取同一场景。
            scene,
            // 完整重建 normal retained 内容。
            &DirtyRegion::full(),
            // 传递字体句柄。
            input.font,
            // 传递字体服务。
            input.font_service,
            // 传递图片服务。
            input.image_service,
            // 调试图元只允许在最终提交前的唯一顶层 Pass 绘制。
            false,
            // 内容 Pass 不读取悬停状态。
            None,
            // 正常树同步进入渲染对象索引。
            Some(&mut self.render_object_tree),
        ) {
            // 绘制失败时保留全部场景 invalidation。
            self.layer_tree.invalidate();
            // 返回统一 typed failure。
            return Self::failed_backdrop_frame(error, tree_version);
        }
        // 结束正常树录制并取得无平台身份的有序命令流。
        let normal_frame = match self.recorder.finish_recording() {
            // 成功时移交只读 encoder。
            Ok(encoder) => encoder,
            // 录制结束失败不得进入中间提交。
            Err(error) => {
                // 保持 Picture 与 overlay 缓存为 dirty。
                self.layer_tree.invalidate();
                // 返回统一 typed failure。
                return Self::failed_backdrop_frame(error, tree_version);
            }
        };
        // CPU 光栅分段等非原生载荷只表示 retained backdrop 优化无法无损执行。
        if normal_frame.validate_gpu_native().is_err() {
            // 验证只读结束后即可回收命令容量，降级最终帧会立即复用。
            self.recorder.recycle_frame_encoder(normal_frame);
            // 记录当前 overlay 生命周期的能力事实，避免每帧重复录制同一个非原生树。
            self.overlay_backdrop_blocked = true;
            // 丢弃可能来自上一代正常树的快照，禁止复用过期像素。
            if let Err(error) = engine.release_overlay_backdrop() {
                // 真实资源释放失败仍保留 typed recovery 语义。
                return Self::failed_backdrop_frame(error, tree_version);
            }
            // 强制完整树重绘，确保前置录制没有消费场景 dirty 事实。
            self.layer_tree.invalidate();
            // 降级分支仍需要建立唯一最终帧，直绘函数本身不拥有 begin 边界。
            match engine.begin_frame(UpdateStrategy::FullRedraw) {
                // 只有可绘制帧允许继续直绘。
                RenderOutcome::FrameReady(_) => {}
                // begin 不得越过 paint/end 直接报告呈现。
                RenderOutcome::Present(_) | RenderOutcome::PresentPending(_) => {
                    // 保留场景 dirty 并返回稳定状态错误。
                    return Self::failed_backdrop_frame(
                        // 构造 begin 合同违反诊断。
                        Error::new(
                            // 该结果表示当前帧生命周期非法。
                            Errc::InvalidState,
                            // 说明降级阶段发现的边界。
                            "overlay backdrop fallback begin reported presentation",
                        ),
                        // 保留当前场景代际。
                        tree_version,
                    );
                }
                // 暂无可绘制 surface 时不消费 invalidation。
                RenderOutcome::Idle => {
                    // 返回无副作用的 idle 帧。
                    return FrameRenderOutput {
                        // 保留 target 的 idle 事实。
                        outcome: RenderOutcome::Idle,
                        // 未呈现时禁止消费 dirty。
                        inv_source: InvalidationSource::None,
                        // 保留当前树代际。
                        tree_version,
                    };
                }
                // 真实 begin 失败保持原始图形分类。
                RenderOutcome::Failed(error) => {
                    // 返回不可提交的失败帧。
                    return FrameRenderOutput {
                        // 不重写底层 GraphicsFailure。
                        outcome: RenderOutcome::Failed(error),
                        // 失败帧不消费 invalidation。
                        inv_source: InvalidationSource::None,
                        // 保留当前树代际。
                        tree_version,
                    };
                }
            }
            // 在同一最终 present 内直接由 GPU canvas 绘制正常树与浮层。
            return self.render_gpu_native(
                // 复用唯一 retained GPU target。
                engine,
                // 复用同一场景代际。
                scene,
                // 传递本帧服务与调试输入。
                input,
                // 保留当前树代际。
                tree_version,
                // refresh 原本已要求完整树区域。
                region,
                // 降级帧保留调用方计算的完整 damage。
                Some(damage),
                // 无有效 clean snapshot 时禁止恢复 backdrop。
                false,
                // refresh 使用完整区域，不借用滚动绘制 scratch。
                false,
            );
        }
        // 第一阶段只写 retained texture，不触发 acquire 或 present。
        let execute_result = engine.try_execute_encoded_frame(&normal_frame);
        // 中间提交为同步借用；snapshot 阶段不再需要读取该命令流。
        self.recorder.recycle_frame_encoder(normal_frame);
        match execute_result {
            // 完整 normal tree 已成为新的 clean retained 内容。
            Ok(EncodedFrameExecution::Executed) => {}
            // GPU refresh 不允许绕回私有 adapter 或不完整执行。
            Ok(EncodedFrameExecution::Unsupported) => {
                // 未覆盖 lowering 时保持场景 dirty。
                self.layer_tree.invalidate();
                // 使用稳定错误分类阻止错误帧提交。
                return Self::failed_backdrop_frame(
                    // 构造公开契约缺口诊断。
                    Error::new(
                        // 当前 target 无法执行所需中间 FrameEncoder。
                        Errc::NotImplemented,
                        // 说明失败发生在 backdrop refresh 正常树阶段。
                        "overlay backdrop refresh requires retained FrameEncoder execution",
                    ),
                    // 保留当前场景代际。
                    tree_version,
                );
            }
            // 资源、设备或提交失败保持原始 typed error。
            Err(error) => {
                // 保持场景 dirty 供 recovery 后重试。
                self.layer_tree.invalidate();
                // 中止当前帧。
                return Self::failed_backdrop_frame(error, tree_version);
            }
        }
        // 从刚提交的正常树 retained texture 重建独立 clean snapshot。
        let has_snapshot = match engine.snapshot_overlay_backdrop() {
            // true 表示可以派生 effect 并在后续帧复用。
            Ok(has_snapshot) => has_snapshot,
            // 快照资源事务失败进入统一恢复。
            Err(error) => return Self::failed_backdrop_frame(error, tree_version),
        };
        // 只有真实能力与有效请求同时存在时派生 effect texture。
        if has_snapshot && engine.supports_backdrop_blur() {
            // 默认关闭时保持 clean mask-only 路径。
            if let Some(effect) = effect {
                // 区域与半径已经由 UI typed contract 校验。
                match engine.blur_overlay_backdrop(effect.region(), effect.radius()) {
                    // 执行成功后 restore 会优先使用 effect texture。
                    Ok(true) => {}
                    // 能力在事务间失效时显式降级为 clean mask-only。
                    Ok(false) => {}
                    // 真实 blur failure 不能继续绘制或 present。
                    Err(error) => return Self::failed_backdrop_frame(error, tree_version),
                }
            }
        }
        // blur 完成后才开始本事务唯一的最终帧，避免活动帧阻塞中间提交。
        let begin_damage = match engine.begin_frame(UpdateStrategy::FullRedraw) {
            // 保存 backend 实际接受的 damage。
            RenderOutcome::FrameReady(damage) => damage,
            // 最终帧开始不得直接报告已经呈现。
            RenderOutcome::Present(_) | RenderOutcome::PresentPending(_) => {
                // 保持场景 dirty 并返回稳定状态错误。
                return Self::failed_backdrop_frame(
                    // begin 只能返回可绘制帧。
                    Error::new(
                        // 违反 RenderTarget begin 契约。
                        Errc::InvalidState,
                        // 提供 refresh 专用诊断。
                        "overlay backdrop final begin reported presentation",
                    ),
                    // 保留当前场景代际。
                    tree_version,
                );
            }
            // 暂无可绘制帧时保留全部 invalidation。
            RenderOutcome::Idle => {
                // 不执行 restore、overlay 或 end。
                return FrameRenderOutput {
                    // 保持 target 的 idle 事实。
                    outcome: RenderOutcome::Idle,
                    // 未呈现不得消费 dirty。
                    inv_source: InvalidationSource::None,
                    // 回传当前场景代际。
                    tree_version,
                };
            }
            // begin 的 typed 图形失败直接进入恢复。
            RenderOutcome::Failed(error) => {
                // 不执行任何最终帧动作。
                return FrameRenderOutput {
                    // 保留原始 GraphicsFailure 分类。
                    outcome: RenderOutcome::Failed(error),
                    // 未呈现不得消费 dirty。
                    inv_source: InvalidationSource::None,
                    // 回传当前场景代际。
                    tree_version,
                };
            }
        };
        // begin 内发生恢复切换时不能把 GPU snapshot 写入新软件 target。
        if engine.raster_pipeline() != RasterPipeline::GpuNative {
            // 新 pipeline 不拥有旧 GPU backdrop，执行检查式释放。
            if let Err(error) = engine.release_overlay_backdrop() {
                // 资源失败保留 typed owner 诊断。
                return Self::failed_backdrop_frame(error, tree_version);
            }
            // 强制下次按新 pipeline 重建完整场景。
            self.raster_pipeline = None;
            // 当前已开始帧不能安全继续。
            return Self::failed_backdrop_frame(
                // 报告 begin 期间的 pipeline 代际变化。
                Error::new(
                    // 这是明确的生命周期状态变化。
                    Errc::InvalidState,
                    // 提供恢复层可诊断文本。
                    "overlay backdrop final begin changed the raster pipeline",
                ),
                // 保留当前场景代际。
                tree_version,
            );
        }
        // snapshot 存在时恢复 effect/clean，并取消 begin_frame 的全幅清理计划。
        if has_snapshot {
            // restore 只 submit retained copy，不 present。
            match engine.restore_overlay_backdrop() {
                // 恢复成功后只需绘制 overlay。
                Ok(true) => {}
                // 代际在事务间变化时保留刚提交的 normal retained 内容并 mask-only 绘制。
                Ok(false) => {
                    // 失效 owner 必须检查式释放。
                    if let Err(error) = engine.release_overlay_backdrop() {
                        // 清理失败同样中止当前帧。
                        return Self::failed_backdrop_frame(error, tree_version);
                    }
                }
                // copy/submit failure 保持 typed recovery。
                Err(error) => return Self::failed_backdrop_frame(error, tree_version),
            }
        }
        // 第二阶段只重放 root-level overlays，不覆盖新 normal retained 内容。
        if let Err(error) = self.layer_tree.render_overlays(
            // 直接写入 live GPU canvas，最终由唯一 end_frame flush/present。
            engine,
            // 读取同一场景。
            scene,
            // overlay 必须完整重放，避免 restore 后丢失未损伤区域的 mask。
            &DirtyRegion::full(),
            // 传递字体句柄。
            input.font,
            // 传递字体服务。
            input.font_service,
            // 传递图片服务。
            input.image_service,
            // 调试图元只允许在最终提交前的唯一顶层 Pass 绘制。
            false,
            // overlay 内容 Pass 不读取悬停状态。
            None,
            // overlay 同步进入渲染对象索引。
            Some(&mut self.render_object_tree),
        ) {
            // overlay 绘制失败时保留场景 dirty。
            self.layer_tree.invalidate();
            // 禁止提交不完整帧。
            return Self::failed_backdrop_frame(error, tree_version);
        }
        // refresh 成功后允许后续帧复用新快照。
        self.overlay_backdrop_blocked = !has_snapshot;
        // 调试遥测只在完整场景之后绘制。
        if input.debug_mode {
            // 保持与普通 GPU 路径相同的遥测边界。
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
        // 只有此处执行本事务唯一的最终 present。
        let final_damage = if begin_damage.full {
            // backend 提升为 full 时必须提交完整表面。
            DamageRegion::full()
        } else {
            // 保留调用方计算的 refresh damage。
            damage
        };
        // 使用 begin 后解析的实际 damage 结束唯一最终帧。
        let end_outcome = engine.end_frame(&final_damage);
        // 读取 begin_frame 后可能变化的 live capabilities。
        let caps = engine.capabilities();
        // 复用普通 GPU 路径的最终结果归一化。
        let outcome = normalize_end_outcome(end_outcome, caps, final_damage);
        // 只有真实 present 才消费 full refresh invalidation。
        let inv_source = match outcome {
            // 成功提交后按完整区域分类。
            RenderOutcome::Present(_) | RenderOutcome::PresentPending(_) => {
                // rendered_first 为 true，但使用实际完整 refresh region。
                classify_invalidation(input.rendered_first, &region, input.invalidation_source)
            }
            // 失败或未提交时保留全部 dirty。
            RenderOutcome::Idle | RenderOutcome::FrameReady(_) | RenderOutcome::Failed(_) => {
                // 不消费任何 invalidation。
                InvalidationSource::None
            }
        };
        // 返回与普通场景管线相同的帧结果。
        FrameRenderOutput {
            // 保留归一化提交结果。
            outcome,
            // 回传 invalidation 来源。
            inv_source,
            // 回传当前场景代际。
            tree_version,
        }
    }
}
