//! 通用 GPU Renderer 的有限、有序帧计划。
//!
//! `FramePlan` 只承载已经完成 UI 语义降级的 pass、copy 和 draw packet；
//! 原生 adapter 不能从这里取得 widget、字体或路径算法。

#![allow(dead_code)]
// 使用共享字节载荷表达帧内资源更新，并保持计划可克隆审计。
use std::sync::Arc;
// 引入框架错误类型，保证执行失败不会被当作成功帧。
use crate::core::error::{Errc, Error, Result};
// 引入最终呈现的 damage 值。
use crate::core::PresentDamage;
// 引入 platform 私有的薄 RHI 原语。
use crate::native::present::rhi::{
    DrawPacket, GraphicsContextRhi, GraphicsDevice, GraphicsSurface, LoadAction,
    RenderTargetHandle, RhiColor, RhiScissor, RhiViewport, SubmissionHandle, SurfaceToken,
    TextureCopy, TextureHandle, TextureMove,
};
// 将不触发 surface present 的离屏执行边界拆到独立文件。
#[path = "frame_plan_execution.rs"]
mod execution;
// 描述 render pass 目标，surface 目标在 acquire 后才绑定具体句柄。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RenderTargetRef {
    // 指向当前 acquired surface image。
    Surface,
    // 指向 device 管理的离屏 render target。
    Texture(RenderTargetHandle),
}

// 描述 pass 内已经排序的低层命令。
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FramePlanCommand {
    // 设置 pass 的 viewport。
    SetViewport(RhiViewport),
    // 设置 pass 的 scissor。
    SetScissor(Option<RhiScissor>),
    // 在 pass 内按明确物理矩形清理 premultiplied-alpha 颜色。
    ClearRect {
        // 保存局部清理颜色。
        color: RhiColor,
        // 保存左上原点的物理清理区域。
        scissor: RhiScissor,
    },
    // 绑定一个通用采样资源。
    BindTexture {
        // 保存 shader binding slot。
        slot: u32,
        // 保存被采样纹理。
        texture: TextureHandle,
        // 保存采样器。
        sampler: crate::native::present::rhi::SamplerHandle,
    },
    // 在 pass 内按 painter order 更新一个已创建的 buffer。
    UpdateBuffer {
        // 保存目标 buffer。
        buffer: crate::native::present::rhi::BufferHandle,
        // 保存字节偏移。
        offset: usize,
        // 保存紧密排列的上传数据。
        data: Arc<[u8]>,
    },
    // 执行一个已完成几何降级的 draw packet。
    Draw(DrawPacket),
}

// 描述一个按 painter order 执行的 render pass。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RenderPassPlan {
    // 保存 pass 的目标。
    target: RenderTargetRef,
    // 保存 pass 的加载动作。
    load: LoadAction,
    // 保存 pass 内有序命令。
    commands: Vec<FramePlanCommand>,
}

// 为 render pass 提供构造和命令追加入口。
impl RenderPassPlan {
    // 创建一个空的 render pass。
    pub(crate) fn new(target: RenderTargetRef, load: LoadAction) -> Self {
        // 返回尚未追加 draw 命令的 pass。
        Self {
            target,
            load,
            commands: Vec::new(),
        }
    }

    // 追加一个保持顺序的低层命令。
    pub(crate) fn push(&mut self, command: FramePlanCommand) {
        // 让通用 renderer 明确控制 painter order。
        self.commands.push(command);
    }
}

// 描述一帧的顶层步骤。
#[derive(Debug, Clone, PartialEq)]
enum FramePlanStep {
    // 描述一个 render pass。
    Pass(RenderPassPlan),
    // 描述 pass 外的纹理复制。
    Copy(TextureCopy),
    // 描述 pass 外、对重叠区域安全的纹理移动。
    Move(TextureMove),
}

// 描述一次已经成功最终呈现的提交。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FrameCommit {
    // 保存本次提交使用的 surface token。
    pub(crate) surface: SurfaceToken,
    // 保存 device 返回的 submit 身份。
    pub(crate) submission: SubmissionHandle,
}

