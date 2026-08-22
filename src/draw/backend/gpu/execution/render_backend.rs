//! [`GpuBackend`] 的 `RenderBackend` 实现 — backend 子模块。
//!
//! offscreen 生命周期、编码 Picture/Frame 执行、叠加层 backdrop 快照。

use std::any::Any;

use crate::core::{DamageRegion, Errc, Error, Rect};
use crate::draw::Canvas2D;
use crate::draw::backend::contract::{
    BackendCapabilities, BackendKind, DrawSurface, RenderBackend,
};
// test-harness 实现只暴露 API 无关的规范 surface 快照。
#[cfg(feature = "test-harness")]
use crate::draw::backend::contract::SurfaceReadback;
use crate::draw::geometry::types::{BlendMode, ImageHandle};
use crate::draw::painting::{
    EncodedFrameExecution, EncodedPictureExecution, FrameCommand, FrameEncoder,
};
use crate::platform::presentation::rhi::PresentTestResult;
// 引入迁移期 RHI 的离屏纹理描述。
use crate::platform::presentation::rhi::{
    LoadAction, RhiColor, RhiExtent, RhiViewport, TextureDesc, TextureFormat,
};

use super::super::canvas::NativeGpuCanvas2D;
// 复用通用 soft staging helper，保证主 surface 与 Picture 使用同一采样契约。
use super::rhi_surface_soft::try_upload_rhi_canvas_soft;
use super::{GpuBackend, NativeGpuOffscreen, device_pixel_ratio_from_surface};

// 从 retained 主颜色目标与 Device 可选原语组装场景层能力。
fn retained_gpu_capabilities(
    has_rhi_offscreen_owner: bool,
    // 接收 Drawing retained target 是否能够跨帧保留未损坏像素。
    supports_retained_redraw: bool,
    // 接收当前 Device Adapter 是否实现共享 TextureMove 原语。
    supports_texture_region_move: bool,
) -> BackendCapabilities {
    // 只有 Drawing 自己能够跨帧保留主颜色目标时才开放局部重绘。
    let mut capabilities = if supports_retained_redraw {
        // retained texture 可以安全消费局部场景 damage。
        BackendCapabilities::gpu()
    } else {
        // 任一证明缺失时保持完整重绘回退。
        BackendCapabilities::gpu_full_redraw()
    };
    // 只有通用 renderer 拥有离屏纹理时才向场景层开放 Picture 能力。
    capabilities.offscreen = has_rhi_offscreen_owner;
    // 滚动 memmove 同时依赖跨帧保留证明与底层重叠移动原语。
    capabilities.scroll_memmove = supports_retained_redraw && supports_texture_region_move;
    // 返回供场景管线消费的迁移期能力快照。
    capabilities
}

// 把逻辑离屏尺寸收敛为只允许 RHI owner 创建的纹理 extent。
fn rhi_offscreen_extent(
    // 指示当前 backend 是否持有通用 RHI renderer。
    has_rhi_offscreen_owner: bool,
    // 接收调用方请求的逻辑宽度。
    width: i32,
    // 接收调用方请求的逻辑高度。
    height: i32,
    // 返回可创建的 RHI extent；不满足单一所有权或尺寸契约时拒绝。
) -> Option<RhiExtent> {
    // 拒绝没有通用 RHI owner 或非正尺寸的离屏请求。
    if !has_rhi_offscreen_owner || width <= 0 || height <= 0 {
        // 不为 legacy adapter 创建第二套资源。
        return None;
    }
    // 安全地把已验证的正 i32 尺寸转换为 RHI 无符号 extent。
    Some(RhiExtent::new(width as u32, height as u32))
}

