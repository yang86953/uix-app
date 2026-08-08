//! 主 surface retained 颜色目标的 owner-thread 生命周期。

// 引入统一错误和结果类型。
use crate::core::error::{Errc, Error, Result};
// 引入 FramePlan 的纹理 target 引用。
use crate::draw::backend::frame_plan::RenderTargetRef;
// 引入薄 RHI 的 surface token、纹理描述和颜色格式。
use crate::native::present::rhi::{RenderTargetHandle, TextureDesc, TextureFormat, TextureHandle};

// 引入当前 GPU backend owner。
use super::GpuBackend;

// 为 GpuBackend 提供 retained surface 纹理的创建、重建和销毁边界。
impl GpuBackend {
    // 返回当前 RHI 主 surface 的持久 render target；没有组合 RHI 时保持兼容回退。
    pub(super) fn try_rhi_surface_target(&mut self) -> Result<Option<RenderTargetRef>, Error> {
        // 没有通用 renderer 时不能让 retained target 脱离统一资源缓存。
        if self.rhi_renderer.is_none() {
            // 让调用方继续使用既有兼容路径。
            return Ok(None);
        }
        // 只有暴露组合 RHI 的 adapter 才能创建并提交 retained texture。
        if self.gpu_ctx.rhi_context().is_none() {
            // 其他原生后端暂不改变原有 surface 路径。
            return Ok(None);
        }
        // 按当前 surface generation 确保纹理身份和 extent 一致。
        let texture = self.ensure_rhi_surface_texture()?;
        // 把同一 opaque texture 同时作为 render target 和 sampled source 使用。
        Ok(Some(RenderTargetRef::Texture(
            RenderTargetHandle::from_raw(texture.raw()),
        )))
    }

    // 确保 retained texture 与当前 surface token 同代且尺寸一致。
    pub(super) fn ensure_rhi_surface_texture(&mut self) -> Result<TextureHandle, Error> {
        // 读取 owner-thread context 的当前 surface 身份。
        let token = self
            .gpu_ctx
            .rhi_context()
            .map(|context| context.token())
            .ok_or_else(|| {
                // 没有 context 时不能把空句柄当作 retained target。
                Error::new(
                    Errc::InvalidState,
                    "retained RHI surface requires a composable graphics context",
                )
            })?;
        // 同一代际和 extent 可以安全复用现有颜色纹理。
        if self.rhi_surface_token == Some(token) {
            // token 相同意味着已有纹理已经通过本边界创建。
            if let Some(texture) = self.rhi_surface_texture {
                // 返回跨帧复用的 retained target。
                return Ok(texture);
            }
        }
        // 代际或尺寸变化时先取出旧句柄，避免资源表借用跨过字段更新。
        let previous_texture = self.rhi_surface_texture.take();
        // 保存旧 token，销毁失败时恢复完整 owner 状态。
        let previous_token = self.rhi_surface_token.take();
        // 旧 retained texture 必须在创建新代际前由同一 owner context 检查式释放。
        if let Some(texture) = previous_texture {
            // 借用 context 只覆盖一次 native destroy 调用。
            let destroy_result = self
                .gpu_ctx
                .rhi_context()
                .ok_or_else(|| {
                    // context 丢失时恢复旧句柄，交给 shutdown/recovery 重试。
                    Error::new(
                        Errc::InvalidState,
                        "retained RHI surface lost its owner context during rebuild",
                    )
                })
                .and_then(|context| context.destroy_texture(texture));
            // 资源销毁失败不能伪造新 target 已准备好。
            if let Err(error) = destroy_result {
                // 恢复旧资源身份，保留后续 checked shutdown 的重试机会。
                self.rhi_surface_texture = Some(texture);
                // 恢复旧代际信息，避免下次检查误判为空。
                self.rhi_surface_token = previous_token;
                // 向上层传播真实 owner-thread 失败。
                return Err(error);
            }
        }
        // 创建与 drawable 物理 extent 完全一致的 BGRA retained target。
        let texture = self
            .gpu_ctx
            .rhi_context()
            .ok_or_else(|| {
                // context 在生命周期中途消失时保持 typed state error。
                Error::new(
                    Errc::InvalidState,
                    "retained RHI surface context disappeared before creation",
                )
            })?
            .create_texture(TextureDesc {
                // retained image 必须覆盖整个当前 drawable。
                extent: token.extent,
                // BGRA 与 Windows swapchain/native image 的字节布局一致。
                format: TextureFormat::Bgra8Unorm,
            })?;
        // 登记新代际 texture，后续 FramePlan 只复用这一身份。
        self.rhi_surface_texture = Some(texture);
        // 登记 generation 和 extent，resize/lost 时强制重建。
        self.rhi_surface_token = Some(token);
        // 新 target 没有任何已提交像素，下一次 pass 必须透明初始化。
        self.surface.needs_gpu_clear = true;
        // 新 target 不再继承旧代际的局部 clear 队列。
        self.surface.pending_clear_rects.clear();
        // 新 target 尚未完成最终合成 present。
        self.rhi_surface_frame_pending_present = false;
        // 返回刚创建的 retained surface texture。
        Ok(texture)
    }