// 持有一帧有限且有序的 GPU 执行计划。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FramePlan {
    // 保存计划生成时观察到的 surface token。
    surface: SurfaceToken,
    // 保存最终呈现时提交的 damage。
    damage: PresentDamage,
    // 保存 pass 和 copy 的严格执行顺序。
    steps: Vec<FramePlanStep>,
}

// 为 FramePlan 提供构造、验证和执行入口。
impl FramePlan {
    // 创建一帧空计划。
    pub(crate) fn new(surface: SurfaceToken, damage: PresentDamage) -> Self {
        // 返回等待通用 renderer 追加步骤的计划。
        Self {
            surface,
            damage,
            steps: Vec::new(),
        }
    }

    // 追加一个 render pass。
    pub(crate) fn push_pass(&mut self, pass: RenderPassPlan) {
        // 保留调用方已经确定的 pass 顺序。
        self.steps.push(FramePlanStep::Pass(pass));
    }

    // 追加一个 pass 外纹理复制。
    pub(crate) fn push_copy(&mut self, copy: TextureCopy) {
        // 保留资源复制和 pass 之间的严格顺序。
        self.steps.push(FramePlanStep::Copy(copy));
    }

    // 追加一个具有 memmove 语义的纹理区域移动。
    pub(crate) fn push_move(&mut self, movement: TextureMove) {
        // 保留 retained framebuffer 的移动与 pass 之间的严格顺序。
        self.steps.push(FramePlanStep::Move(movement));
    }

