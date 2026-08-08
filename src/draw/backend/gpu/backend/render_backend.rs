//! [`GpuBackend`] 的 `RenderBackend` 实现 — backend 子模块。
//!
//! offscreen 生命周期、编码 Picture/Frame 执行、叠加层 backdrop 快照。

use std::any::Any;

use crate::core::{DamageRegion, Errc, Error, PresentDamage, Rect};
use crate::draw::backend::contract::{
    BackendCapabilities, BackendKind, DrawSurface, RenderBackend,
};
use crate::draw::geometry::types::{BlendMode, ImageHandle};
use crate::draw::painting::{
    EncodedFrameExecution, EncodedPictureExecution, FrameCommand, FrameEncoder,
};
use crate::draw::Canvas2D;
use crate::native::present::PresentTestResult;
// 引入迁移期 RHI 的离屏纹理描述。
use crate::native::present::rhi::{
    LoadAction, RenderTargetHandle, RhiColor, RhiExtent, RhiViewport, TextureDesc, TextureFormat,
};

use super::super::canvas::NativeGpuCanvas2D;
// 复用通用 soft staging helper，保证主 surface 与 Picture 使用同一采样契约。
use super::rhi_surface_soft::try_upload_rhi_canvas_soft;
use super::{GpuBackend, NativeGpuOffscreen};

// 迁移期的兼容回退仍可能直接清空并重绘 swapchain，因此不能向场景层承诺局部重绘。
fn migration_safe_gpu_capabilities(offscreen_targets: bool) -> BackendCapabilities {
    // 先采用不会依赖 swapchain 内容保留的完整重绘能力。
    let mut capabilities = BackendCapabilities::gpu_full_redraw();
    // 离屏 target 是独立事实能力，不应因主 surface 的保守策略而被关闭。
    capabilities.offscreen = offscreen_targets;
    // 返回供场景管线消费的迁移期能力快照。
    capabilities
}

