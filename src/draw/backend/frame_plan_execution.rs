//! FramePlan 的离屏执行边界。

// 引入统一错误和结果类型。
use crate::core::error::{Errc, Error, Result};
// 引入薄 RHI 的组合 context 与提交句柄。
use crate::native::present::rhi::{GraphicsContextRhi, SubmissionHandle};

// 引入父模块的计划私有结构。
use super::{FramePlan, FramePlanCommand, FramePlanStep, RenderTargetRef};

// 为 FramePlan 提供不触发 surface present 的离屏执行入口。
impl FramePlan {
    // 执行需要写入 acquired surface 但暂不触发 present 的有序片段。
    pub(crate) fn execute_surface_segment_on_context(
        &self,
        context: &mut dyn GraphicsContextRhi,
    ) -> Result<SubmissionHandle> {
        // 先验证计划，保证 malformed command 不触碰 native 资源。
        self.validate()?;
        // 检查 context 是否满足通用 GPU 基线。
        if let Some(missing) = context.capabilities().first_missing_gpu_baseline() {
            // 用 NotImplemented 稳定报告 adapter 的底层缺口。
            return Err(Error::new(
                Errc::NotImplemented,
                format!("GPU adapter is missing required capability: {missing}"),
            ));
        }
        // 旧代 surface 计划不能向当前 owner-thread context 写入。
        if context.token() != self.surface {
            // 返回 surface lost，而不是把代际错误伪装成资源错误。
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "surface segment generation is stale before acquire",
            ));
        }
        // 获取当前 acquired surface image，但把最终 present 留给上层边界。
        let frame = context.acquire()?;
        // 检查 acquire 返回的 image 代际。
        if frame.token != self.surface {
            // 迟到或错误代际的 image 不得接收当前计划。
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "acquired surface frame does not match segment generation",
            ));
        }
        // 按计划顺序执行所有 pass 和 copy。
        for step in &self.steps {
            // 分派当前顶层步骤。
            match step {
                // 执行一个可以包含 surface 或 texture target 的 render pass。
                FramePlanStep::Pass(pass) => {
                    // 把 surface target 解析为当前 acquired image。
                    let target = match pass.target {
                        // Surface 只允许绑定本次 acquire 的目标。
                        RenderTargetRef::Surface => frame.target,
                        // Texture target 直接沿用通用句柄。
                        RenderTargetRef::Texture(target) => target,
                    };
                    // 开始当前 pass。
                    context.begin_render_pass(target, pass.load)?;
                    // 顺序执行 pass 内低层命令。
                    for command in &pass.commands {
                        // 把 command 映射到薄 RHI。
                        match command {
                            // 设置 viewport。
                            FramePlanCommand::SetViewport(viewport) => {
                                // 交给 adapter 设置当前 viewport。
                                context.set_viewport(*viewport)?;
                            }
                            // 设置 scissor。
                            FramePlanCommand::SetScissor(scissor) => {
                                // 交给 adapter 设置当前 scissor。
                                context.set_scissor(*scissor)?;
                            }
                            // 执行一个不改变其它 pass 状态的局部清理。
                            FramePlanCommand::ClearRect { color, scissor } => {
                                // adapter 负责把清理颜色和矩形编码为原生命令。
                                context.clear_rect(*color, *scissor)?;
                            }
                            // 绑定 sampled texture 和 sampler。
                            FramePlanCommand::BindTexture {
                                slot,
                                texture,
                                sampler,
                            } => {
                                // 保持明确的 RHI slot 语义。
                                context.bind_texture(*slot, *texture, *sampler)?;
                            }
                            // 更新当前 pass 的 buffer 数据。
                            FramePlanCommand::UpdateBuffer {
                                buffer,
                                offset,
                                data,
                            } => {
                                // 载荷所有权由 FramePlan 保持到 update 完成。
                                context.update_buffer(*buffer, *offset, data)?;
                            }
                            // 执行已经完成语义降级的 draw packet。
                            FramePlanCommand::Draw(packet) => {
                                // adapter 只接收固定 ABI 的 draw。
                                context.draw(*packet)?;
                            }
                        }
                    }
                    // 结束 pass，允许后续 copy 或 pass 继续。
                    context.end_render_pass()?;
                }
                // 执行 pass 外纹理复制。
                FramePlanStep::Copy(copy) => {
                    // 复制原语由 adapter 校验资源状态和范围。
                    context.copy_texture(*copy)?;
                }
                // 执行一个重叠安全的纹理区域移动。
                FramePlanStep::Move(movement) => {
                    // retained framebuffer 移动由 adapter 提供 memmove 语义。
                    context.move_texture_region(*movement)?;
                }
            }
        }
        // 片段只提交 command，不触发 surface present。
        let submission = context.submit()?;
        // 提交后再次检查 surface 代际。
        if context.token() != frame.token {
            // 已提交但当前 acquired image 已失效，不能报告成功边界。
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "surface generation changed after segment submit",
            ));
        }
        // 返回提交身份，最终 present 由调用方统一完成。
        Ok(submission)
    }

    // 执行只包含离屏 target 的计划，并返回 device submit 身份。
    pub(crate) fn execute_offscreen_on_context(
        &self,
        context: &mut dyn GraphicsContextRhi,
    ) -> Result<SubmissionHandle> {
        // 先验证计划，保证 malformed command 不触碰 native 资源。
        self.validate()?;
        // 检查 device 是否满足通用 GPU 基线。
        if let Some(missing) = context.capabilities().first_missing_gpu_baseline() {
            // 用 NotImplemented 稳定报告 adapter 的底层缺口。
            return Err(Error::new(
                Errc::NotImplemented,
                format!("GPU adapter is missing required capability: {missing}"),
            ));
        }
        // 旧代 surface 计划不能向当前 owner-thread context 提交离屏工作。
        if context.token() != self.surface {
            // 返回 surface lost，而不是把代际错误伪装成资源错误。
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "offscreen FramePlan surface generation is stale before submit",
            ));
        }
        // 按计划顺序执行所有离屏 pass 和 pass 外 copy。
        for step in &self.steps {
            // 分派当前顶层步骤。
            match step {
                // 执行一个离屏 render pass。
                FramePlanStep::Pass(pass) => {
                    // 离屏执行禁止隐含 acquire 的 Surface 目标。
                    let RenderTargetRef::Texture(target) = pass.target else {
                        // 返回稳定的参数错误。
                        return Err(Error::new(
                            Errc::InvalidArgument,
                            "offscreen FramePlan cannot target the presentation surface",
                        ));
                    };
                    // 开始当前离屏 pass。
                    context.begin_render_pass(target, pass.load)?;
                    // 顺序执行 pass 内所有底层命令。
                    for command in &pass.commands {
                        // 分派当前命令到薄 RHI。
                        match command {
                            // 设置 viewport。
                            FramePlanCommand::SetViewport(viewport) => {
                                // 把 viewport 交给 adapter。
                                context.set_viewport(*viewport)?;
                            }
                            // 设置 scissor。
                            FramePlanCommand::SetScissor(scissor) => {
                                // 把 scissor 交给 adapter。
                                context.set_scissor(*scissor)?;
                            }
                            // 执行一个不改变其它 pass 状态的局部清理。
                            FramePlanCommand::ClearRect { color, scissor } => {
                                // adapter 负责把清理颜色和矩形编码为原生命令。
                                context.clear_rect(*color, *scissor)?;
                            }
                            // 绑定 sampled texture 和 sampler。
                            FramePlanCommand::BindTexture {
                                slot,
                                texture,
                                sampler,
                            } => {
                                // 保持 RHI slot 语义，不让 adapter 猜测绑定位置。
                                context.bind_texture(*slot, *texture, *sampler)?;
                            }
                            // 更新当前 pass 的 vertex/uniform 资源。
                            FramePlanCommand::UpdateBuffer {
                                buffer,
                                offset,
                                data,
                            } => {
                                // 载荷在计划中保持 Arc 所有权直到更新完成。
                                context.update_buffer(*buffer, *offset, data)?;
                            }
                            // 执行已经完成语义 lowering 的 draw packet。
                            FramePlanCommand::Draw(packet) => {
                                // adapter 只接收固定 ABI 的 packet。
                                context.draw(*packet)?;
                            }
                        }
                    }
                    // 结束离屏 pass，允许后续 copy 或 pass 继续。
                    context.end_render_pass()?;
                }
                // 执行 pass 外纹理复制。
                FramePlanStep::Copy(copy) => {
                    // 复制原语由 adapter 校验资源状态和范围。
                    context.copy_texture(*copy)?;
                }
                // 执行一个重叠安全的纹理区域移动。
                FramePlanStep::Move(movement) => {
                    // retained framebuffer 移动由 adapter 提供 memmove 语义。
                    context.move_texture_region(*movement)?;
                }
            }
        }
        // 离屏计划只提交命令，不获取或呈现主 surface。
        let submission = context.submit()?;
        // 提交后再次检查 surface 代际，避免调用方继续使用失效资源。
        if context.token() != self.surface {
            // submit 已发生但没有可消费的最终 frame commit。
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "offscreen FramePlan surface generation changed after submit",
            ));
        }
        // 返回离屏工作的提交身份。
        Ok(submission)
    }
}