    // 验证计划是否可以进入 native adapter。
    pub(crate) fn validate(&self) -> Result<()> {
        // 拒绝无效 surface extent，避免 adapter 接受零尺寸 target。
        if !self.surface.extent.is_positive() {
            // 返回稳定的参数错误。
            return Err(Error::new(
                Errc::InvalidArgument,
                "FramePlan surface extent must be positive",
            ));
        }
        // 计划必须至少包含一个 render pass。
        if !self
            .steps
            .iter()
            .any(|step| matches!(step, FramePlanStep::Pass(_)))
        {
            // 没有 pass 的计划不能形成可呈现帧。
            return Err(Error::new(
                Errc::InvalidArgument,
                "FramePlan must contain at least one render pass",
            ));
        }
        // 逐项检查 pass 和 copy 的通用不变量。
        for step in &self.steps {
            // 分派当前顶层步骤的验证逻辑。
            match step {
                // 检查 render pass 内容。
                FramePlanStep::Pass(pass) => {
                    // 空 pass 没有可观察的绘制语义，通常是上层 lowering 错误。
                    if pass.commands.is_empty() {
                        // 返回稳定的参数错误。
                        return Err(Error::new(
                            Errc::InvalidArgument,
                            "FramePlan render pass must contain commands",
                        ));
                    }
                    // 检查 pass 的清理颜色和每个低层命令。
                    if let LoadAction::Clear(color) = pass.load {
                        // 清理颜色必须保持有限。
                        if !color.is_finite() {
                            // 返回稳定的参数错误。
                            return Err(Error::new(
                                Errc::InvalidArgument,
                                "FramePlan clear color must be finite",
                            ));
                        }
                    }
                    // 验证 pass 内的命令顺序元素。
                    for command in &pass.commands {
                        // 只拒绝不能产生任何几何的 draw packet。
                        if let FramePlanCommand::Draw(packet) = command {
                            // 空 draw packet 会掩盖 renderer 的 lowering 缺口。
                            if !packet.is_non_empty() {
                                // 返回稳定的参数错误。
                                return Err(Error::new(
                                    Errc::InvalidArgument,
                                    "FramePlan draw packet must be non-empty",
                                ));
                            }
                        }
                        // 验证 viewport 不携带非有限或零尺寸。
                        if let FramePlanCommand::SetViewport(viewport) = command {
                            // 无效 viewport 会让不同 adapter 产生不同裁剪结果。
                            if !viewport.is_valid() {
                                // 返回稳定的参数错误。
                                return Err(Error::new(
                                    Errc::InvalidArgument,
                                    "FramePlan viewport must be finite and positive",
                                ));
                            }
                        }
                        // 验证显式 scissor 的坐标和尺寸。
                        if let FramePlanCommand::SetScissor(Some(scissor)) = command {
                            // 无效 scissor 不能交给原生 rasterizer 猜测。
                            if !scissor.is_valid() {
                                // 返回稳定的参数错误。
                                return Err(Error::new(
                                    Errc::InvalidArgument,
                                    "FramePlan scissor must have a non-negative positive extent",
                                ));
                            }
                        }
                        // 验证局部清理的颜色和物理矩形。
                        if let FramePlanCommand::ClearRect { color, scissor } = command {
                            // 局部清理必须保持有限颜色和正数区域。
                            if !color.is_finite() || !scissor.is_valid() {
                                // 返回稳定的参数错误，避免 adapter 产生未定义清理范围。
                                return Err(Error::new(
                                    Errc::InvalidArgument,
                                    "FramePlan clear rect must be finite and positive",
                                ));
                            }
                        }
                        // 验证 binding slot 不超过 UIX 预留的有限范围。
                        if let FramePlanCommand::BindTexture { slot, .. } = command {
                            // 统一限制避免 adapter 依赖 API 的 slot 上限。
                            if *slot >= 32 {
                                // 返回稳定的参数错误。
                                return Err(Error::new(
                                    Errc::InvalidArgument,
                                    "FramePlan texture binding slot is out of range",
                                ));
                            }
                        }
                        // 验证 buffer 更新不会把空载荷交给 adapter。
                        if let FramePlanCommand::UpdateBuffer { data, .. } = command {
                            // 空更新通常意味着 renderer 没有生成完整 uniform/vertex 数据。
                            if data.is_empty() {
                                // 返回稳定的参数错误。
                                return Err(Error::new(
                                    Errc::InvalidArgument,
                                    "FramePlan buffer update payload must be non-empty",
                                ));
                            }
                        }
                    }
                }
                // 检查纹理复制区域。
                FramePlanStep::Copy(copy) => {
                    // 复制宽高必须严格大于零。
                    if copy.width == 0 || copy.height == 0 {
                        // 返回稳定的参数错误。
                        return Err(Error::new(
                            Errc::InvalidArgument,
                            "FramePlan texture copy extent must be positive",
                        ));
                    }
                }
                // 检查重叠安全的纹理区域移动。
                FramePlanStep::Move(movement) => {
                    // 移动宽高必须严格大于零。
                    if movement.width == 0 || movement.height == 0 {
                        // 返回稳定的参数错误。
                        return Err(Error::new(
                            Errc::InvalidArgument,
                            "FramePlan texture move extent must be positive",
                        ));
                    }
                }
            }
        }
        // 所有计划不变量通过。
        Ok(())
    }
    // 在一个 device 和一个 surface 上执行计划，并隐含唯一最终 present。
    pub(crate) fn execute(
        &self,
        device: &mut dyn GraphicsDevice,
        surface: &mut dyn GraphicsSurface,
    ) -> Result<FrameCommit> {
        // 先验证计划，保证失败不会触碰 native 资源。
        self.validate()?;
        // 检查 device 是否满足进入正常 GPU 流程的底层基线。
        let capabilities = device.capabilities();
        if let Some(missing) = capabilities.first_missing_gpu_baseline() {
            // 用 NotImplemented 表示 adapter 缺少 RHI 基线，而不是 UI 操作缺失。
            return Err(Error::new(
                Errc::NotImplemented,
                format!("GPU adapter is missing required capability: {missing}"),
            ));
        }
        // 检查计划生成时的 surface 代际。
        if surface.token() != self.surface {
            // 旧计划不能写入新一代 surface。
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "FramePlan surface generation is stale before acquire",
            ));
        }
        // 获取唯一的本帧 surface image。
        let frame = surface.acquire()?;
        // 检查 acquire 返回的 image 代际。
        if frame.token != self.surface {
            // 迟到或错误代际的 image 不得接收当前计划。
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "acquired surface frame does not match FramePlan generation",
            ));
        }
        // 按计划顺序执行所有 pass 和 copy。
        for step in &self.steps {
            // 分派当前步骤到薄 RHI。
            match step {
                // 执行一个完整 render pass。
                FramePlanStep::Pass(pass) => {
                    // 把逻辑 surface 目标解析为本次 acquire 的 opaque target。
                    let target = match pass.target {
                        // surface target 只绑定当前 acquired image。
                        RenderTargetRef::Surface => frame.target,
                        // 离屏 target 直接沿用通用句柄。
                        RenderTargetRef::Texture(target) => target,
                    };
                    // 开始 pass，并保留 load action 语义。
                    device.begin_render_pass(target, pass.load)?;
                    // 逐条执行 pass 内命令。
                    for command in &pass.commands {
                        // 分派每个低层命令。
                        match command {
                            // 设置 viewport。
                            FramePlanCommand::SetViewport(viewport) => {
                                // 将 viewport 交给 adapter。
                                device.set_viewport(*viewport)?;
                            }
                            // 设置 scissor。
                            FramePlanCommand::SetScissor(scissor) => {
                                // 将 scissor 交给 adapter。
                                device.set_scissor(*scissor)?;
                            }
                            // 执行一个不改变其它 pass 状态的局部清理。
                            FramePlanCommand::ClearRect { color, scissor } => {
                                // adapter 负责把清理颜色和矩形编码为原生命令。
                                device.clear_rect(*color, *scissor)?;
                            }
                            // 绑定纹理和采样器。
                            FramePlanCommand::BindTexture {
                                slot,
                                texture,
                                sampler,
                            } => {
                                // 将通用资源绑定交给 adapter。
                                device.bind_texture(*slot, *texture, *sampler)?;
                            }
                            // 更新当前 pass 需要的 vertex/uniform 数据。
                            FramePlanCommand::UpdateBuffer {
                                buffer,
                                offset,
                                data,
                            } => {
                                // 上传载荷由通用 renderer 预先生成并保持不可变。
                                device.update_buffer(*buffer, *offset, data)?;
                            }
                            // 执行 draw packet。
                            FramePlanCommand::Draw(packet) => {
                                // adapter 只接收已经完成语义降级的 packet。
                                device.draw(*packet)?;
                            }
                        }
                    }
                    // 结束 pass，确保后续 copy 或 pass 不发生嵌套。
                    device.end_render_pass()?;
                }
                // 执行 pass 外纹理复制。
                FramePlanStep::Copy(copy) => {
                    // 将复制原语交给 adapter。
                    device.copy_texture(*copy)?;
                }
                // 执行一个重叠安全的纹理区域移动。
                FramePlanStep::Move(movement) => {
                    // 将 retained framebuffer 移动原语交给 adapter。
                    device.move_texture_region(*movement)?;
                }
            }
        }
        // 所有内部工作完成后只允许一次 device submit。
        let submission = device.submit()?;
        // 检查执行期间 surface 是否被重建。
        if surface.token() != frame.token {
            // 不把旧 image 的 submit 当作当前 surface 的成功 present。
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "surface generation changed before final present",
            ));
        }
        // 最终 present 是唯一提交成功边界。
        surface.present(frame, submission, self.damage.clone())?;
        // 只有 present 成功后才返回可消费 damage 的 commit。
        Ok(FrameCommit {
            surface: self.surface,
            submission,
        })
    }
    // 在一个同时拥有 device 与 surface 的原生 context 上执行计划。
    pub(crate) fn execute_on_context(
        &self,
        context: &mut dyn GraphicsContextRhi,
    ) -> Result<FrameCommit> {
        // 先验证计划，保证失败不会触碰 native 资源。
        self.validate()?;
        // 检查 context 是否满足进入正常 GPU 流程的底层基线。
        if let Some(missing) = context.capabilities().first_missing_gpu_baseline() {
            // 用 NotImplemented 表示 adapter 缺少 RHI 基线。
            return Err(Error::new(
                Errc::NotImplemented,
                format!("GPU adapter is missing required capability: {missing}"),
            ));
        }
        // 检查计划生成时观察到的 surface 代际。
        if context.token() != self.surface {
            // 旧计划不能写入新一代 surface。
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "FramePlan surface generation is stale before acquire",
            ));
        }
        // 获取唯一的本帧 surface image。
        let frame = context.acquire()?;
        // 检查 acquire 返回的 image 代际。
        if frame.token != self.surface {
            // 迟到或错误代际的 image 不得接收当前计划。
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "acquired surface frame does not match FramePlan generation",
            ));
        }
        // 按计划顺序执行所有 pass 和 copy。
        for step in &self.steps {
            // 分派当前顶层步骤的验证逻辑。
            match step {
                // 执行一个完整 render pass。
                FramePlanStep::Pass(pass) => {
                    // 把 surface 目标解析为本次 acquire 的 opaque target。
                    let target = match pass.target {
                        // surface target 只绑定当前 acquired image。
                        RenderTargetRef::Surface => frame.target,
                        // 离屏 target 直接沿用通用句柄。
                        RenderTargetRef::Texture(target) => target,
                    };
                    // 开始 pass，并保留 load action 语义。
                    context.begin_render_pass(target, pass.load)?;
                    // 逐条执行 pass 内命令。
                    for command in &pass.commands {
                        // 分派每个低层命令。
                        match command {
                            // 设置 viewport。
                            FramePlanCommand::SetViewport(viewport) => {
                                // 将 viewport 交给 adapter。
                                context.set_viewport(*viewport)?;
                            }
                            // 设置 scissor。
                            FramePlanCommand::SetScissor(scissor) => {
                                // 将 scissor 交给 adapter。
                                context.set_scissor(*scissor)?;
                            }
                            // 执行一个不改变其它 pass 状态的局部清理。
                            FramePlanCommand::ClearRect { color, scissor } => {
                                // adapter 负责把清理颜色和矩形编码为原生命令。
                                context.clear_rect(*color, *scissor)?;
                            }
                            // 绑定纹理和采样器。
                            FramePlanCommand::BindTexture {
                                slot,
                                texture,
                                sampler,
                            } => {
                                // 将通用资源绑定交给 adapter。
                                context.bind_texture(*slot, *texture, *sampler)?;
                            }
                            // 更新当前 pass 需要的 vertex/uniform 数据。
                            FramePlanCommand::UpdateBuffer {
                                buffer,
                                offset,
                                data,
                            } => {
                                // 上传载荷由通用 renderer 预先生成并保持不可变。
                                context.update_buffer(*buffer, *offset, data)?;
                            }
                            // 执行一个已经完成语义降级的 draw packet。
                            FramePlanCommand::Draw(packet) => {
                                // adapter 只接收已经完成高层降级的 packet。
                                context.draw(*packet)?;
                            }
                        }
                    }
                    // 结束 pass，确保后续 copy 或 pass 不发生嵌套。
                    context.end_render_pass()?;
                }
                // 执行 pass 外纹理复制。
                FramePlanStep::Copy(copy) => {
                    // 将复制原语交给 adapter。
                    context.copy_texture(*copy)?;
                }
                // 执行一个重叠安全的纹理区域移动。
                FramePlanStep::Move(movement) => {
                    // 将 retained framebuffer 移动原语交给 adapter。
                    context.move_texture_region(*movement)?;
                }
            }
        }
        // 所有内部工作完成后只允许一次 device submit。
        let submission = context.submit()?;
        // 检查执行期间 surface 是否被重建。
        if context.token() != frame.token {
            // 不把旧 image 的 submit 当作当前 surface 的成功 present。
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "surface generation changed before final present",
            ));
        }
        // 最终 present 是唯一提交成功边界。
        context.present(frame, submission, self.damage.clone())?;
        // 只有 present 成功后才返回可消费 damage 的 commit。
        Ok(FrameCommit {
            surface: self.surface,
            submission,
        })
    }
}