impl RenderBackend for GpuBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Gpu
    }

    fn capabilities(&self) -> BackendCapabilities {
        // 只有所有主 surface 路径都保持 retained 内容后，才可重新暴露 partial redraw。
        migration_safe_gpu_capabilities(self.surface.native_caps.offscreen_targets)
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        let logical_w = width.max(1);
        let logical_h = height.max(1);
        // swapchain resize 会推进 surface generation，先释放旧代际 retained texture。
        self.destroy_rhi_surface_texture()?;
        // 生产 GPU adapter 优先由薄 RHI surface 执行实际重建和代际推进。
        match self.gpu_ctx.resize_rhi_surface(logical_w, logical_h) {
            // RHI resize 成功后不再穿过逐 UI 兼容生命周期。
            Ok(()) => {}
            // 尚未接入 RHI 的旧 adapter 才保留兼容 resize 回退。
            Err(error) if error.code() == Errc::NotImplemented => {
                // 兼容回退只处理明确的迁移期能力缺口。
                self.gpu_ctx.resize(logical_w, logical_h)?;
            }
            // surface lost、device lost、参数错误等真实失败不能被回退吞掉。
            Err(error) => return Err(error),
        }
        // D3D11/D3D12 等会按 HWND GetClientRect 校正缓冲尺寸；canvas/布局必须跟
        // 实际 RT 一致，否则清出更大黑底而 UI 仍画旧几何 → 窗口黑边。
        self.factory_prepared = true;
        self.adopt_factory_drawable_extent();
        Ok(())
    }

    fn initialize_prepared(&mut self, width: i32, height: i32) -> Result<(i32, i32), Error> {
        // `IGraphicsContext::initialize` already ran in the factory against
        // the real surface. Startup only synchronizes draw-owned state to the
        // factory-reported drawable; it must not recreate the swapchain.
        if !self.factory_prepared {
            self.gpu_ctx.resize(width.max(1), height.max(1))?;
            self.factory_prepared = true;
        }
        Ok(self.adopt_factory_drawable_extent())
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        if self.shutdown {
            return Ok(());
        }
        self.destroy_all_offscreens()?;
        // 在 owner-thread context 关闭前释放主 surface 的 retained texture。
        self.destroy_rhi_surface_texture()?;
        // 在 owner-thread context 关闭前释放 RHI renderer 持有的跨帧 MSDF atlas pages。
        if let Some(renderer) = self.rhi_renderer.as_mut() {
            // 只有暴露薄 RHI 的 native context 才有对应资源表可释放。
            if let Some(context) = self.gpu_ctx.rhi_context() {
                // 失败时保留 typed error，禁止在资源仍存活时伪造 shutdown 成功。
                renderer.release_msdf_atlas(context)?;
            }
        }
        self.gpu_ctx.try_shutdown()?;
        self.shutdown = true;
        Ok(())
    }

    fn surface(&mut self) -> &mut dyn DrawSurface {
        &mut self.surface
    }

    fn make_current(&mut self) -> Result<(), Error> {
        self.gpu_ctx.make_current()
    }

    fn device_pixel_ratio(&self) -> f32 {
        self.gpu_ctx.device_pixel_ratio()
    }

    fn create_offscreen(&mut self, width: i32, height: i32) -> Option<ImageHandle> {
        if !self.surface.native_caps.offscreen_targets || width <= 0 || height <= 0 {
            return None;
        }
        let target = self
            .gpu_ctx
            .create_offscreen_target(width, height)
            .inspect_err(|err| {
                tracing::warn!(
                    "GpuBackend: create_offscreen_target failed: {}",
                    err.short_what()
                );
            })
            .ok()?;
        // 在暴露 RHI 的 adapter 上同步创建可渲染、可采样的离屏纹理。
        let rhi_texture = match self.gpu_ctx.rhi_context() {
            // 让 RHI adapter 自己负责纹理和 RTV/SRV 的具体资源创建。
            Some(context) => match context.create_texture(TextureDesc {
                extent: RhiExtent::new(width as u32, height as u32),
                format: TextureFormat::Bgra8Unorm,
            }) {
                // 保存通用 texture 句柄，后续 FramePlan 以同一身份作为 target 和 source。
                Ok(texture) => Some(texture),
                // RHI 离屏资源失败时保留旧 target，确保兼容路径仍可工作。
                Err(error) => {
                    tracing::warn!(
                        "GpuBackend: create RHI offscreen texture failed: {}",
                        error.short_what()
                    );
                    None
                }
            },
            // 未暴露 RHI 的 adapter 继续使用 legacy offscreen target。
            None => None,
        };
        let id = if let Some(id) = self.free_offscreen_ids.pop() {
            id
        } else {
            let id = self.next_offscreen_id;
            self.next_offscreen_id = self.next_offscreen_id.saturating_add(1);
            id
        };
        let idx = id as usize;
        while self.offscreens.len() <= idx {
            self.offscreens.push(None);
        }
        self.offscreens[idx] = Some(NativeGpuOffscreen {
            target,
            rhi_texture,
            canvas: if self.gpu_only {
                NativeGpuCanvas2D::new_gpu_only(width, height, self.surface.native_caps)
            } else {
                NativeGpuCanvas2D::new(width, height, self.surface.native_caps)
            },
            width,
            height,
        });
        Some(ImageHandle(id))
    }

    fn try_destroy_offscreen(&mut self, handle: ImageHandle) -> Result<(), Error> {
        let idx = handle.0 as usize;
        let Some(Some(off)) = self.offscreens.get(idx) else {
            return Ok(());
        };
        // 复制 legacy target 身份，释放对 slot 的借用再进入 owner-thread 调用。
        let target = off.target;
        if self.active_offscreen == Some(handle.0) {
            self.gpu_ctx.bind_swapchain_target()?;
        }
        // 先销毁可选 RHI 纹理，避免 legacy destroy 后留下悬挂句柄。
        let rhi_texture = self
            .offscreens
            .get(idx)
            .and_then(Option::as_ref)
            .and_then(|off| off.rhi_texture);
        if let Some(texture) = rhi_texture {
            // 具有 RHI 纹理的 slot 必须仍由同一 owner-thread context 管理。
            let Some(context) = self.gpu_ctx.rhi_context() else {
                // 不在无法回收 RHI 资源时静默释放 slot。
                return Err(Error::new(
                    Errc::InvalidState,
                    "RHI offscreen texture lost its owner context before destroy",
                ));
            };
            // 检查式释放 RHI 纹理。
            context.destroy_texture(texture)?;
            // 只有释放成功后才从 backend slot 清除句柄。
            if let Some(Some(off)) = self.offscreens.get_mut(idx) {
                off.rhi_texture = None;
            }
        }
        // 兼容 target 仍需经过原有 checked destruction boundary。
        self.gpu_ctx.try_destroy_offscreen_target(target)?;
        self.offscreens[idx] = None;
        if self.active_offscreen == Some(handle.0) {
            self.active_offscreen = None;
            self.offscreen_flush_committed = false;
        }
        self.free_offscreen_ids.push(handle.0);
        self.compact_offscreen_slots();
        Ok(())
    }

    fn destroy_offscreen(&mut self, handle: ImageHandle) {
        if let Err(error) = self.try_destroy_offscreen(handle) {
            self.remember_frame_failure(error);
        }
    }

    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        let idx = handle.0 as usize;
        self.offscreens
            .get_mut(idx)?
            .as_mut()
            .map(|o| &mut o.canvas as &mut dyn Canvas2D)
    }

    fn try_execute_encoded_picture(
        &mut self,
        handle: &ImageHandle,
        encoder: &FrameEncoder,
    ) -> Result<EncodedPictureExecution, Error> {
        if self.active_offscreen != Some(handle.0) {
            return Err(Error::new(
                Errc::InvalidState,
                "FrameEncoder Picture execution requires its bound offscreen target",
            ));
        }
        let target = self
            .offscreens
            .get(handle.0 as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    "Picture offscreen target disappeared before FrameEncoder execution",
                )
            })?;
        if (target.width, target.height) != (encoder.width(), encoder.height()) {
            return Err(Error::new(
                Errc::InvalidState,
                format!(
                    "FrameEncoder {}x{} does not match Picture target {}x{}",
                    encoder.width(),
                    encoder.height(),
                    target.width,
                    target.height
                ),
            ));
        }
        let target_id = target.target;
        let rhi_texture = target.rhi_texture;

        self.gpu_ctx.bind_offscreen_target(target_id)?;
        // 已有 RHI texture 时优先把整条 Picture encoder 写入同一 texture target。
        if let Some(texture) = rhi_texture {
            // 新 Picture 首次执行必须透明初始化，后续片段保留已有内容。
            let load = if self.offscreen_rhi_initialized {
                LoadAction::Load
            } else {
                LoadAction::Clear(RhiColor([0.0, 0.0, 0.0, 0.0]))
            };
            let rhi_target = RenderTargetHandle::from_raw(texture.raw());
            if self.try_execute_frame_encoder_rhi(
                encoder,
                crate::draw::backend::frame_plan::RenderTargetRef::Texture(rhi_target),
                load,
                false,
            )? {
                // 只有整条 encoder RHI 提交成功才消费 Picture staging 状态。
                if let Some(Some(offscreen)) = self.offscreens.get_mut(handle.0 as usize) {
                    offscreen.canvas.commit_presented_frame();
                }
                self.offscreen_rhi_initialized = true;
                return Ok(EncodedPictureExecution::Executed);
            }
        }
        // RHI 不覆盖时保留既有整条兼容执行器，不混合两种 lowering。
        if rhi_texture.is_some() {
            // 已经提交过 RHI 内容的 Picture 不能在同一资源上切换到 legacy。
            if self.offscreen_rhi_initialized {
                return Err(Error::new(
                    Errc::NotImplemented,
                    "Picture RHI target cannot switch to legacy rendering after commit",
                ));
            }
            // 首次 lowering 失败后销毁 RHI texture，避免后续 blit 读取空资源。
            self.downgrade_offscreen_rhi_texture(*handle)?;
        }
        self.execute_frame_encoder(encoder, false)?;
        // legacy execution 已经初始化同一 Picture 的唯一提交目标。
        self.offscreen_rhi_initialized = true;
        Ok(EncodedPictureExecution::Executed)
    }

    fn try_execute_encoded_frame(
        &mut self,
        encoder: &FrameEncoder,
    ) -> Result<EncodedFrameExecution, Error> {
        if self.active_offscreen.is_some() {
            return Err(Error::new(
                Errc::InvalidState,
                "main FrameEncoder execution cannot run while a Picture target is bound",
            ));
        }
        if (self.width, self.height) != (encoder.width(), encoder.height()) {
            return Err(Error::new(
                Errc::InvalidState,
                format!(
                    "FrameEncoder {}x{} does not match main native target {}x{}",
                    encoder.width(),
                    encoder.height(),
                    self.width,
                    self.height
                ),
            ));
        }

        let execute = (|| {
            self.gpu_ctx.make_current()?;
            self.gpu_ctx.bind_swapchain_target()?;
            // 首条 Clear 可完整替代 begin_frame 的 pending damage clear。
            let starts_with_clear =
                matches!(encoder.commands().first(), Some(FrameCommand::Clear { .. }));
            let rhi_load = if starts_with_clear {
                // RHI lowering 会从 encoder 的首条 Clear 读取真实颜色。
                Some(LoadAction::Load)
            } else if !self.surface.pending_clear_rects.is_empty() {
                // 当前 FramePlan 尚未表达局部 retained clear，交回兼容路径。
                None
            } else if self.surface.needs_gpu_clear {
                // 无显式 Clear 时沿用 begin_frame 的透明初始化。
                Some(LoadAction::Clear(RhiColor([0.0, 0.0, 0.0, 0.0])))
            } else {
                // 保留 retained surface 的前序像素。
                Some(LoadAction::Load)
            };
            if let Some(load) = rhi_load {
                if self.try_execute_frame_encoder_rhi(
                    encoder,
                    crate::draw::backend::frame_plan::RenderTargetRef::Surface,
                    load,
                    false,
                )? {
                    // RHI 片段已替代当前 encoder，外层仍负责最终 present。
                    self.surface.needs_gpu_clear = false;
                    self.surface.pending_clear_rects.clear();
                    return Ok(());
                }
            }
            // RHI lowering 若回退，先丢弃可能已创建但未提交的 retained target。
            self.abandon_rhi_surface_texture_for_legacy()?;
            let target_initialized = self.prepare_main_frame_encoder_target(encoder)?;
            self.execute_frame_encoder(encoder, target_initialized)
        })();
        if let Err(error) = execute {
            self.surface.needs_gpu_clear = true;
            return Err(error);
        }

        // `begin_frame` may have prepared a clear or retained Canvas2D state.
        // The FrameEncoder has replaced the target, so final `present` must
        // not submit a second clear/draw sequence over it.
        self.surface.needs_gpu_clear = false;
        self.surface.pending_clear_rects.clear();
        self.surface.canvas.commit_presented_frame();
        Ok(EncodedFrameExecution::Executed)
    }

    fn begin_offscreen_paint(&mut self, handle: &ImageHandle) -> bool {
        match self.try_begin_offscreen_paint(handle) {
            Ok(()) => true,
            Err(error) => {
                self.remember_frame_failure(error);
                false
            }
        }
    }

    fn try_begin_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        self.offscreen_flush_committed = false;
        let idx = handle.0 as usize;
        let Some(Some(off)) = self.offscreens.get(idx) else {
            return Err(Error::new(
                Errc::InvalidState,
                "Picture offscreen target does not exist",
            ));
        };
        let target = off.target;
        let rhi_texture = off.rhi_texture;
        // RHI 离屏由 flush 时的 FramePlan Clear 初始化，不提前绑定 legacy target。
        if rhi_texture.is_none() {
            // 没有 RHI 资源时沿用 legacy target 的立即清理语义。
            self.gpu_ctx.bind_offscreen_target(target)?;
            if let Err(error) = self.gpu_ctx.clear_render_target(0.0, 0.0, 0.0, 0.0) {
                return match self.gpu_ctx.bind_swapchain_target() {
                    Ok(()) => Err(error),
                    Err(restore_error) => Err(restore_error.with_source(error)),
                };
            }
        }
        if let Some(Some(off)) = self.offscreens.get_mut(idx) {
            off.canvas.reset_for_repaint();
        }
        self.active_offscreen = Some(handle.0);
        Ok(())
    }

    fn flush_offscreen_paint(&mut self, handle: &ImageHandle) {
        if let Err(error) = self.try_flush_offscreen_paint(handle) {
            // The legacy void entry remains for old callers.  The production
            // compositor uses `try_*` and therefore returns this failure
            // before a final present can be reported as success.
            self.remember_frame_failure(error);
        }
    }

    fn try_flush_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        if self.active_offscreen == Some(handle.0) {
            self.offscreen_flush_committed = false;
        }
        let idx = handle.0 as usize;
        let off = self
            .offscreens
            .get_mut(idx)
            .and_then(Option::as_mut)
            .ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    "offscreen target disappeared before flush",
                )
            })?;
        if let Some(error) = off.canvas.take_deferred_error() {
            return Err(error);
        }
        let target = off.target;
        let rhi_texture = off.rhi_texture;
        // 记录本次 flush 是否仍处于 RHI target 的首个提交边界。
        let rhi_was_uninitialized = !self.offscreen_rhi_initialized;
        // 先尝试把当前 Picture queue 作为离屏 FramePlan 提交，不触发主 surface present。
        let rhi_submitted = if let Some(texture) = rhi_texture {
            // 首次提交清理新纹理，后续有序 flush 保留已有离屏内容。
            let load = if self.offscreen_rhi_initialized {
                // 保留 Picture target 已经绘制的前序内容。
                LoadAction::Load
            } else {
                // 以透明色初始化 Picture target。
                LoadAction::Clear(RhiColor([0.0, 0.0, 0.0, 0.0]))
            };
            // 缺少 renderer/context 时交回 legacy target，而不是伪造 RHI 成功。
            if let Some(renderer) = self.rhi_renderer.as_mut() {
                if let Some(context) = self.gpu_ctx.rhi_context() {
                    // 使用离屏 texture 的同一不透明身份作为 render target。
                    let rhi_target = RenderTargetHandle::from_raw(texture.raw());
                    // 离屏 target 使用自身物理尺寸，不借用主窗口 drawable 的 DPR。
                    let viewport = RhiViewport {
                        width: off.width.max(1) as f32,
                        height: off.height.max(1) as f32,
                    };
                    // 记录当前 Picture 是否同时含有 native 与 soft staging。
                    let has_native = !off.canvas.pending_native.is_empty();
                    // 记录当前 Picture 是否有尚未合成的 CPU soft 内容。
                    let has_soft = off.canvas.soft_has_content;
                    // native 前缀存在时只提交已验证的 queue，保留后续 soft 分段合成机会。
                    let native_submitted = if has_native {
                        off.canvas.submit_rhi_mixed_for_geometry(
                            renderer,
                            context,
                            load,
                            crate::draw::backend::frame_plan::RenderTargetRef::Texture(rhi_target),
                            PresentDamage::Full,
                            viewport,
                            1.0,
                            1.0,
                            has_soft,
                        )?
                    } else {
                        true
                    };
                    // native queue 未完全 lowering 时不能继续消费 soft staging。
                    if has_native && !native_submitted {
                        if self.offscreen_rhi_initialized {
                            return Err(Error::new(
                                Errc::NotImplemented,
                                "Picture RHI target cannot switch to legacy rendering after commit",
                            ));
                        }
                        false
                    } else {
                        // native 成功后 soft 段必须以 Load 继续写入同一离屏 target。
                        let soft_submitted = if has_soft || !has_native {
                            try_upload_rhi_canvas_soft(
                                &mut off.canvas,
                                off.width,
                                off.height,
                                viewport,
                                1.0,
                                1.0,
                                rhi_target,
                                if has_native { LoadAction::Load } else { load },
                                context,
                                renderer,
                            )?
                        } else {
                            true
                        };
                        // 只有 native 与全部 soft blend 段都成功才消费 Picture staging。
                        native_submitted && soft_submitted
                    }
                } else {
                    // 当前 context 未暴露 RHI，使用 legacy target。
                    false
                }
            } else {
                // 当前 backend 没有 renderer cache，使用 legacy target。
                false
            }
        } else {
            // 未暴露 RHI texture 的 adapter 直接使用兼容 target。
            false
        };
        if rhi_submitted {
            // RHI 离屏 submit 成功后才提交 canvas staging 状态。
            off.canvas.commit_presented_frame();
            // 标记 target 已经有可被后续 Load pass 保留的内容。
            self.offscreen_rhi_initialized = true;
        } else {
            // 首次 RHI lowering 失败后永久收敛到 legacy target，避免双写资源失去同步。
            if rhi_was_uninitialized && rhi_texture.is_some() {
                self.downgrade_offscreen_rhi_texture(*handle)?;
            }
            // RHI 未覆盖当前队列时，回到原生 target 的完整兼容提交。
            self.gpu_ctx.bind_offscreen_target(target)?;
            // RHI target 首次回退时需要显式清理 legacy target。
            if rhi_was_uninitialized && rhi_texture.is_some() {
                self.gpu_ctx.clear_render_target(0.0, 0.0, 0.0, 0.0)?;
            }
            // 重新取得 slot，继续在 legacy target 上提交完整队列。
            let off = self
                .offscreens
                .get_mut(idx)
                .and_then(Option::as_mut)
                .ok_or_else(|| {
                    Error::new(
                        Errc::InvalidState,
                        "offscreen target disappeared during legacy fallback",
                    )
                })?;
            // 先提交 native queue，再提交 soft queue，保持既有 painter order。
            off.canvas.submit_native(self.gpu_ctx.as_mut())?;
            off.canvas.submit_soft(self.gpu_ctx.as_mut())?;
            // 兼容 flush 成功后同样视为 target 已初始化。
            off.canvas.commit_presented_frame();
            self.offscreen_rhi_initialized = true;
        }
        if self.active_offscreen == Some(handle.0) {
            self.offscreen_flush_committed = true;
        }
        Ok(())
    }

    fn end_offscreen_paint(&mut self) {
        if let Err(error) = self.try_end_offscreen_paint() {
            self.remember_frame_failure(error);
        }
    }

    fn try_end_offscreen_paint(&mut self) -> Result<(), Error> {
        let active = self.active_offscreen.take();
        let restore = self.gpu_ctx.bind_swapchain_target();
        if restore.is_ok() && self.offscreen_flush_committed {
            if let Some(offscreen) = active
                .and_then(|id| self.offscreens.get_mut(id as usize))
                .and_then(Option::as_mut)
            {
                offscreen.canvas.release_committed_picture_staging();
            }
        }
        self.offscreen_flush_committed = false;
        self.offscreen_rhi_initialized = false;
        restore
    }

    fn blit_offscreen(&mut self, handle: &ImageHandle, dst_rect: Rect) {
        let Some(Some(off)) = self.offscreens.get(handle.0 as usize) else {
            return;
        };
        let src = Rect::new(0.0, 0.0, off.width as f32, off.height as f32);
        self.blit_offscreen_src(handle, src, dst_rect);
    }

    fn blit_offscreen_src(&mut self, handle: &ImageHandle, src_rect: Rect, dst_rect: Rect) {
        if let Err(error) = self.try_blit_offscreen_src(handle, src_rect, dst_rect) {
            self.remember_frame_failure(error);
        }
    }

    fn try_blit_offscreen_src(
        &mut self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
    ) -> Result<(), Error> {
        let idx = handle.0 as usize;
        let Some(Some(off)) = self.offscreens.get(idx) else {
            return Err(Error::new(
                Errc::InvalidState,
                "Picture offscreen target does not exist before blit",
            ));
        };
        let target = off.target;
        let source_rhi_texture = off.rhi_texture;
        let source_width = off.width;
        let source_height = off.height;
        let opacity = if let Some(active) = self.active_offscreen {
            self.offscreens
                .get(active as usize)
                .and_then(|slot| slot.as_ref())
                .map(|slot| slot.canvas.opacity())
                .unwrap_or(1.0)
        } else {
            self.surface.canvas.opacity()
        };
        let additive = if let Some(active) = self.active_offscreen {
            self.offscreens
                .get(active as usize)
                .and_then(|slot| slot.as_ref())
                .map(|slot| matches!(slot.canvas.current_blend_mode(), BlendMode::Additive))
                .unwrap_or(false)
        } else {
            matches!(
                self.surface.canvas.current_blend_mode(),
                BlendMode::Additive
            )
        };
        if !opacity.is_finite() || opacity <= 0.0 {
            return Ok(());
        }
        if let Some(active) = self.active_offscreen {
            if active == handle.0 {
                return Err(Error::new(
                    Errc::InvalidArgument,
                    "Picture offscreen target cannot blit into itself",
                ));
            }
            // 在提交目标队列前统一源/目标资源所有权，避免 RHI 与 legacy 双写失同步。
            self.prepare_nested_picture_blit(source_rhi_texture.is_some(), ImageHandle(active))?;
            // The destination is the currently bound Picture target. Submit
            // its queued native/soft commands before the immediate source
            // blit so painter order remains destination commands → blit.
            // Flushing the main swapchain here would clear/submit the wrong
            // target and invert that order.
            self.try_flush_offscreen_paint(&ImageHandle(active))?;
        } else {
            // `blit_offscreen_target` is immediate on native APIs. Flush
            // clear, native work and any bounded CPU segment before it so
            // Picture does not leapfrog preceding painter-order commands.
            // The subsequent commands remain queued and are committed by the
            // same final present.
            if source_rhi_texture.is_some() {
                // native/soft queue 或局部清理都必须先建立 retained painter-order boundary。
                if self.surface.canvas.soft_has_content
                    || !self.surface.canvas.pending_native.is_empty()
                    || !self.surface.pending_clear_rects.is_empty()
                    || !self.surface.pending_scroll_copies.is_empty()
                {
                    // RHI source 不能跟在已经落入 legacy swapchain 的前缀后面。
                    if !self.try_flush_main_segment_rhi()? {
                        // 不支持的组合保持明确失败，不消费尚未验证的 staging。
                        return Err(Error::new(
                            Errc::NotImplemented,
                            "RHI Picture blit requires a fully retained preceding main segment",
                        ));
                    }
                    // RHI source 不能采样已经落到 legacy swapchain 的前置内容。
                    if self.rhi_surface_texture.is_none() {
                        // 将不完整组合报告为未实现，而不是返回错误的像素结果。
                        return Err(Error::new(
                            Errc::NotImplemented,
                            "RHI Picture blit requires the preceding main queue to stay retained",
                        ));
                    }
                }
            } else {
                // legacy source 仍需沿用原有的立即目标 flush 语义。
                self.flush_main_segment_before_ordered_boundary()?;
            }
        }
        // RHI 离屏资源必须通过同一 sampled pipeline 合成，不能交给不认识该句柄的 legacy target。
        if let Some(texture) = source_rhi_texture {
            return self.blit_rhi_offscreen_texture(
                texture,
                source_width,
                source_height,
                src_rect,
                dst_rect,
                opacity.clamp(0.0, 1.0),
                additive,
            );
        }
        // 没有 RHI 纹理时继续走兼容 adapter 的立即 blit。
        self.gpu_ctx.blit_offscreen_target(
            target,
            src_rect,
            dst_rect,
            opacity.clamp(0.0, 1.0),
            additive,
        )
    }

    // 尝试使用已有 RHI target 执行 blur；未覆盖部分仍由兼容路径处理。
    fn try_blur_offscreen(
        &mut self,
        handle: &ImageHandle,
        region: Rect,
        radius: f32,
    ) -> Result<(), Error> {
        if !self.surface.native_caps.offscreen_targets {
            return Err(Error::new(
                Errc::NotImplemented,
                "native GPU backend lacks offscreen targets required for separable blur",
            ));
        }
        if !radius.is_finite() || radius < 0.5 {
            return Ok(());
        }
        let idx = handle.0 as usize;
        let Some(Some(off)) = self.offscreens.get(idx) else {
            return Err(Error::new(
                Errc::InvalidState,
                "Picture offscreen target does not exist before blur",
            ));
        };
        let target = off.target;
        let rhi_texture = off.rhi_texture;
        let offscreen_width = off.width;
        let offscreen_height = off.height;
        // 模糊前必须把挂起的绘制落到纹理，且不能在绑定为目标时采样。
        if self.active_offscreen == Some(handle.0) {
            self.try_flush_offscreen_paint(handle)?;
            self.try_end_offscreen_paint()?;
        } else if self.active_offscreen.is_some() {
            // 另一离屏正绑定：先 flush 当前绑定，避免命令落错目标。
            if let Some(active) = self.active_offscreen {
                self.try_flush_offscreen_paint(&ImageHandle(active))?;
            }
        } else {
            self.flush_main_segment_before_ordered_boundary()?;
        }
        // RHI texture 已经拥有完整 Picture 内容时，执行真正的两段 RHI blur。
        if let Some(texture) = rhi_texture {
            // Picture RHI texture 按自身逻辑 extent 创建，不能重复乘主 surface DPR。
            let rhi_region = super::rhi_surface_blit::lower_picture_blur_region(region);
            // 分开借用 owner-thread context 和通用 renderer cache。
            let (gpu_ctx, rhi_renderer) = (&mut self.gpu_ctx, &mut self.rhi_renderer);
            // RHI texture 不能在缺少 renderer 时静默切回不一致的 legacy target。
            let Some(renderer) = rhi_renderer.as_mut() else {
                // 返回稳定的迁移期未实现错误。
                return Err(Error::new(
                    Errc::NotImplemented,
                    "RHI offscreen blur requires the RHI renderer cache",
                ));
            };
            // 只有组合 RHI context 能执行 texture target 的多阶段计划。
            let Some(context) = gpu_ctx.rhi_context() else {
                // 返回稳定的迁移期未实现错误。
                return Err(Error::new(
                    Errc::NotImplemented,
                    "RHI offscreen blur requires a composable RHI context",
                ));
            };
            // 当前 Picture texture 同时作为 source 和最终 target，scratch 由 renderer 管理。
            return renderer.execute_blur_without_present(
                context,
                PresentDamage::Full,
                texture,
                RhiExtent::new(offscreen_width as u32, offscreen_height as u32),
                rhi_region,
                radius,
                TextureFormat::Bgra8Unorm,
                crate::draw::backend::frame_plan::RenderTargetRef::Texture(
                    RenderTargetHandle::from_raw(texture.raw()),
                ),
            );
        }
        // 没有 RHI texture 的 adapter 继续使用原有 checked blur 边界。
        self.gpu_ctx.blur_offscreen_target(target, region, radius)
    }

    fn snapshot_overlay_backdrop(&mut self) -> bool {
        // 叠加层 backdrop 职责在独立模块实现，保持本文件处于行数上限内。
        self.snapshot_overlay_backdrop_impl()
    }

    fn restore_overlay_backdrop(&mut self) -> bool {
        // 恢复与释放委托给独立 backdrop 模块。
        self.restore_overlay_backdrop_impl()
    }

    fn release_overlay_backdrop(&mut self) {
        // 释放委托给独立 backdrop 模块。
        self.release_overlay_backdrop_impl();
    }

    fn has_overlay_backdrop(&self) -> bool {
        // 查询委托给独立 backdrop 模块。
        self.has_overlay_backdrop_impl()
    }

    fn present(&mut self, damage: &DamageRegion) -> Result<(), Error> {
        self.present_impl(damage)
    }

    fn test_present(&mut self) -> Result<PresentTestResult, Error> {
        self.gpu_ctx.test_present()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    // 引入待验证的迁移期能力推导函数。
    use super::migration_safe_gpu_capabilities;

    #[test]
    // 验证离屏支持不会重新开启不安全的主 surface 局部重绘。
    fn migration_capabilities_keep_partial_redraw_disabled() {
        // 构造支持离屏 target 的生产能力组合。
        let with_offscreen = migration_safe_gpu_capabilities(true);
        // 主 surface 必须保持完整重绘，避免兼容回退只留下 damage 区域。
        assert!(!with_offscreen.partial_redraw);
        // 独立离屏能力仍应透传给 Picture 与效果管线。
        assert!(with_offscreen.offscreen);

        // 构造不支持离屏 target 的生产能力组合。
        let without_offscreen = migration_safe_gpu_capabilities(false);
        // 无离屏能力时同样不得依赖 swapchain 内容保留。
        assert!(!without_offscreen.partial_redraw);
        // 原生 adapter 未声明的离屏能力不得被虚构。
        assert!(!without_offscreen.offscreen);
    }
}
