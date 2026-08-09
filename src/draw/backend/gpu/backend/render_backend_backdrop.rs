//! [`GpuBackend`] 的 overlay backdrop 薄 RHI 实现。
//!
//! 叠加层干净背景由通用 renderer 以 BGRA texture 持有；native adapter
//! 只执行 create/copy/submit/destroy，不理解 overlay 或 backdrop 语义。

// 引入统一错误和结果类型。
use crate::core::error::{Errc, Error, Result};
// 引入薄 RHI 的资源、复制与代际值。
use crate::native::present::rhi::{
    // device 原语用于执行资源事务。
    GraphicsDevice,
    // 物理尺寸限定全幅复制范围。
    RhiExtent,
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
            // context 丢失时不能丢弃 opaque handle。
            .ok_or_else(|| {
                // 返回可恢复层识别的稳定状态错误。
                Error::new(
                    Errc::InvalidState,
                    "overlay backdrop lost its owner context during destroy",
                )
            })
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
    pub(super) fn snapshot_overlay_backdrop_impl(&mut self) -> bool {
        // 没有 retained profile 的 adapter 不进入通用快照路径。
        if !self.surface.native_caps.retained_framebuffer {
            // 保持现有不支持语义。
            return false;
        }
        // 快照只能读取已经存在的 retained surface；禁止创建透明纹理冒充上一帧。
        let (Some(retained), Some(token)) = (self.rhi_surface_texture, self.rhi_surface_token)
        else {
            // 首帧或 legacy 回退后没有可捕获内容。
            return false;
        };
        // 旧快照必须先检查式释放，确保只有一个明确 owner。
        if let Err(error) = self.destroy_rhi_overlay_backdrop_texture() {
            // 资源回收失败时不覆盖旧 owner。
            tracing::warn!(
                "GpuBackend: overlay backdrop replacement destroy failed: {}",
                error.short_what()
            );
            // 向无错误返回值的场景边界报告失败。
            return false;
        }
        // overlay 的无帧 RHI 事务也复用同一设备准备入口。
        if let Err(error) = self.prepare_rhi_device() {
            // 设备维护失败时不进入资源创建事务。
            tracing::warn!(
                "GpuBackend: RHI overlay backdrop snapshot device preparation failed: {}",
                error.short_what()
            );
            // 没有创建新纹理，场景可安全退回整树重绘。
            return false;
        }
        // 将 context 借用限制在 token 校验与创建提交事务内。
        let result = {
            // 只有暴露组合 RHI 的 adapter 才能执行通用快照。
            let Some(context) = self.gpu_ctx.rhi_context() else {
                // 兼容 adapter 不应声明 retained RHI 快照成功。
                return false;
            };
            // surface 重建后旧 retained handle 不能进入复制。
            if context.token() != token {
                // 代际不一致由调用方退回整树重绘。
                return false;
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
                true
            }
            // typed 失败降级为既有布尔边界并记录诊断。
            Err(error) => {
                // 保留底层错误文本便于真实 adapter 调试。
                tracing::warn!(
                    "GpuBackend: RHI overlay backdrop snapshot failed: {}",
                    error.short_what()
                );
                // 失败事务未登记任何快照。
                false
            }
        }
    }

    // 将已验证同代的干净背景恢复到 retained surface。
    pub(super) fn restore_overlay_backdrop_impl(&mut self) -> bool {
        // 同时读取快照、retained target 与二者的代际。
        let (Some(backdrop), Some(backdrop_token), Some(retained), Some(surface_token)) = (
            self.rhi_overlay_backdrop_texture,
            self.rhi_overlay_backdrop_token,
            self.rhi_surface_texture,
            self.rhi_surface_token,
        ) else {
            // 任一 owner 缺失都不能伪造恢复成功。
            return false;
        };
        // backdrop 只能写回创建它的同代同尺寸 retained surface。
        if backdrop_token != surface_token {
            // resize 或 recovery 后交由场景整树重绘。
            return false;
        }
        // 恢复复制同样通过 thin RHI device maintenance 准备 owner context。
        if let Err(error) = self.prepare_rhi_device() {
            // 保留快照 owner，等待场景释放或下一次恢复。
            tracing::warn!(
                "GpuBackend: RHI overlay backdrop restore device preparation failed: {}",
                error.short_what()
            );
            // begin_frame 的清理状态保持不变，后续整树重绘仍安全。
            return false;
        }
        // 将 context 借用限制在代际校验和一次复制提交内。
        let result = {
            // 只有组合 RHI 可以恢复通用 texture。
            let Some(context) = self.gpu_ctx.rhi_context() else {
                // context 丢失时保留快照 owner 供显式释放或恢复。
                return false;
            };
            // 当前 native surface token 也必须与 retained owner 完全一致。
            if context.token() != surface_token {
                // 迟到恢复不能写入重建后的资源表。
                return false;
            }
            // 编码 backdrop 到 retained 的全幅复制并提交。
            restore_rhi_overlay_backdrop(context, backdrop, retained, surface_token.extent)
        };
        // typed 失败通过日志和布尔边界触发整树重绘。
        if let Err(error) = result {
            // 保留原快照，后续 release 仍可检查式回收。
            tracing::warn!(
                "GpuBackend: RHI overlay backdrop restore failed: {}",
                error.short_what()
            );
            // 不消费 begin_frame 清理状态。
            return false;
        }
        // 恢复已经覆盖完整 retained surface，取消 begin_frame 的全幅清理。
        self.surface.needs_gpu_clear = false;
        // 清空被完整背景恢复替代的局部清理计划。
        self.surface.pending_clear_rects.clear();
        // 恢复本身已经产生新的 retained 内容，最终边界必须合成并 present。
        self.rhi_surface_frame_pending_present = true;
        // 场景现在可以只重绘 overlays。
        true
    }

    // 显式释放 overlay backdrop，保持无错误场景接口的兼容行为。
    pub(super) fn release_overlay_backdrop_impl(&mut self) {
        // 内部仍执行检查式销毁，失败时保留 owner 并记录诊断。
        if let Err(error) = self.destroy_rhi_overlay_backdrop_texture() {
            // 无 Result 的场景清理边界只能记录并等待 shutdown 重试。
            tracing::warn!(
                "GpuBackend: RHI overlay backdrop release failed: {}",
                error.short_what()
            );
        }
    }

    // 查询当前是否持有与 retained surface 同代的有效快照。
    pub(super) fn has_overlay_backdrop_impl(&self) -> bool {
        // texture 与 token 必须成对存在，且 token 与当前 retained owner 相同。
        self.rhi_overlay_backdrop_texture.is_some()
            && self.rhi_overlay_backdrop_token.is_some()
            && self.rhi_overlay_backdrop_token == self.rhi_surface_token
    }
}