#[cfg(test)]
mod tests {
    // 引入测试用集合，记录 RHI 的执行顺序。
    use std::collections::VecDeque;
    // 引入测试用共享上传载荷。
    use std::sync::Arc;

    // 引入框架错误类型和最终 damage。
    use crate::core::error::{Errc, Error, Result};
    // 引入测试计划依赖的薄 RHI 类型。
    use crate::native::present::rhi::{
        BufferHandle, DrawPacket, GraphicsCapabilities, GraphicsDevice, GraphicsSurface,
        LoadAction, PipelineHandle, RenderTargetHandle, RhiColor, RhiExtent, RhiScissor,
        RhiViewport, SubmissionHandle, SurfaceFrame, SurfaceToken, TextureCopy, TextureHandle,
        TextureMove,
    };
    // 引入当前文件的计划类型。
    use super::{FramePlan, FramePlanCommand, RenderPassPlan, RenderTargetRef};

    // 记录一个不会触碰真实图形 API 的 mock device。
    struct RecordingDevice {
        // 保存薄 RHI 调用顺序。
        log: VecDeque<&'static str>,
        // 保存测试适配器报告的底层能力。
        capabilities: GraphicsCapabilities,
        // 保存是否强制 submit 失败。
        fail_submit: bool,
    }

    // 为记录型 device 实现薄 RHI 的执行原语。
    impl GraphicsDevice for RecordingDevice {
        // 返回完整 GPU 基线，测试重点放在顺序而不是能力拒绝。
        fn capabilities(&self) -> GraphicsCapabilities {
            // 返回测试实例配置的能力快照。
            self.capabilities
        }