// 把 Picture queue 的 RHI lowering 结果收敛为稳定的 typed failure。
fn require_lossless_rhi_submission(
    // 指示当前 queue 是否已经完整提交到唯一 RHI texture。
    submitted: bool,
    // 保存具体 lowering 边界的诊断文本。
    message: &'static str,
    // 成功时继续提交状态，失败时阻止任何 adapter 高层回退。
) -> Result<(), Error> {
    // 完整提交后允许调用方消费 staging。
    if submitted {
        // 不引入额外资源状态变更。
        return Ok(());
    }
    // 未完整 lowering 时返回可由恢复层识别的稳定错误。
    Err(Error::new(
        // 当前缺口属于通用 RHI lowering 尚未实现。
        Errc::NotImplemented,
        // 保留调用点提供的具体 RHI lowering 阶段。
        message,
    ))
}

impl RenderBackend for GpuBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Gpu
    }

    fn capabilities(&self) -> BackendCapabilities {
        // Drawing 局部重绘只依赖 retained texture；最终 present damage 由 Surface 独立收敛。
        let supports_retained_redraw = self.rhi_renderer.is_some()
            // 主颜色目标必须在帧间保留未受 damage 影响的像素。
            && self.surface.native_caps.retained_color_target;
        // Picture 与主 surface 能力分别从各自事实投影。
        retained_gpu_capabilities(
            // 通用 renderer 是否拥有离屏资源。
            self.rhi_renderer.is_some(),
            // retained target 的跨帧像素保留证明。
            supports_retained_redraw,
            // 纹理重叠移动必须由选中的 Device Adapter 明确声明。
            self.surface.native_caps.rhi_texture_region_move,
        )
    }

    // 从 retained RHI owner 事实投影真实 backdrop blur 能力。
    fn supports_backdrop_blur(&self) -> bool {
        // 通用 renderer、保留 framebuffer 与纹理复制缺一不可。
        self.rhi_renderer.is_some() && self.surface.native_caps.retained_color_target
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        // 零尺寸由窗口调度器保持暂停；后端不得把暂停伪装成 1x1 原生 Surface。
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        let logical_w = width;
        let logical_h = height;
        // swapchain resize 会推进 surface generation，先释放旧代际 retained texture。
        self.destroy_rhi_surface_texture()?;
        // 生产 GPU adapter 只由薄 RHI surface 执行实际重建和代际推进。
        // 已验证 owner 通过 recipe 专用视图执行唯一 surface resize 事务。
        self.gpu_ctx.resize_surface(logical_w, logical_h)?;
        // 原生 adapter 会按窗口客户区校正缓冲尺寸；canvas/布局必须跟
        // 实际 RT 一致，否则清出更大黑底而 UI 仍画旧几何 → 窗口黑边。
        self.adopt_factory_drawable_extent();
        Ok(())
    }

    // factory 已完成原生 surface 准备，初始化只同步 drawable 元数据。
    fn initialize_prepared(&mut self, _width: i32, _height: i32) -> Result<(i32, i32), Error> {
        // native context 构造成功时已经绑定真实 surface 并进入可用状态。
        // 启动只同步 draw-owned state 与 factory-reported drawable，不能重建 swapchain。
        // 不再保留 unprepared hybrid fixture 分支或二次 native resize。
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
            // 已验证 owner 丢失时必须返回 typed error，不能跳过资源释放。
            let context = self.gpu_ctx.rhi_device()?;
            // 失败时保留 typed error，禁止在资源仍存活时伪造 shutdown 成功。
            renderer.release_msdf_atlas(context)?;
            // 同步释放物理 1:1 文本使用的跨帧 R8 coverage atlas。
            renderer.release_coverage_atlas(context)?;
        }
        self.gpu_ctx.try_shutdown()?;
        self.shutdown = true;
        Ok(())
    }

    fn surface(&mut self) -> &mut dyn DrawSurface {
        &mut self.surface
    }

    // 每帧准备只通过 thin RHI 设备维护，不把平台 current 语义泄露给 renderer。
    fn prepare_frame(&mut self) -> Result<(), Error> {
        // 每个测试帧从空执行证据开始，禁止把上一帧移动计入本次回读。
        #[cfg(feature = "test-harness")]
        {
            // 只有随后成功提交的共享 FramePlan 才能推进该计数。
            self.executed_texture_moves_in_frame = 0;
        }
        // 复用 GPU backend 的单一 owner-context 准备入口。
        self.prepare_rhi_device()
    }

    fn device_pixel_ratio(&self) -> f32 {
        // 每次查询只读取一个完整 surface 快照。
        device_pixel_ratio_from_surface(self.gpu_ctx.present_surface())
    }

    // 让 GPU Picture 创建区分正常无资源与 thin RHI typed failure。
    fn try_create_offscreen(
        // 借用 GPU backend 的唯一可变 owner。
        &mut self,
        // 接收 Picture 的逻辑宽度。
        width: i32,
        // 接收 Picture 的逻辑高度。
        height: i32,
        // 返回正常无资源、成功 handle 或 typed RHI 失败。
    ) -> Result<Option<ImageHandle>, Error> {
        // 只接受能由通用 RHI renderer 独占的有效 Picture extent。
        let Some(extent) = rhi_offscreen_extent(self.rhi_renderer.is_some(), width, height) else {
            // 缺少 Picture owner 或非正尺寸保持正常无资源语义。
            return Ok(None);
        };
        // 离屏资源必须由当前 owner-thread 的薄 RHI context 创建。
        // owner-thread 或 context 状态失败原样进入 renderer recovery。
        let context = self.gpu_ctx.rhi_device()?;
        // 创建唯一一份同时可渲染和可采样的通用纹理。
        let rhi_texture = context
            // 把已经验证的 extent 和统一像素格式交给薄 RHI。
            .create_texture(TextureDesc::new(
                // 使用 Picture 自身的逻辑尺寸，不重复应用主 surface DPR。
                extent,
                // 与 retained surface 和 Picture 合成保持同一颜色格式。
                TextureFormat::Bgra8Unorm,
                // RHI DeviceLost/OOM 等错误必须保持原分类。
            ))?;
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
            // Picture slot 只保存这一份 RHI 纹理身份。
            rhi_texture,
            // Picture 与主 surface 共用 GPU-only 语义，不得复活 hybrid soft upload owner。
            canvas: NativeGpuCanvas2D::new_gpu_only(width, height, self.surface.native_caps),
            width,
            height,
        });
        // 只有 RHI texture 与 backend slot 都建立后才发布 handle。
        Ok(Some(ImageHandle(id)))
    }

    fn try_destroy_offscreen(&mut self, handle: ImageHandle) -> Result<(), Error> {
        let idx = handle.0 as usize;
        let Some(Some(off)) = self.offscreens.get(idx) else {
            return Ok(());
        };
        // 复制唯一 RHI 纹理身份，释放 slot 借用后进入 owner context。
        let rhi_texture = off.rhi_texture;
        // 离屏纹理必须仍由同一 owner-thread context 管理。
        // 已验证 owner 丢失时直接返回 typed 状态错误。
        let context = self.gpu_ctx.rhi_device()?;
        // 只有 RHI 销毁成功后才释放 backend 槽位。
        context.destroy_texture(rhi_texture)?;
        // 清除已完成资源回收的 Picture slot。
        self.offscreens[idx] = None;
        if self.active_offscreen == Some(handle.0) {
            self.active_offscreen = None;
            self.offscreen_flush_committed = false;
        }
        self.free_offscreen_ids.push(handle.0);
        self.compact_offscreen_slots();
        Ok(())
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
        // 复制唯一 RHI 纹理身份，结束对 Picture slot 的不可变借用。
        let rhi_texture = target.rhi_texture;
        // 新 Picture 首次执行必须透明初始化，后续片段保留已有内容。
        let load = if self.offscreen_rhi_initialized {
            // 已提交过的 Picture 继续保留前序像素。
            LoadAction::Load
        } else {
            // 新建纹理先清为透明色，防止未初始化采样。
            LoadAction::Clear(RhiColor::transparent())
        };
        // Picture encoder 必须整条无损 lower 到通用 RHI，不能触碰 adapter 绘制接口。
        let submitted = self.try_execute_frame_encoder_rhi(
            // 传入待执行的完整 Picture encoder。
            encoder,
            // 指定唯一 RHI texture 为本次 FramePlan 目标。
            crate::draw::backend::frame_plan::RenderTargetRef::Texture(rhi_texture),
            // 使用由初始化状态推导出的 load action。
            load,
        )?;
        // 对未覆盖操作返回 typed failure，禁止静默降级到 legacy target。
        require_lossless_rhi_submission(
            // 传入完整 encoder 的实际提交结果。
            submitted,
            // 明确说明失败发生在无损 Picture lowering 边界。
            "Picture FrameEncoder cannot be lowered losslessly to the RHI texture",
        )?;
        // 只有整条 encoder RHI 提交成功才消费 Picture staging 状态。
        if let Some(Some(offscreen)) = self.offscreens.get_mut(handle.0 as usize) {
            // 让 Canvas2D 基线与已提交到纹理的内容保持一致。
            offscreen.canvas.commit_presented_frame();
        }
        // 标记唯一 RHI target 已完成初始化。
        self.offscreen_rhi_initialized = true;
        // 向场景层报告 Picture 已由通用 RHI 执行。
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
            // 首条 Clear 可完整替代 begin_frame 的 pending damage clear。
            let starts_with_clear =
                matches!(encoder.commands().first(), Some(FrameCommand::Clear { .. }));
            let rhi_load = if starts_with_clear {
                // RHI lowering 会从 encoder 的首条 Clear 读取真实颜色。
                LoadAction::Load
            } else if self.surface.needs_gpu_clear {
                // 无显式 Clear 时沿用 begin_frame 的透明初始化。
                LoadAction::Clear(RhiColor::transparent())
            } else {
                // 保留 retained surface 的前序像素。
                LoadAction::Load
            };
            // 没有全幅 clear 时，先把 begin_frame 记录的 damage 清理写入 retained texture。
            if !starts_with_clear
                && !self.surface.needs_gpu_clear
                && !self.surface.pending_clear_rects.is_empty()
            {
                // 确保局部清理与后续 encoder 写入同一代 retained texture。
                let retained_texture = self.ensure_rhi_surface_texture()?;
                // 清理必须完整 lower，不能在提交一半后切换到 legacy swapchain。
                let cleared = self.try_clear_rhi_surface_rects(retained_texture)?;
                // 缺少 ClearRect 能力或几何无法证明时保持 typed failure。
                require_lossless_rhi_submission(
                    cleared,
                    "main FrameEncoder pending clears cannot be lowered losslessly to retained RHI",
                )?;
            }
            // 主帧与 Picture 共用同一无损 RHI lowering 门禁。
            let submitted = self.try_execute_frame_encoder_rhi(
                encoder,
                crate::draw::backend::frame_plan::RenderTargetRef::Surface,
                rhi_load,
            )?;
            // 未覆盖命令必须交给恢复层，不能复活整面 readback/replace 分叉。
            require_lossless_rhi_submission(
                submitted,
                "main FrameEncoder cannot be lowered losslessly to retained RHI",
            )?;
            // RHI 片段已经替代当前 encoder，外层仍负责唯一最终 present。
            self.surface.needs_gpu_clear = false;
            // 局部清理只在后续整条 encoder 同样提交成功后消费。
            self.surface.pending_clear_rects.clear();
            // 返回无损主帧提交成功。
            Ok(())
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

    fn try_begin_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        self.offscreen_flush_committed = false;
        let idx = handle.0 as usize;
        let Some(Some(_off)) = self.offscreens.get(idx) else {
            return Err(Error::new(
                Errc::InvalidState,
                "Picture offscreen target does not exist",
            ));
        };
        if let Some(Some(off)) = self.offscreens.get_mut(idx) {
            off.canvas.reset_for_repaint();
        }
        self.active_offscreen = Some(handle.0);
        Ok(())
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
        // 复制唯一 RHI 纹理身份，供本次 FramePlan 同时作为 target 与后续 source。
        let rhi_texture = off.rhi_texture;
        // 首次提交清理新纹理，后续有序 flush 保留已有离屏内容。
        let load = if self.offscreen_rhi_initialized {
            // 保留 Picture target 已经绘制的前序内容。
            LoadAction::Load
        } else {
            // 以透明色初始化 Picture target。
            LoadAction::Clear(RhiColor::transparent())
        };
        // Picture 离屏不再允许缺少 renderer cache 的 adapter 兼容分叉。
        let Some(renderer) = self.rhi_renderer.as_mut() else {
            // 用 typed failure 暴露当前 backend 无法执行通用 Picture 计划。
            return Err(Error::new(
                // 缺失的是尚未提供的通用 RHI 能力。
                Errc::NotImplemented,
                // 给上层保留稳定的能力诊断。
                "Picture offscreen flush requires the RHI renderer cache",
            ));
        };
        // Picture 纹理必须仍由创建它的 owner-thread RHI context 管理。
        // 已验证 owner 丢失时终止当前帧并保留 typed failure。
        let context = self.gpu_ctx.rhi_context()?;
        // 冻结 Picture 纹理自己的物理范围，禁止借用主 Surface extent。
        let rhi_extent = RhiExtent::new(off.width.max(1) as u32, off.height.max(1) as u32);
        // 离屏 target 使用自身物理尺寸，不借用主窗口 drawable 的 DPR。
        let viewport = RhiViewport {
            // 纹理创建已验证正宽度，此处保持防御式下限。
            width: rhi_extent.width as f32,
            // 纹理创建已验证正高度，此处保持防御式下限。
            height: rhi_extent.height as f32,
        };
        // 记录当前 Picture 是否含有 native staging。
        let has_native = !off.canvas.pending_native.is_empty();
        // 记录当前 Picture 是否有尚未合成的 CPU soft 内容。
        let has_soft = off.canvas.soft_has_content;
        // native 前缀存在时提交已验证的通用 RHI queue。
        let native_submitted = if has_native {
            // 把 native 几何 lowering 到同一离屏 texture。
            off.canvas.submit_rhi_mixed_for_geometry(
                // 使用 backend 持有的通用 renderer cache。
                renderer,
                // Picture lowering 只取得 owner-thread Device 角色。
                context.device(),
                // 使用 Picture 纹理自身的物理范围。
                rhi_extent,
                // 采用由初始化状态推导出的 load action。
                load,
                // 指定唯一离屏纹理作为目标。
                rhi_texture,
                // 使用 Picture 自身 viewport。
                viewport,
                // Picture 逻辑坐标不额外缩放横轴。
                1.0,
                // Picture 逻辑坐标不额外缩放纵轴。
                1.0,
                // 告知 lowering 后续是否还要合成 soft staging。
                has_soft,
            )?
        } else {
            // 空 native queue 不需要额外 pass。
            true
        };
        // 未完整 lowering 的 native queue 不得再提交到 adapter target。
        if has_native {
            // 在消费 soft staging 前检查 native queue 的完整提交事实。
            require_lossless_rhi_submission(
                // 传入 native queue 的实际提交结果。
                native_submitted,
                // 明确禁止从唯一 RHI owner 切换到 legacy 绘制。
                "Picture native queue cannot be lowered losslessly to the RHI texture",
            )?;
        }
        // native 成功后 soft 段必须以 Load 继续写入同一离屏 target。
        let soft_submitted = if has_soft || !has_native {
            // 上传并合成当前 Picture 的 CPU staging。
            try_upload_rhi_canvas_soft(
                // 传入待消费的 Picture canvas。
                &mut off.canvas,
                // 使用 Picture 逻辑宽度。
                off.width,
                // 使用 Picture 逻辑高度。
                off.height,
                // 复用同一 Picture viewport。
                viewport,
                // Picture 横轴不额外应用 DPR。
                1.0,
                // Picture 纵轴不额外应用 DPR。
                1.0,
                // 继续写入唯一离屏纹理。
                rhi_texture,
                // native pass 后必须保留已经提交的像素。
                if has_native { LoadAction::Load } else { load },
                // soft 离屏合成只取得同一 owner-thread Device 角色。
                context.device(),
                // 复用同一 renderer cache。
                renderer,
            )?
        } else {
            // 没有 soft staging 时无需上传。
            true
        };
        // soft staging 未完整提交时不能报告 Picture flush 成功。
        require_lossless_rhi_submission(
            // 传入 soft 上传的实际提交结果。
            soft_submitted,
            // 明确失败发生在 Picture soft 上传边界。
            "Picture soft staging cannot be uploaded to the RHI texture",
        )?;
        // RHI 离屏 submit 成功后才提交 canvas staging 状态。
        off.canvas.commit_presented_frame();
        // 标记 target 已经有可被后续 Load pass 保留的内容。
        self.offscreen_rhi_initialized = true;
        if self.active_offscreen == Some(handle.0) {
            self.offscreen_flush_committed = true;
        }
        Ok(())
    }

    fn try_end_offscreen_paint(&mut self) -> Result<(), Error> {
        // 解除当前 Picture 绑定；薄 RHI 不需要恢复 adapter draw target。
        let active = self.active_offscreen.take();
        // 只有成功 flush 的 Picture 才能释放已经提交的 staging。
        if self.offscreen_flush_committed {
            // 定位刚刚结束的 Picture slot。
            if let Some(offscreen) = active
                // 把稳定句柄转换为槽位索引。
                .and_then(|id| self.offscreens.get_mut(id as usize))
                // 忽略已被显式销毁的空槽位。
                .and_then(Option::as_mut)
            {
                // 释放已由 RHI 消费的 Picture staging。
                offscreen.canvas.release_committed_picture_staging();
            }
        }
        // 为下一次 Picture paint 清空 flush 状态。
        self.offscreen_flush_committed = false;
        // 新的 Picture paint 将重新建立目标初始化状态。
        self.offscreen_rhi_initialized = false;
        // 结束薄 RHI Picture 生命周期无需 adapter 侧恢复操作。
        Ok(())
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
        // 复制唯一的 RHI source 纹理身份，结束对 Picture slot 的借用。
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
            // The destination is the currently bound Picture target. Submit
            // its queued native/soft commands before the immediate source
            // blit so painter order remains destination commands → blit.
            // Flushing the main swapchain here would clear/submit the wrong
            // target and invert that order.
            self.try_flush_offscreen_paint(&ImageHandle(active))?;
        } else {
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
        }
        // 唯一 RHI 离屏资源通过通用 sampled pipeline 合成。
        self.blit_rhi_offscreen_texture(
            // 传入源 Picture 的唯一纹理身份。
            source_rhi_texture,
            // 传入源纹理逻辑宽度。
            source_width,
            // 传入源纹理逻辑高度。
            source_height,
            // 传入经过上层裁剪的源区域。
            src_rect,
            // 传入父画布中的目标区域。
            dst_rect,
            // 把有限 opacity 收敛到采样 pipeline 支持的范围。
            opacity.clamp(0.0, 1.0),
            // 保留父画布的 Additive 或 SrcOver 语义。
            additive,
        )
    }

    // 使用 Picture 唯一的 RHI target 执行离屏模糊。
    fn try_blur_offscreen(
        &mut self,
        handle: &ImageHandle,
        region: Rect,
        radius: f32,
    ) -> Result<(), Error> {
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
        // Picture slot 已经保证 blur 使用通用 renderer 拥有的唯一 RHI texture。
        let texture = rhi_texture;
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
        // Picture 离屏 blur 只借用资源、命令与 submit 所需的 Device 角色。
        let device = gpu_ctx.rhi_device()?;
        // 当前 Picture texture 同时作为 source 和最终 target，scratch 由 renderer 管理。
        renderer.execute_blur_without_present(
            device,
            texture,
            RhiExtent::new(offscreen_width as u32, offscreen_height as u32),
            rhi_region,
            radius,
            TextureFormat::Bgra8Unorm,
            texture,
        )
    }

    fn snapshot_overlay_backdrop(&mut self) -> Result<bool, Error> {
        // 叠加层 backdrop 职责在独立模块实现，保持本文件处于行数上限内。
        self.snapshot_overlay_backdrop_impl()
    }

    // 对已捕获 backdrop 执行通用 RHI 双 pass blur。
    fn blur_overlay_backdrop(&mut self, region: Rect, radius: f32) -> Result<bool, Error> {
        // 多阶段效果仍委托独立 backdrop 模块，native adapter 不理解 overlay。
        self.blur_overlay_backdrop_impl(region, radius)
    }

    fn restore_overlay_backdrop(&mut self) -> Result<bool, Error> {
        // 恢复与释放委托给独立 backdrop 模块。
        self.restore_overlay_backdrop_impl()
    }

    fn release_overlay_backdrop(&mut self) -> Result<(), Error> {
        // 释放委托给独立 backdrop 模块。
        self.release_overlay_backdrop_impl()
    }

    fn has_overlay_backdrop(&self) -> bool {
        // 查询委托给独立 backdrop 模块。
        self.has_overlay_backdrop_impl()
    }

    fn present(&mut self, damage: &DamageRegion) -> Result<(), Error> {
        self.present_impl(damage)
    }

    fn test_present(&mut self) -> Result<PresentTestResult, Error> {
        // 构造门禁已经要求 backend-managed GPU 始终提供组合 thin RHI。
        // 已验证 owner 丢失时由统一 typed 状态错误进入恢复层。
        let context = self.gpu_ctx.rhi_surface()?;
        // 只通过 surface 生命周期契约执行无帧遮挡退出探测。
        context.test_present()
    }

    // 安排下一次最终 composite 在 submit 与 present 之间读取真实 surface。
    #[cfg(feature = "test-harness")]
    fn request_surface_readback_for_test(&mut self) -> Result<(), Error> {
        // 同一 backend 同时只允许一个请求，禁止覆盖尚未消费的结果。
        if self.surface_readback_requested || self.surface_readback_result.is_some() {
            // 返回可重试的稳定状态冲突。
            return Err(Error::new(
                // 重复请求属于当前状态不允许的操作。
                Errc::InvalidState,
                // 诊断不依赖具体 Adapter。
                "GPU surface readback is already pending",
            ));
        }
        // 最终 present 路径将在准确的 FramePlan 边界消费该标记。
        self.surface_readback_requested = true;
        // 请求已被唯一 backend owner 接受。
        Ok(())
    }

    // 消费最终 present 事务产生的回读结果，并始终清理请求状态。
    #[cfg(feature = "test-harness")]
    fn take_surface_readback_for_test(&mut self) -> Result<SurfaceReadback, Error> {
        // 无论 present 是否到达 composite，都允许下一次请求重新开始。
        self.surface_readback_requested = false;
        // 已执行 composite 时返回 Adapter 的真实规范结果。
        self.surface_readback_result.take().unwrap_or_else(|| {
            // 未到达回读边界通常意味着本帧在更早阶段失败。
            Err(Error::new(
                // 当前事务没有可消费结果。
                Errc::InvalidState,
                // 诊断固定在最终 composite 时序。
                "GPU frame ended before the surface readback boundary",
            ))
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// 仅在库单测构建中注册拆分后的后端契约测试。
#[cfg(test)]
// 固定同目录测试文件，避免主实现文件突破行数边界。
#[path = "../../../../../tests/unit/draw/backend/gpu/backend/render_backend_tests.rs"]
// 保持测试模块私有，不扩展生产 API。
mod tests;
