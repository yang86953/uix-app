//! [`GpuBackend`] 的 overlay backdrop 薄 RHI 实现。
//!
//! 叠加层干净背景由通用 renderer 以 BGRA texture 持有；native adapter
//! 只执行 create/copy/submit/destroy，不理解 overlay 或 backdrop 语义。

// 引入统一错误和结果类型。
use crate::core::error::{Errc, Error, Result};
// 引入无帧 blur 的逻辑区域值。
use crate::core::Rect;
// 引入薄 RHI 的资源、复制与代际值。
use crate::native::present::rhi::{
    // device 原语用于执行资源事务。
    GraphicsDevice,
    // 物理尺寸限定全幅复制范围。
    RhiExtent,
    // 物理 scissor 保存主 surface DPR lowering 结果。
    RhiScissor,
    // 提交句柄保留恢复事务的 typed 结果。
    SubmissionHandle,
    // 复制载荷只携带底层资源事实。
    TextureCopy,
    // 纹理描述用于创建独立快照。
    TextureDesc,
    // BGRA 格式与 retained 主颜色目标一致。
    TextureFormat,
    // opaque 句柄不向通用层泄漏 API 对象。
    TextureHandle,
};

// 引入当前 GPU backend owner。
use super::GpuBackend;

// 将逻辑 overlay 区域 lower 为当前 backdrop extent 内的物理 scissor。
pub(super) fn lower_overlay_blur_region(
    // 接收 UI 逻辑坐标中的目标区域。
    region: Rect,
    // 接收与快照同一 surface 元数据中的 DPR。
    device_pixel_ratio: f32,
    // 使用快照代际登记的物理边界裁剪结果。
    extent: RhiExtent,
) -> RhiScissor {
    // 非有限、非正区域或非法 DPR 不能生成 native 命令。
    if !region.x.is_finite()
        // 校验逻辑纵坐标。
        || !region.y.is_finite()
        // 校验逻辑宽度。
        || !region.w.is_finite()
        // 校验逻辑高度。
        || !region.h.is_finite()
        // 拒绝空或反向宽度。
        || region.w <= 0.0
        // 拒绝空或反向高度。
        || region.h <= 0.0
        // DPR 必须来自有效的原子 surface 快照。
        || !device_pixel_ratio.is_finite()
        // 非正 DPR 无法定义逻辑到物理映射。
        || device_pixel_ratio <= 0.0
    {
        // 空 scissor 由通用 blur renderer 作为无资源 no-op。
        return RhiScissor {
            // 保持合法零起点。
            x: 0,
            // 保持合法零起点。
            y: 0,
            // 零宽度表示不可见。
            width: 0,
            // 零高度表示不可见。
            height: 0,
        };
    }
    // 左上边界向外取整，避免逻辑区域遗漏覆盖像素。
    let left = (region.x * device_pixel_ratio).floor();
    // 顶边同样向外取整。
    let top = (region.y * device_pixel_ratio).floor();
    // 右边界从逻辑终点独立换算，避免宽度累计取整误差。
    let right = ((region.x + region.w) * device_pixel_ratio).ceil();
    // 底边从逻辑终点独立换算。
    let bottom = ((region.y + region.h) * device_pixel_ratio).ceil();
    // 溢出后的非有限边界不能进入整数转换。
    if [left, top, right, bottom]
        // 逐边检查浮点有效性。
        .iter()
        // 任一异常都退化为空区域。
        .any(|edge| !edge.is_finite())
    {
        // 复用空区域返回值。
        return RhiScissor {
            // 保持合法零起点。
            x: 0,
            // 保持合法零起点。
            y: 0,
            // 零宽度表示不可见。
            width: 0,
            // 零高度表示不可见。
            height: 0,
        };
    }
    // 将物理边界裁到快照纹理宽度。
    let left = left.clamp(0.0, extent.width as f32) as i32;
    // 将物理顶边裁到快照纹理高度。
    let top = top.clamp(0.0, extent.height as f32) as i32;
    // 将物理右边界裁到相同宽度。
    let right = right.clamp(0.0, extent.width as f32) as i32;
    // 将物理底边界裁到相同高度。
    let bottom = bottom.clamp(0.0, extent.height as f32) as i32;
    // 返回已经完成 DPR 和 extent 裁剪的区域。
    RhiScissor {
        // 保存裁剪后的左边界。
        x: left,
        // 保存裁剪后的顶边界。
        y: top,
        // 终点不大于起点时形成稳定空宽度。
        width: right.saturating_sub(left),
        // 终点不大于起点时形成稳定空高度。
        height: bottom.saturating_sub(top),
    }
}