        // 记录 render pass 开始。
        fn begin_render_pass(
            &mut self,
            _target: RenderTargetHandle,
            _load: LoadAction,
        ) -> Result<()> {
            // 记录调用事件。
            self.log.push_back("begin_pass");
            // 返回成功。
            Ok(())
        }

        // 记录 viewport 设置。
        fn set_viewport(&mut self, _viewport: RhiViewport) -> Result<()> {
            // 记录调用事件。
            self.log.push_back("viewport");
            // 返回成功。
            Ok(())
        }

        // 记录 scissor 设置。
        fn set_scissor(&mut self, _scissor: Option<RhiScissor>) -> Result<()> {
            // 记录调用事件。
            self.log.push_back("scissor");
            // 返回成功。
            Ok(())
        }

        // 记录局部清理，验证 retained damage 不会被 adapter 静默丢弃。
        fn clear_rect(&mut self, _color: RhiColor, _scissor: RhiScissor) -> Result<()> {
            // 记录调用事件。
            self.log.push_back("clear_rect");
            // 返回成功。
            Ok(())
        }

        // 记录纹理绑定。
        fn bind_texture(
            &mut self,
            _slot: u32,
            _texture: TextureHandle,
            _sampler: crate::native::present::rhi::SamplerHandle,
        ) -> Result<()> {
            // 记录调用事件。
            self.log.push_back("bind_texture");
            // 返回成功。
            Ok(())
        }