    // 在 resize、device shutdown 或 surface 重建前释放 retained texture。
    pub(super) fn destroy_rhi_surface_texture(&mut self) -> Result<(), Error> {
        // backdrop 与 retained surface 共享代际，必须先检查式释放快照。
        self.destroy_rhi_overlay_backdrop_texture()?;
        // 没有纹理时只清理代际标记，保持幂等。
        let Some(texture) = self.rhi_surface_texture.take() else {
            // 空 owner 不应保留陈旧代际。
            self.rhi_surface_token = None;
            self.rhi_surface_frame_pending_present = false;
            return Ok(());
        };
        // 暂存 token，销毁失败时恢复句柄与代际。
        let token = self.rhi_surface_token.take();
        // 资源必须在创建它的组合 context 上检查式销毁。
        let result = self
            .gpu_ctx
            .rhi_context()
            .ok_or_else(|| {
                // context 消失时不能丢弃尚未释放的 opaque handle。
                Error::new(
                    Errc::InvalidState,
                    "retained RHI surface lost its owner context during destroy",
                )
            })
            .and_then(|context| context.destroy_texture(texture));
        // 销毁失败时恢复 owner 状态，禁止资源泄漏被隐藏。
        if let Err(error) = result {
            // 保留未释放的 texture 供下一次 shutdown/recovery 重试。
            self.rhi_surface_texture = Some(texture);
            // 恢复原代际用于后续 stale 检查。
            self.rhi_surface_token = token;
            // 返回真实资源错误。
            return Err(error);
        }
        // 成功销毁后清空等待 present 的标记。
        self.rhi_surface_frame_pending_present = false;
        // 返回检查式释放成功。
        Ok(())
    }

    // 放弃当前 retained surface，准备安全回到直接 swapchain 的兼容帧。
    pub(super) fn abandon_rhi_surface_texture_for_legacy(&mut self) -> Result<(), Error> {
        // 没有 retained texture 时无需改变当前兼容路径状态。
        if self.rhi_surface_texture.is_none() {
            // 保持调用幂等，避免无意义的 context 访问。
            return Ok(());
        }
        // 先按 owner-thread 规则销毁旧纹理，避免 legacy 绘制读取陈旧副本。
        self.destroy_rhi_surface_texture()?;
        // 丢弃 retained 内容后，下一次直接绘制必须从透明全清开始。
        self.surface.needs_gpu_clear = true;
        // 旧代际的局部 clear 不能继续作用于新兼容目标。
        self.surface.pending_clear_rects.clear();
        // 旧 retained 内容已被销毁，尚未提交的 scroll 不能再被误应用。
        self.surface.pending_scroll_copies.clear();
        // 告知调用方已经安全切换到 legacy target。
        Ok(())
    }
}