// 构造覆盖完整物理纹理范围的有向复制。
fn full_texture_copy(
    // 指定源纹理。
    source: TextureHandle,
    // 指定目标纹理。
    destination: TextureHandle,
    // 使用二者共同的物理尺寸。
    extent: RhiExtent,
) -> TextureCopy {
    // 返回零偏移的全幅复制命令。
    TextureCopy {
        // 保留源资源身份。
        source,
        // 保留目标资源身份。
        destination,
        // 全幅复制从源左上角开始。
        source_x: 0,
        // 全幅复制从源顶边开始。
        source_y: 0,
        // 全幅复制写到目标左上角。
        destination_x: 0,
        // 全幅复制写到目标顶边。
        destination_y: 0,
        // 宽度严格采用 surface token 的物理宽度。
        width: extent.width,
        // 高度严格采用 surface token 的物理高度。
        height: extent.height,
    }
}

// 创建一次 overlay backdrop，并在提交失败时检查式回收新纹理。
pub(super) fn create_rhi_overlay_backdrop(
    // 使用 native adapter 暴露的薄 device 原语。
    device: &mut (impl GraphicsDevice + ?Sized),
    // 从已经提交的 retained surface 复制干净背景。
    retained: TextureHandle,
    // 快照尺寸来自同代 SurfaceToken。
    extent: RhiExtent,
) -> Result<TextureHandle> {
    // 创建与 retained surface 相同格式和尺寸的独立纹理。
    let backdrop = device.create_texture(TextureDesc {
        // 快照覆盖完整 drawable。
        extent,
        // retained surface 当前统一使用 premultiplied BGRA。
        format: TextureFormat::Bgra8Unorm,
    })?;
    // 复制并提交形成可跨帧读取的确定边界。
    let submitted = device
        // 第一步只编码 retained 到 backdrop 的全幅复制。
        .copy_texture(full_texture_copy(retained, backdrop, extent))
        // 第二步提交 device 命令，但不获取或呈现 surface image。
        .and_then(|()| device.submit().map(|_| ()));
    // 任一步失败都不能把半成品纹理登记为有效快照。
    if let Err(error) = submitted {
        // 新纹理必须检查式销毁，避免失败帧泄漏 GPU 资源。
        return match device.destroy_texture(backdrop) {
            // 清理成功时保留原始复制或提交错误。
            Ok(()) => Err(error),
            // 清理也失败时以清理错误包裹原始失败，保留两条证据。
            Err(cleanup_error) => Err(cleanup_error.with_source(error)),
        };
    }
    // 只有复制和提交都成功后才把纹理交给 owner。
    Ok(backdrop)
}

// 将已提交的 backdrop 恢复到 retained surface。
pub(super) fn restore_rhi_overlay_backdrop(
    // 使用同一个 owner-thread device 执行复制。
    device: &mut (impl GraphicsDevice + ?Sized),
    // 读取稳定的干净背景纹理。
    backdrop: TextureHandle,
    // 写回当前 retained surface 纹理。
    retained: TextureHandle,
    // 复制范围来自已经验证相等的 token。
    extent: RhiExtent,
) -> Result<SubmissionHandle> {
    // 先编码 backdrop 到 retained 的全幅复制。
    device.copy_texture(full_texture_copy(backdrop, retained, extent))?;
    // 单独提交复制，最终 swapchain present 仍由统一帧边界负责。
    device.submit()
}

// 为 GpuBackend 提供 backdrop owner 生命周期。
impl GpuBackend {
    // 检查式销毁当前 backdrop texture；失败时恢复 owner 状态供重试。
    pub(super) fn destroy_rhi_overlay_backdrop_texture(&mut self) -> Result<(), Error> {
        // 没有纹理时只清理不可能独立存在的代际标记。
        let Some(texture) = self.rhi_overlay_backdrop_texture.take() else {
            // 空 owner 不应保留陈旧 token。
            self.rhi_overlay_backdrop_token = None;
            // 幂等释放成功。
            return Ok(());
        };
        // 暂存 token，销毁失败时恢复完整 owner 状态。
        let token = self.rhi_overlay_backdrop_token.take();
        // 资源必须由创建它的组合 RHI context 检查式销毁。
        let result = self
            // 取得当前 owner context。
            .gpu_ctx
            // 只借用一次底层资源表。
            .rhi_context()
            // 由 adapter 执行底层 texture 释放。
            .and_then(|context| context.destroy_texture(texture));
        // 失败时保留资源身份，禁止把泄漏伪装为成功。
        if let Err(error) = result {
            // 恢复 texture 供 shutdown 或 recovery 重试。
            self.rhi_overlay_backdrop_texture = Some(texture);
            // 恢复原 token，保持代际审计信息。
            self.rhi_overlay_backdrop_token = token;
            // 传播真实 owner-thread 失败。
            return Err(error);
        }
        // 成功释放后保持两个 owner 字段都为空。
        Ok(())
    }

