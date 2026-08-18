//! FramePlan 的离屏执行边界。

// 引入统一错误和结果类型。
use crate::core::error::{Errc, Error, Result};
// 引入薄 RHI 的 Device、Surface、组合 context、目标句柄与提交句柄。
use crate::native::present::rhi::{
    GraphicsContextRhi, GraphicsDevice, GraphicsSurface, RenderTargetHandle, RhiBufferUpload,
    RhiPresentTransaction, SubmissionHandle,
};

// 引入父模块的计划私有结构。
use super::{
    FrameCommit, FramePlan, FramePlanCommand, FramePlanStep, RenderPassPlan, RenderTargetRef,
};

// 描述唯一命令执行器允许解析的 render target 范围。
#[derive(Debug, Clone, Copy)]
enum FramePlanTargetMode {
    // 把逻辑 Surface 解析为当前 acquire 得到的不透明目标。
    AcquiredSurface(RenderTargetHandle),
    // 禁止逻辑 Surface，只允许显式 texture target。
    OffscreenOnly,
}

// 把已经验证的 FramePlan 步骤唯一映射到一个薄 RHI device。
pub(super) struct FramePlanExecutor<'device, D>
where
    // 支持独立 device 和同时实现 device/surface 的组合 context。
    D: GraphicsDevice + ?Sized,
{
    // 借用当前 owner-thread device，但不取得其资源生命周期。
    device: &'device mut D,
    // 冻结本次执行允许使用的目标解析方式。
    target_mode: FramePlanTargetMode,
}

