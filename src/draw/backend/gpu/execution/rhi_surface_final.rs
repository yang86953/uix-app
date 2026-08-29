//! retained 主颜色纹理到真实 surface 的唯一最终合成边界。

// 引入 typed error 与最终 present damage。
use crate::core::{Errc, Error, PresentDamage};
// 引入已有纹理 sampled quad 的通用 RHI 载荷。
use crate::draw::backend::rhi_renderer::RhiSampledQuad;
// 引入物理 viewport 与 retained texture 句柄。
use crate::platform::presentation::rhi::{RhiViewport, TextureHandle};

// 引入当前唯一 GPU backend owner。
use super::GpuBackend;
// 引入最终 retained-to-swapchain 的统一 damage/scissor 规划。
use super::rhi_surface_composite::plan_surface_composite;

// 为主 surface retained texture 提供统一的最终采样合成与 present 边界。
impl GpuBackend {
    // 把 retained texture 全幅采样到当前 swapchain，并完成唯一最终 present。
    pub(super) fn present_rhi_surface_texture(
        &mut self,
        texture: TextureHandle,
        damage: PresentDamage,
    ) -> Result<(), Error> {
        // 读取当前 surface 的物理 extent，避免用逻辑尺寸猜测 drawable。
        let extent = self
            .gpu_ctx
            .rhi_surface()
            // retained target 的物理尺寸只依赖当前 surface 代际。
            .map(|context| context.token().extent)?;
        // 构造全幅 sampled quad，保持 retained image 的原始 premultiplied 像素。
        let viewport = RhiViewport {
            // 采样目标宽度使用物理 drawable extent。
            width: extent.width as f32,
            // 采样目标高度使用物理 drawable extent。
            height: extent.height as f32,
        };
        // 平台窗口层把系统差异归一化为唯一物理圆角值，Drawing 不分支识别操作系统。
        let surface_corner_radius = self
            .gpu_ctx
            .rhi_surface()
            .map(|surface| surface.surface_corner_radius())?;
        // 客户端阴影环事实与圆角同源，缺口补画必须与外圈阴影共用同一外观参数。
        let (surface_shadow_fill, surface_shadow_fill_range) = self
            .gpu_ctx
            .rhi_surface()
            .map(|surface| surface.surface_shadow_fill())?;
        // 为最终合成准备覆盖整个 swapchain 的四角几何。
        let quad = RhiSampledQuad {
            // 全幅合成从物理左上角开始。
            x: 0.0,
            // 全幅合成从物理顶部开始。
            y: 0.0,
            // 覆盖当前 drawable 宽度。
            w: viewport.width,
            // 覆盖当前 drawable 高度。
            h: viewport.height,
            // 采样四角保持左上原点约定。
            corners: super::super::GpuGlyphBlit::axis_aligned_corners(
                0.0,
                0.0,
                viewport.width,
                viewport.height,
            ),
            // retained texture 已经包含最终颜色，不再额外改变 tint。
            rgba: [1.0; 4],
            // 主 surface 合成固定使用 SrcOver。
            additive: false,
            // 绑定本代际 retained texture 作为 sampled source。
            texture,
            // 采样完整纹理的左侧 UV。
            u0: 0.0,
            // 采样完整纹理的顶部 UV。
            v0: 0.0,
            // 采样完整纹理的右侧 UV。
            u1: 1.0,
            // 采样完整纹理的底部 UV。
            v1: 1.0,
            // 只在最终 retained-to-surface 合成应用窗口边界遮罩。
            surface_corner_radius,
            // 客户端阴影环启用时在圆角缺口内补画外圈阴影的连续延伸。
            surface_shadow_fill,
            surface_shadow_fill_range,
            // 最终合成不额外裁剪。
            scissor: None,
        };
        // 用同一份验证结果生成逐矩形绘制与最终 present damage。
        let composite = plan_surface_composite(damage, extent, quad);
        // test-harness / Agent 截屏只在本次唯一最终 composite 消费一个回读请求。
        #[cfg(any(feature = "test-harness", feature = "agent-control"))]
        let readback_requested = std::mem::take(&mut self.surface_readback_requested);
        // 冻结当前内容帧已经成功执行的 API 无关纹理移动证据。
        #[cfg(any(feature = "test-harness", feature = "agent-control"))]
        let executed_texture_moves = self.executed_texture_moves_in_frame;
        // 保存 submit/present 间隙由 Adapter 产生的规范结果。
        #[cfg(any(feature = "test-harness", feature = "agent-control"))]
        let mut readback_result = None;
        // 只在 owner-thread 借用范围内执行最终 swapchain pass。
        let result = {
            // 同时借用 context 与 renderer，保持资源和执行在同一 owner thread。
            let (gpu_ctx, rhi_renderer) = (&mut self.gpu_ctx, &mut self.rhi_renderer);
            // retained target 只由已经准备好的通用 renderer 负责采样。
            let Some(renderer) = rhi_renderer.as_mut() else {
                // 缺少 renderer 时不把合成当成成功。
                return Err(Error::new(
                    Errc::InvalidState,
                    "retained RHI surface has no renderer cache",
                ));
            };
            // 只有组合 RHI context 能执行 surface acquire/submit/present。
            // 已验证 owner 丢失时保留 typed state error，禁止绕过 RHI 合成。
            let context = gpu_ctx.rhi_context()?;
            // test-harness / Agent 截屏在同一 FramePlan submit 与 present 之间读取最终 composite。
            #[cfg(any(feature = "test-harness", feature = "agent-control"))]
            {
                // 构造只观察当前 Surface 的一次性钩子。
                let mut before_present =
                    |surface: &mut dyn crate::platform::presentation::rhi::GraphicsSurface| {
                        // 未安排请求时不调用任何同步 GPU 回读。
                        if readback_requested {
                            // 标记同步 surface 回读即将进入 Adapter，便于定位 GPU 等待边界。
                            tracing::debug!("surface readback adapter call started");
                            // 在局部值中保存 Adapter 回读和共享执行证据。
                            let result = GpuBackend::try_readback(surface).map(|readback| {
                                // 附加共享 FramePlan 已成功执行的移动次数。
                                readback.with_executed_texture_moves(executed_texture_moves)
                            });
                            // 记录 Adapter 调用已经返回及其 typed 成功状态。
                            tracing::debug!(
                                // 只输出成功布尔值，不输出像素载荷。
                                success = result.is_ok(),
                                // 使用稳定事件名闭合回读调用区间。
                                "surface readback adapter call finished"
                            );
                            // 保存规范像素或 typed Adapter failure，不能影响最终 present。
                            readback_result = Some(result);
                        }
                    };
                // 用一次 acquire/submit/hook/present 执行全部 damage rect 的 sampled draw。
                renderer.execute_sampled_quads_with_present_hook(
                    // 借用当前唯一组合 context。
                    context,
                    // native present 消费与 quad scissor 同源的 damage。
                    composite.damage.clone(),
                    // 使用当前物理 viewport。
                    viewport,
                    // partial 使用 Load，Full 使用透明 Clear。
                    composite.load,
                    // 每个 partial rect 对应一个裁剪 quad。
                    &composite.quads,
                    // 观察钩子严格位于最终 present 前。
                    &mut before_present,
                )
            }
            // 普通构建保持唯一 acquire/submit/present 路径且没有回读开销。
            #[cfg(not(any(feature = "test-harness", feature = "agent-control")))]
            {
                // 用一次 acquire/submit/present 执行全部 damage rect 的 sampled draw。
                renderer.execute_sampled_quads(
                    // 借用当前唯一组合 context。
                    context,
                    // native present 消费与 quad scissor 同源的 damage。
                    composite.damage.clone(),
                    // 使用当前物理 viewport。
                    viewport,
                    // partial 使用 Load，Full 使用透明 Clear。
                    composite.load,
                    // 每个 partial rect 对应一个裁剪 quad。
                    &composite.quads,
                )
            }
        };
        // test-harness / Agent 截屏把本次观察结果交给 Renderer 在 present 返回后消费。
        #[cfg(any(feature = "test-harness", feature = "agent-control"))]
        if readback_requested {
            // 钩子未执行时保持 None，Renderer 将优先报告最终 present 失败。
            self.surface_readback_result = readback_result;
        }
        // 最终合成失败时保留下一帧的全量重试边界。
        if result.is_err() {
            // 失败帧不能继续把当前 retained 内容当成已交付帧。
            self.surface.needs_gpu_clear = true;
            // 失败后等待 present 的标记必须重新进入明确状态。
            self.rhi_surface_frame_pending_present = false;
        }
        // 返回底层执行或 present 的真实结果。
        result
    }
}