    // 叠加层 backdrop 快照依赖已经提交的 retained framebuffer。
    pub(super) fn snapshot_overlay_backdrop_impl(&mut self) -> Result<bool, Error> {
        // 没有 retained profile 的 adapter 不进入通用快照路径。
        if !self.surface.native_caps.retained_framebuffer {
            // 保持现有不支持语义。
            return Ok(false);
        }
        // 快照只能读取已经存在的 retained surface；禁止创建透明纹理冒充上一帧。
        let (Some(retained), Some(token)) = (self.rhi_surface_texture, self.rhi_surface_token)
        else {
            // 首帧或 legacy 回退后没有可捕获内容。
            return Ok(false);
        };
        // 旧快照必须先检查式释放，确保只有一个明确 owner。
        self.destroy_rhi_overlay_backdrop_texture()?;
        // overlay 的无帧 RHI 事务也复用同一设备准备入口。
        self.prepare_rhi_device()?;
        // 将 context 借用限制在 token 校验与创建提交事务内。
        let result = {
            // 只有暴露组合 RHI 的 adapter 才能执行通用快照。
            // 构造期已验证的 owner 丢失时必须返回 typed failure。
            let context = self.gpu_ctx.rhi_context()?;
            // surface 重建后旧 retained handle 不能进入复制。
            if context.token() != token {
                // 代际不一致由调用方退回整树重绘。
                return Ok(false);
            }
            // 执行 create/copy/submit，并让 helper 处理失败清理。
            create_rhi_overlay_backdrop(context, retained, token.extent)
        };
        // 只有完整事务成功才登记快照 owner。
        match result {
            // 保存 texture 与对应 surface token。
            Ok(texture) => {
                // 登记可复用的干净背景纹理。
                self.rhi_overlay_backdrop_texture = Some(texture);
                // 登记创建时的 surface 代际和 extent。
                self.rhi_overlay_backdrop_token = Some(token);
                // 向场景管线报告可恢复背景。
                Ok(true)
            }
            // 失败事务未登记任何快照，并保留底层 typed failure。
            Err(error) => Err(error),
        }
    }

    // 对当前同代 overlay backdrop 执行区域高斯模糊。
    pub(super) fn blur_overlay_backdrop_impl(
        // 借用唯一 GPU backend owner。
        &mut self,
        // 接收主 surface 的逻辑区域。
        region: Rect,
        // 使用与 Picture blur 相同的逻辑半径语义。
        radius: f32,
    ) -> Result<bool, Error> {
        // 缺失任一资源身份时不能伪造已执行。
        let (Some(backdrop), Some(backdrop_token), Some(surface_token)) = (
            // 读取已捕获纹理。
            self.rhi_overlay_backdrop_texture,
            // 读取快照代际。
            self.rhi_overlay_backdrop_token,
            // 读取当前 retained surface 代际。
            self.rhi_surface_token,
        ) else {
            // 首帧、legacy 回退或释放后的 owner 不支持本次操作。
            return Ok(false);
        };
        // resize 或 recovery 后禁止改写旧代纹理。
        if backdrop_token != surface_token {
            // 交由上层重新捕获干净背景。
            return Ok(false);
        }
        // 非有限或负半径是调用契约错误，不能静默伪装成功。
        if !radius.is_finite() || radius < 0.0 {
            // 返回稳定 typed 参数错误。
            return Err(Error::new(
                // 使用统一无效参数分类。
                Errc::InvalidArgument,
                // 提供不依赖 adapter 的诊断文本。
                "overlay backdrop blur radius is invalid",
            ));
        }
        // 小于半像素的半径按现有 blur 语义保持无资源 no-op。
        if radius < 0.5 {
            // owner 有效且请求无需改写，报告事务已处理。
            return Ok(true);
        }
        // 从与 recipe owner 同一原子 surface 快照读取 DPR。
        let device_pixel_ratio = super::device_pixel_ratio_from_surface(
            // 不拆分查询 drawable extent 与比例。
            self.gpu_ctx.present_surface(),
        );
        // 使用快照代际的 extent 完成逻辑区域 lowering。
        let physical_region = lower_overlay_blur_region(
            // 传入调用方逻辑区域。
            region,
            // 传入规范化后的有效 DPR。
            device_pixel_ratio,
            // 以 backdrop 的物理尺寸为唯一裁剪边界。
            backdrop_token.extent,
        );
        // 无帧多阶段事务也必须先执行 device maintenance。
        self.prepare_rhi_device()?;
        // 分开借用组合 context 与通用 renderer cache。
        let (gpu_ctx, rhi_renderer) = (&mut self.gpu_ctx, &mut self.rhi_renderer);
        // 缺失 renderer cache 时禁止走平台专属效果路径。
        let Some(renderer) = rhi_renderer.as_mut() else {
            // 返回稳定的迁移期未实现错误。
            return Err(Error::new(
                // 能力未装配归类为未实现。
                Errc::NotImplemented,
                // 诊断明确要求通用 renderer owner。
                "overlay backdrop blur requires the RHI renderer cache",
            ));
        };
        // 只有构造期验证的组合 RHI context 可以执行计划。
        let context = gpu_ctx.rhi_context()?;
        // native surface 代际也必须仍与快照一致。
        if context.token() != backdrop_token {
            // 迟到事务不触碰重建后的资源表。
            return Ok(false);
        }
        // 复用通用 renderer 的 backdrop 原位双 pass 入口。
        renderer.execute_overlay_backdrop_blur(
            // 传入 owner-thread context。
            context,
            // backdrop 同时作为 source 和最终 target。
            backdrop,
            // 使用登记代际的物理 extent。
            backdrop_token.extent,
            // 使用已经完成 DPR lowering 的 scissor。
            physical_region,
            // 保留调用方半径。
            radius,
        )?;
        // 事务完整提交且 scratch 已清理。
        Ok(true)
    }