// 为唯一 FramePlan 命令执行组件提供目标校验与有序分派。
impl<'device, D> FramePlanExecutor<'device, D>
where
    // 只依赖 GraphicsDevice 窄契约，不依赖 surface 或原生 Adapter。
    D: GraphicsDevice + ?Sized,
{
    // 创建允许写入当前 acquired surface 的执行器。
    pub(super) fn for_surface(device: &'device mut D, target: RenderTargetHandle) -> Self {
        // 绑定当前帧唯一的 surface target。
        Self {
            // 保存执行期间的独占 device 借用。
            device,
            // 记录 acquire 已经解析出的不透明目标。
            target_mode: FramePlanTargetMode::AcquiredSurface(target),
        }
    }

    // 创建只允许写入显式离屏纹理的执行器。
    pub(super) fn offscreen(device: &'device mut D) -> Self {
        // 不取得主 surface image 或 present 能力。
        Self {
            // 保存执行期间的独占 device 借用。
            device,
            // 固定拒绝所有逻辑 Surface target。
            target_mode: FramePlanTargetMode::OffscreenOnly,
        }
    }

    // 在触碰 device 前校验目标模式，再按计划顺序执行全部步骤。
    pub(super) fn execute(mut self, steps: &[FramePlanStep]) -> Result<()> {
        // 先完成执行模式专属校验，避免离屏错误产生部分 native 副作用。
        self.validate_targets(steps)?;
        // 在第一条原生命令前激活当前 owner；OpenGL 在此恢复正确 context。
        self.device.activate()?;
        // 激活成功后再执行设备健康 preflight，失败计划不得进入任何命令。
        self.device.maintain()?;
        // 严格保持 FramePlan 已经确定的步骤顺序。
        for step in steps {
            // 把每类顶层步骤映射到唯一薄 RHI 调用路径。
            match step {
                // render pass 由同一个 pass 执行函数处理。
                FramePlanStep::Pass(pass) => self.execute_pass(pass)?,
                // pass 外纹理复制直接交给 device。
                FramePlanStep::Copy(copy) => self.device.copy_texture(*copy)?,
                // retained 区域移动保持重叠安全语义。
                FramePlanStep::Move(movement) => {
                    // 只调用 device 声明的统一移动原语。
                    self.device.move_texture_region(*movement)?;
                }
            }
        }
        // 全部步骤均已按顺序交付 device。
        Ok(())
    }

    // 校验 offscreen 模式不会在执行中途遇到逻辑 Surface target。
    fn validate_targets(&self, steps: &[FramePlanStep]) -> Result<()> {
        // 检查任一 pass 是否错误地请求主 surface。
        let contains_surface = steps.iter().any(|step| {
            // 只匹配 render pass 中的逻辑 Surface target。
            matches!(
                step,
                FramePlanStep::Pass(pass) if matches!(pass.target, RenderTargetRef::Surface)
            )
        });
        // 离屏执行不得隐含 acquire 或主 surface 写入。
        if matches!(self.target_mode, FramePlanTargetMode::OffscreenOnly) && contains_surface {
            // 在任何 device 调用前返回稳定参数错误。
            return Err(Error::new(
                Errc::InvalidArgument,
                "offscreen FramePlan cannot target the presentation surface",
            ));
        }
        // 在激活 device 前解析所有离屏 texture target 的共享能力。
        for step in steps {
            // 只读取 render pass 的逻辑目标，不触碰其它顶层命令。
            if let FramePlanStep::Pass(pass) = step {
                // 只有 texture target 需要向资源 owner 查询真实描述。
                if let RenderTargetRef::Texture(texture) = pass.target {
                    // 共享资源表必须在任何 native side effect 前证明目标能力。
                    self.device.resolve_render_target(texture)?;
                }
            }
        }
        // 所有 pass 都显式指向离屏 texture。
        Ok(())
    }

    // 执行一个完整 render pass，并保证命令不会跨过 pass 边界。
    fn execute_pass(&mut self, pass: &RenderPassPlan) -> Result<()> {
        // 按执行模式解析通用 target 身份。
        let target = self.resolve_target(pass.target)?;
        // 使用计划冻结的 load action 开始 pass。
        self.device.begin_render_pass(target, pass.load)?;
        // 严格保持 pass 内命令的 painter order。
        for command in &pass.commands {
            // 所有提交模式复用同一个命令分派函数，并在失败时进入共同收尾。
            if let Err(command_error) = self.execute_command(command) {
                // pass 已成功开始，任何中途错误都必须尝试清除共享和原生状态。
                if let Err(cleanup_error) = self.device.end_render_pass() {
                    // cleanup 失败不能覆盖最初的命令根因，但必须留下明确诊断。
                    tracing::error!(
                        // 同时记录主错误和收尾错误，便于判断 Device 是否需要重建。
                        "FramePlan render pass cleanup failed after command error: primary={}; cleanup={}",
                        // 保留最初失败命令的稳定诊断。
                        command_error.what(),
                        // 保留 Adapter 收尾失败诊断。
                        cleanup_error.what(),
                    );
                }
                // 返回最初的命令错误，禁止 cleanup 结果改变恢复分类。
                return Err(command_error);
            }
        }
        // 显式结束 pass，禁止后续 copy 或 pass 嵌套。
        self.device.end_render_pass()?;
        // 当前 pass 已完整交付 device。
        Ok(())
    }

    // 把一条已验证命令映射为唯一的薄 RHI device 调用。
    fn execute_command(&mut self, command: &FramePlanCommand) -> Result<()> {
        // 分派当前命令而不引入任何 API 或平台判断。
        match command {
            // 设置统一的 top-left viewport。
            FramePlanCommand::SetViewport(viewport) => self.device.set_viewport(*viewport),
            // 设置或清除统一的 top-left scissor。
            FramePlanCommand::SetScissor(scissor) => self.device.set_scissor(*scissor),
            // 执行不改变其它 pass 状态的局部清理。
            FramePlanCommand::ClearRect { color, scissor } => {
                // 将规范颜色和物理矩形一起交给 Adapter。
                self.device.clear_rect(*color, *scissor)
            }
            // 绑定已经由计划冻结的原子 sampled resource。
            FramePlanCommand::BindSampledTexture(binding) => {
                // Adapter 只解析值对象中的两个不透明资源句柄。
                self.device.bind_sampled_texture(*binding)
            }
            // 在唯一执行边界把类型化顶点编码为 Device 原语所需字节。
            FramePlanCommand::UploadVertex { buffer, data } => {
                // 字节表示不再进入 FramePlan 存储或上层 renderer。
                let bytes = data.encode_ne_bytes();
                // 将资源身份与从零开始的类型化载荷绑定成唯一上传命令。
                self.device
                    .update_buffer(RhiBufferUpload::new(*buffer, &bytes))
            }
            // 在唯一执行边界把类型化 Uniform 编码为固定 ABI 字节。
            FramePlanCommand::UploadUniform { buffer, data } => {
                // 每个 Uniform 变体只调用共享值对象拥有的编码器。
                let bytes = data.encode_ne_bytes();
                // Uniform 更新通过同一上传值对象进入完整替换门禁。
                self.device
                    .update_buffer(RhiBufferUpload::new(*buffer, &bytes))
            }
            // 执行已经完成高层语义 lowering 的 draw packet。
            FramePlanCommand::Draw(packet) => self.device.draw(*packet),
        }
    }

    // 把通用 target 引用解析为本次 device 调用使用的不透明句柄。
    fn resolve_target(&self, target: RenderTargetRef) -> Result<RenderTargetHandle> {
        // 只允许 acquired surface 或显式 texture 两种确定映射。
        match (self.target_mode, target) {
            // 当前 acquired image 是逻辑 Surface 的唯一物理目标。
            (FramePlanTargetMode::AcquiredSurface(surface), RenderTargetRef::Surface) => {
                // 返回 acquire 交付的不透明 target。
                Ok(surface)
            }
            // 显式 texture target 在唯一 Device 边界提升为 render target。
            (_, RenderTargetRef::Texture(texture)) => self.device.resolve_render_target(texture),
            // offscreen 模式的 Surface 已由前置校验拒绝，此分支保留防御。
            (FramePlanTargetMode::OffscreenOnly, RenderTargetRef::Surface) => Err(Error::new(
                Errc::InvalidArgument,
                "offscreen FramePlan cannot target the presentation surface",
            )),
        }
    }
}