        // 记录 buffer 上传，验证它位于 draw 前且不会被 adapter 忽略。
        fn update_buffer(
            &mut self,
            _buffer: BufferHandle,
            _offset: usize,
            _data: &[u8],
        ) -> Result<()> {
            // 记录调用事件。
            self.log.push_back("update_buffer");
            // 返回成功。
            Ok(())
        }

        // 记录 draw packet。
        fn draw(&mut self, _packet: DrawPacket) -> Result<()> {
            // 记录调用事件。
            self.log.push_back("draw");
            // 返回成功。
            Ok(())
        }

        // 记录纹理复制。
        fn copy_texture(&mut self, _copy: TextureCopy) -> Result<()> {
            // 记录调用事件。
            self.log.push_back("copy");
            // 返回成功。
            Ok(())
        }

        // 记录重叠安全的纹理区域移动。
        fn move_texture_region(&mut self, _movement: TextureMove) -> Result<()> {
            // 记录调用事件。
            self.log.push_back("move");
            // 返回成功。
            Ok(())
        }

        // 记录 render pass 结束。
        fn end_render_pass(&mut self) -> Result<()> {
            // 记录调用事件。
            self.log.push_back("end_pass");
            // 返回成功。
            Ok(())
        }

        // 记录唯一 submit，并允许测试提交失败。
        fn submit(&mut self) -> Result<SubmissionHandle> {
            // 记录调用事件。
            self.log.push_back("submit");
            // 在失败模式下返回 typed device error。
            if self.fail_submit {
                // 返回设备丢失，禁止进入 present。
                return Err(Error::new(
                    Errc::GraphicsDeviceLost,
                    "recording submit failed",
                ));
            }
            // 返回稳定的测试提交句柄。
            Ok(SubmissionHandle::from_raw(7))
        }
    }

    // 记录一个不会触碰真实窗口的 mock surface。
    struct RecordingSurface {
        // 保存当前 surface token。
        token: SurfaceToken,
        // 保存 surface render target。
        target: RenderTargetHandle,
        // 保存最终 present 次数。
        present_count: usize,
        // 保存是否强制最终 present 失败。
        fail_present: bool,
    }