    // 将已验证同代的干净背景恢复到 retained surface。
    pub(super) fn restore_overlay_backdrop_impl(&mut self) -> Result<bool, Error> {
        // 同时读取快照、retained target 与二者的代际。
        let (Some(backdrop), Some(backdrop_token), Some(retained), Some(surface_token)) = (
            self.rhi_overlay_backdrop_texture,
            self.rhi_overlay_backdrop_token,
            self.rhi_surface_texture,
            self.rhi_surface_token,
        ) else {
            // 任一 owner 缺失都不能伪造恢复成功。
            return Ok(false);
        };
        // backdrop 只能写回创建它的同代同尺寸 retained surface。
        if backdrop_token != surface_token {
            // resize 或 recovery 后交由场景整树重绘。
            return Ok(false);
        }
        // 恢复复制同样通过 thin RHI device maintenance 准备 owner context。
        self.prepare_rhi_device()?;
        // 将 context 借用限制在代际校验和一次复制提交内。
        let result = {
            // 只有组合 RHI 可以恢复通用 texture。
            // context 丢失时保留快照 owner，并把 typed failure 交给恢复层。
            let context = self.gpu_ctx.rhi_context()?;
            // 当前 native surface token 也必须与 retained owner 完全一致。
            if context.token() != surface_token {
                // 迟到恢复不能写入重建后的资源表。
                return Ok(false);
            }
            // 编码 backdrop 到 retained 的全幅复制并提交。
            restore_rhi_overlay_backdrop(context, backdrop, retained, surface_token.extent)
        };
        // typed 失败直接越过场景边界，禁止被整树重绘掩盖。
        result?;
        // 恢复已经覆盖完整 retained surface，取消 begin_frame 的全幅清理。
        self.surface.needs_gpu_clear = false;
        // 清空被完整背景恢复替代的局部清理计划。
        self.surface.pending_clear_rects.clear();
        // 恢复本身已经产生新的 retained 内容，最终边界必须合成并 present。
        self.rhi_surface_frame_pending_present = true;
        // 场景现在可以只重绘 overlays。
        Ok(true)
    }

    // 显式释放 overlay backdrop，并把真实 owner-thread 失败交给恢复边界。
    pub(super) fn release_overlay_backdrop_impl(&mut self) -> Result<(), Error> {
        // 检查式销毁失败时保留 owner，并把 typed failure 交回恢复边界。
        self.destroy_rhi_overlay_backdrop_texture()
    }

    // 查询当前是否持有与 retained surface 同代的有效快照。
    pub(super) fn has_overlay_backdrop_impl(&self) -> bool {
        // texture 与 token 必须成对存在，且 token 与当前 retained owner 相同。
        self.rhi_overlay_backdrop_texture.is_some()
            && self.rhi_overlay_backdrop_token.is_some()
            && self.rhi_overlay_backdrop_token == self.rhi_surface_token
    }
}