// 为 FramePlan 提供不触发 surface present 的离屏执行入口。
impl FramePlan {
    // 在唯一 submit 成功后、最终 present 之前执行一个 owner-thread 观察钩子。
    pub(crate) fn execute_on_context_with_before_present(
        // 借用当前不可变帧计划。
        &self,
        // 借用同时拥有 device 与 surface 的原生 context。
        context: &mut dyn GraphicsContextRhi,
        // 钩子只能观察或回读 Surface，不能重新取得 Device 命令能力。
        before_present: &mut dyn FnMut(&mut dyn GraphicsSurface),
        // 返回仍以最终 present 成功为提交点的 FrameCommit。
    ) -> Result<FrameCommit> {
        // 统一验证计划与组合 context 暴露的窄 device 能力。
        self.validate_for_device(context.device_ref())?;
        // 原子取得计划生成时冻结的 Surface 代际与最终 present damage。
        let (plan_surface, present_damage) = self.required_surface()?;
        // 检查计划生成时观察到的 surface 代际。
        if context.surface_ref().token() != plan_surface {
            // 旧计划不能写入新一代 surface。
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "FramePlan surface generation is stale before acquire",
            ));
        }
        // 获取唯一的本帧 surface image。
        let frame = context.surface().acquire()?;
        // 检查 acquire 返回的 image 代际。
        if frame.token() != plan_surface {
            // 迟到或错误代际的 image 不得接收当前计划。
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "acquired surface frame does not match FramePlan generation",
            ));
        }
        // 组合 context 只把窄 device 角色交给唯一命令执行组件。
        FramePlanExecutor::for_surface(context.device(), frame.target()).execute(&self.steps)?;
        // 所有内部工作完成后只允许一次 device submit。
        let submission = context.device().submit()?;
        // 检查执行期间 surface 是否被重建。
        if context.surface_ref().token() != frame.token() {
            // 不把旧 image 的 submit 当作当前 surface 的成功 present。
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "surface generation changed before final present",
            ));
        }
        // 此时全部绘制命令已经提交，而 Surface 尚未进入 swap/present。
        before_present(
            // 观察器只取得窄 Surface 角色，无法追加 Device 命令或二次 submit。
            context.surface(),
        );
        // 观察钩子不得使当前 surface 代际失效。
        if context.surface_ref().token() != frame.token() {
            // 代际变化仍按 surface lost 进入统一恢复层。
            return Err(Error::new(
                // 使用稳定的 surface 生命周期分类。
                Errc::GraphicsSurfaceLost,
                // 明确失败发生在提交与最终 present 之间。
                "surface generation changed at the before-present boundary",
            ));
        }
        // 最终 present 是唯一提交成功边界。
        context
            // 最终阶段只借用 Surface 角色消费 frame 与 submission。
            .surface()
            // Surface Adapter 只接收绑定 frame、submit 与 damage 的完整事务。
            .present(RhiPresentTransaction::new(
                // 绑定 acquire 返回的同代际 image。
                frame,
                // 绑定 Device 刚签发的提交身份。
                submission,
                // 绑定本帧唯一最终 damage。
                present_damage,
            ))?;
        // 只有 present 成功后才返回可消费 damage 的 commit。
        Ok(FrameCommit {
            // 保存已经成功呈现的 surface 代际。
            surface: plan_surface,
            // 保存 device 返回的提交身份。
            submission,
        })
    }

    // 在独立 device 上执行只包含离屏 target 的计划。
    pub(crate) fn execute_offscreen_on_device<D>(&self, device: &mut D) -> Result<SubmissionHandle>
    where
        // 离屏执行只依赖资源、命令和 submit 原语。
        D: GraphicsDevice + ?Sized,
    {
        // 统一验证计划与 device 基线。
        self.validate_for_device(device)?;
        // Surface 计划不能通过 device-only 入口绕过 acquire/present。
        if self.targets_surface() {
            // 在任何 device 副作用前返回稳定作用域错误。
            return Err(Error::new(
                Errc::InvalidArgument,
                "surface FramePlan cannot enter the offscreen device boundary",
            ));
        }
        // 唯一执行组件在任何 device 调用前拒绝逻辑 Surface target。
        FramePlanExecutor::offscreen(device).execute(&self.steps)?;
        // 离屏计划只提交命令，不获取或呈现主 surface。
        let submission = device.submit()?;
        // 返回离屏工作的提交身份。
        Ok(submission)
    }
}