    // 为记录型 surface 实现 acquire/resize/present。
    impl GraphicsSurface for RecordingSurface {
        // 返回当前代际。
        fn token(&self) -> SurfaceToken {
            // 返回 surface token。
            self.token
        }

        // 返回当前 acquired image。
        fn acquire(&mut self) -> Result<SurfaceFrame> {
            // 构造与当前 token 匹配的 frame。
            Ok(SurfaceFrame::new(self.token, self.target))
        }

        // 推进代际并更新 extent。
        fn resize(&mut self, extent: RhiExtent) -> Result<SurfaceToken> {
            // 递增代际，模拟真实 surface 重建。
            self.token = SurfaceToken::new(self.token.generation + 1, extent);
            // 返回新 token。
            Ok(self.token)
        }

        // 记录最终 present，并在失败模式下返回 surface 错误。
        fn present(
            &mut self,
            _frame: SurfaceFrame,
            _submission: SubmissionHandle,
            _damage: crate::core::PresentDamage,
        ) -> Result<()> {
            // 记录到达最终 present 边界。
            self.present_count += 1;
            // 在失败模式下返回 surface lost。
            if self.fail_present {
                // 返回 typed surface error，调用方不会得到 FrameCommit。
                return Err(Error::new(
                    Errc::GraphicsSurfaceLost,
                    "recording present failed",
                ));
            }
            // 返回成功。
            Ok(())
        }
    }

    // 构造一个包含 viewport、scissor 和 draw 的最小计划。
    fn test_plan(token: SurfaceToken) -> FramePlan {
        // 创建带完整清理的 surface pass。
        let mut pass = RenderPassPlan::new(
            RenderTargetRef::Surface,
            LoadAction::Clear(RhiColor([0.0, 0.0, 0.0, 1.0])),
        );
        // 追加 viewport 设置。
        pass.push(FramePlanCommand::SetViewport(RhiViewport {
            width: 64.0,
            height: 64.0,
        }));
        // 追加 scissor 设置。
        pass.push(FramePlanCommand::SetScissor(None));
        // 追加一个非空 buffer 上传，覆盖真实 renderer 的 vertex/uniform 顺序。
        pass.push(FramePlanCommand::UpdateBuffer {
            buffer: BufferHandle::from_raw(3),
            offset: 0,
            data: Arc::<[u8]>::from(vec![1, 2, 3, 4]),
        });
        // 追加一个非空三角形 packet。
        pass.push(FramePlanCommand::Draw(DrawPacket::triangles(
            PipelineHandle::from_raw(1),
            3,
        )));
        // 创建带完整 damage 的帧计划。
        let mut plan = FramePlan::new(token, crate::core::PresentDamage::Full);
        // 追加唯一 render pass。
        plan.push_pass(pass);
        // 返回测试计划。
        plan
    }

    // 验证低层命令保持顺序且只触发一次最终 present。
    #[test]
    fn executes_in_order_and_presents_once() {
        // 创建第一代 surface。
        let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
        // 创建记录型 device。
        let mut device = RecordingDevice {
            log: VecDeque::new(),
            capabilities: GraphicsCapabilities::full_gpu_baseline(),
            fail_submit: false,
        };
        // 创建记录型 surface。
        let mut surface = RecordingSurface {
            token,
            target: RenderTargetHandle::from_raw(2),
            present_count: 0,
            fail_present: false,
        };
        // 执行计划并要求最终提交成功。
        let commit = test_plan(token).execute(&mut device, &mut surface);
        // 验证得到可消费 damage 的 commit。
        assert!(commit.is_ok());
        // 验证底层顺序以一次 submit 结束。
        assert_eq!(
            device.log.into_iter().collect::<Vec<_>>(),
            vec![
                "begin_pass",
                "viewport",
                "scissor",
                "update_buffer",
                "draw",
                "end_pass",
                "submit"
            ]
        );
        // 验证 surface 只收到一次最终 present。
        assert_eq!(surface.present_count, 1);
    }

    // 将资源移动与 submit 失败边界拆到独立测试载荷，保持计划核心文件短小。
    include!("frame_plan_test_tail.rs");

}
