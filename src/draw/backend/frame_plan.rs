//! 通用 GPU Renderer 的有限、有序帧计划。
//!
//! `FramePlan` 只承载已经完成 UI 语义降级的 pass、copy 和 draw packet；
//! 原生 adapter 不能从这里取得 widget、字体或路径算法。

#![allow(dead_code)]
// 引入框架错误类型，保证执行失败不会被当作成功帧。
use crate::core::error::{Errc, Error, Result};
// 引入最终呈现的 damage 值。
use crate::core::PresentDamage;
// 引入 platform 私有的薄 RHI 原语。
use crate::native::present::rhi::{
    DrawPacket, GraphicsContextRhi, GraphicsDevice, LoadAction, RhiColor, RhiScissor, RhiViewport,
    SampledTextureBinding, SubmissionHandle, SurfaceToken, TextureCopy, TextureHandle, TextureMove,
};
// 将不触发 surface present 的离屏执行边界拆到独立文件。
#[path = "frame_plan_execution.rs"]
mod execution;
// 将顶点与 Uniform 类型化载荷拆到独立契约文件。
#[path = "frame_plan_upload.rs"]
mod upload;
// 将 draw 与前序类型化上传的布局核对拆到独立验证组件。
#[path = "frame_plan_validation.rs"]
mod validation;
// 让通用 renderer 只能构造 FramePlan 明确允许的上传闭集。
pub(crate) use upload::{FrameUniformPayload, FrameVertexPayload};
// 描述 render pass 目标，surface 目标在 acquire 后才绑定具体句柄。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RenderTargetRef {
    // 指向当前 acquired surface image。
    Surface,
    // 指向 device 管理的离屏 render target。
    Texture(TextureHandle),
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
    // 绑定固定 t0/s0 ABI 的原子采样资源。
    BindSampledTexture(SampledTextureBinding),
    // 在 pass 内按 painter order 上传一个类型化顶点流。
    UploadVertex {
        // 保存目标顶点 buffer。
        buffer: crate::native::present::rhi::BufferHandle,
        // 保存声明布局和有限浮点值组成的顶点载荷。
        data: FrameVertexPayload,
    },
    // 在 pass 内完整上传一个类型化 Uniform 值对象。
    UploadUniform {
        // 保存目标 Uniform buffer。
        buffer: crate::native::present::rhi::BufferHandle,
        // 保存与 PipelineContract 闭集一致的 Uniform 载荷。
        data: FrameUniformPayload,
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

// 描述 FramePlan 唯一拥有的 Device 或 Surface 执行作用域。
#[derive(Debug, Clone, PartialEq, Eq)]
enum FramePlanScope {
    // 原子保存最终呈现所需的 Surface 代际与 damage。
    Surface {
        // 冻结计划创建时观察到的 Surface 代际。
        surface: SurfaceToken,
        // 保存仅由最终 Surface present 消费的 damage。
        damage: PresentDamage,
    },
    // 表示计划只允许进入 Device 离屏执行边界。
    Offscreen,
}

// 持有一帧有限且有序的 GPU 执行计划。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FramePlan {
    // 以封闭类型保存唯一执行作用域，禁止外部目标选择器与计划互相矛盾。
    scope: FramePlanScope,
    // 保存 pass 和 copy 的严格执行顺序。
    steps: Vec<FramePlanStep>,
}

// 为 FramePlan 提供构造、验证和执行入口。
impl FramePlan {
    // 创建一帧空计划。
    pub(crate) fn new(surface: SurfaceToken, damage: PresentDamage) -> Self {
        // 返回等待通用 renderer 追加步骤的计划。
        Self {
            // Surface 计划把代际、damage 与执行作用域绑定为一个事实。
            scope: FramePlanScope::Surface { surface, damage },
            steps: Vec::new(),
        }
    }

    // 创建一个只属于 device 的离屏计划。
    pub(crate) fn offscreen() -> Self {
        // 离屏资源不依赖 swapchain generation。
        Self {
            // Device-only 计划不保存或伪造任何 SurfaceToken 与 PresentDamage。
            scope: FramePlanScope::Offscreen,
            steps: Vec::new(),
        }
    }

    // 判断唯一作用域是否要求完整 Surface 事务。
    pub(super) const fn targets_surface(&self) -> bool {
        // 只读取计划自有的封闭作用域，不接受调用方的重复选择器。
        matches!(&self.scope, FramePlanScope::Surface { .. })
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
        // Surface 计划拒绝无效 extent；离屏计划没有窗口 extent。
        if matches!(
            // 只检查封闭作用域内真实存在的 SurfaceToken。
            &self.scope,
            // Device-only 计划没有可被误判的窗口 extent。
            FramePlanScope::Surface { surface, .. } if !surface.extent.is_valid()
        ) {
            // 返回稳定的参数错误。
            return Err(Error::new(
                Errc::InvalidArgument,
                "FramePlan surface extent is outside the shared native domain",
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
                        if !color.is_valid() {
                            // 返回稳定的参数错误。
                            return Err(Error::new(
                                Errc::InvalidArgument,
                                "FramePlan clear color must be finite",
                            ));
                        }
                    }
                    // 验证 pass 内的命令顺序元素。
                    for (command_index, command) in pass.commands.iter().enumerate() {
                        // 每条采样命令都必须立即通过当前 pass 目标反馈环门禁。
                        if let FramePlanCommand::BindSampledTexture(binding) = command {
                            // 不等待后续覆盖绑定，保持 painter order 的失败语义。
                            validation::validate_sampled_binding_target(pass.target, *binding)?;
                        }
                        // 拒绝空范围或超过两个原生 ABI 共同值域的 draw packet。
                        if let FramePlanCommand::Draw(packet) = command {
                            // 统一范围门禁避免 Adapter 对同一个 u32 产生不同解释。
                            if !packet.has_valid_range() {
                                // 返回稳定的参数错误。
                                return Err(Error::new(
                                    Errc::InvalidArgument,
                                    "FramePlan draw packet range is invalid",
                                ));
                            }
                            // 句柄绑定、前序上传布局和采样状态必须匹配共享 pipeline 契约。
                            validation::validate_draw_uploads(pass, command_index, *packet)?;
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
                            if !color.is_valid() || !scissor.is_valid() {
                                // 返回稳定的参数错误，避免 adapter 产生未定义清理范围。
                                return Err(Error::new(
                                    Errc::InvalidArgument,
                                    "FramePlan clear rect must be finite and positive",
                                ));
                            }
                        }
                        // 验证类型化顶点不会把空、残缺或非有限几何交给 Adapter。
                        if let FramePlanCommand::UploadVertex { data, .. } = command {
                            // 顶点布局与浮点值必须同时满足 FramePlan 契约。
                            if !data.is_valid() {
                                // 返回稳定的参数错误。
                                return Err(Error::new(
                                    Errc::InvalidArgument,
                                    "FramePlan vertex upload must contain complete finite vertices",
                                ));
                            }
                        }
                        // 验证类型化 Uniform 属于共享 FramePlan 值域。
                        if let FramePlanCommand::UploadUniform { data, .. } = command {
                            // 固定值对象已经从类型上保证大小，这里统一验证值域。
                            if !data.is_valid() {
                                // 返回稳定的参数错误。
                                return Err(Error::new(
                                    Errc::InvalidArgument,
                                    "FramePlan uniform upload is outside the shared value domain",
                                ));
                            }
                        }
                    }
                }
                // 检查纹理复制区域。
                FramePlanStep::Copy(copy) => {
                    // 类型化复制尺寸必须属于两个 Adapter 的共同原生值域。
                    if !copy.transfer().extent().is_valid() {
                        // 返回稳定的参数错误。
                        return Err(Error::new(
                            Errc::InvalidArgument,
                            "FramePlan texture copy extent must be positive",
                        ));
                    }
                }
                // 检查重叠安全的纹理区域移动。
                FramePlanStep::Move(movement) => {
                    // 类型化移动尺寸必须属于两个 Adapter 的共同原生值域。
                    if !movement.transfer().extent().is_valid() {
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

    // 读取 surface 执行入口必须拥有的窗口代际。
    fn required_surface(&self) -> Result<(SurfaceToken, PresentDamage)> {
        // 只允许 Surface 作用域原子交付其冻结代际与 present damage。
        match &self.scope {
            // 返回与计划作用域原子绑定的 Surface 事务事实。
            FramePlanScope::Surface { surface, damage } => Ok((*surface, damage.clone())),
            // Device-only 计划不能越权进入 acquire 或 present。
            FramePlanScope::Offscreen => Err(Error::new(
                // 使用稳定参数错误标记计划作用域错配。
                Errc::InvalidArgument,
                // 保持跨 Adapter 一致的拒绝诊断。
                "offscreen FramePlan cannot enter a surface execution boundary",
            )),
        }
    }
    // 在进入任何 native 操作前统一验证计划与 device 基线。
    fn validate_for_device<D>(&self, device: &D) -> Result<()>
    where
        // 接受独立 device 或组合 context，而不要求发生 trait-object 上转型。
        D: GraphicsDevice + ?Sized,
    {
        // 先验证 FramePlan 自身的结构与数值不变量。
        self.validate()?;
        // 读取本次执行实际借用的 device 能力事实。
        if let Some(missing) = device
            // FramePlan validation only reads resource/pass/pipeline capabilities from Device。
            .device_capabilities()
            // 返回第一个缺失的 Device 基线事实。
            .first_missing_gpu_baseline()
        {
            // 用 NotImplemented 表示 adapter 缺少 RHI 基线，而不是 UI 操作缺失。
            return Err(Error::new(
                Errc::NotImplemented,
                format!("GPU adapter is missing required capability: {missing}"),
            ));
        }
        // 计划与 device 都满足进入唯一执行器的前置条件。
        Ok(())
    }
    // 在一个同时拥有 device 与 surface 的原生 context 上执行计划。
    pub(crate) fn execute_on_context(
        &self,
        context: &mut dyn GraphicsContextRhi,
    ) -> Result<FrameCommit> {
        // 普通执行路径不插入额外的 submit/present 观察动作。
        self.execute_on_context_with_before_present(context, &mut |_| {})
    }
}

#[cfg(test)]
mod tests {
    // 引入测试用集合，记录 RHI 的执行顺序。
    use std::collections::VecDeque;
    // 引入框架错误类型和最终 damage。
    use crate::core::error::{Errc, Error, Result};
    // 引入测试计划依赖的薄 RHI 类型。
    use crate::native::present::rhi::{
        BufferHandle, DrawPacket, GraphicsDevice, GraphicsDeviceCapabilities, GraphicsSurface,
        LoadAction, PipelineBinding, PipelineHandle, PipelineKind, RenderTargetHandle,
        RhiBufferUpload, RhiColor, RhiExtent, RhiGradientRasterParams, RhiMeshRasterParams,
        RhiPresentTransaction, RhiSampledRasterParams, RhiScissor, RhiTextureTransfer, RhiViewport,
        SampledTextureBinding, SamplerHandle, SubmissionHandle, SurfaceFrame, SurfaceToken,
        TextureCopy, TextureHandle, TextureMove,
    };
    // 引入当前文件的计划类型。
    use super::{
        FramePlan, FramePlanCommand, FramePlanScope, FramePlanStep, FrameUniformPayload,
        FrameVertexPayload, RenderPassPlan, RenderTargetRef,
    };

    // 记录一个不会触碰真实图形 API 的 mock device。
    struct RecordingDevice {
        // 保存薄 RHI 调用顺序。
        log: VecDeque<&'static str>,
        // 保存测试适配器报告的底层能力。
        capabilities: GraphicsDeviceCapabilities,
        // 保存是否强制 submit 失败。
        fail_submit: bool,
        // 保存是否强制 draw 失败以验证 pass 中途收尾。
        fail_draw: bool,
        // 保存是否强制目标能力解析失败。
        fail_target_resolution: bool,
        // 保存是否强制普通 texture copy 预检失败。
        fail_copy_preflight: bool,
        // 保存是否强制 texture move 预检失败。
        fail_move_preflight: bool,
        // 保存是否强制 Draw 资源预检失败。
        fail_draw_preflight: bool,
        // 保存是否强制 sampled 资源预检失败。
        fail_sampled_preflight: bool,
    }

    // 为记录型 device 实现薄 RHI 的执行原语。
    impl GraphicsDevice for RecordingDevice {
        // 返回完整 GPU 基线，测试重点放在顺序而不是能力拒绝。
        fn device_capabilities(&self) -> GraphicsDeviceCapabilities {
            // 返回测试实例配置的能力快照。
            self.capabilities
        }

        // 测试设备把 fixture texture 提升为已验证的目标身份。
        fn resolve_render_target(&self, texture: TextureHandle) -> Result<RenderTargetHandle> {
            // 在失败模式下模拟共享资源表拒绝不可渲染目标。
            if self.fail_target_resolution {
                // 返回稳定参数错误，证明失败发生在激活前。
                return Err(Error::new(
                    // 目标格式或身份不满足共享契约。
                    Errc::InvalidArgument,
                    // 测试诊断不泄漏具体图形 API。
                    "recording texture is not renderable",
                ));
            }
            // 测试入口不模拟真实资源表，只提供稳定的可渲染 mock 事实。
            Ok(RenderTargetHandle::for_test(texture))
        }

        // 在任何 Device 原语前预检普通 texture copy。
        fn preflight_texture_copy(&self, _copy: TextureCopy) -> Result<()> {
            // 注入失败时保持 Device 日志为空。
            if self.fail_copy_preflight {
                // 返回共享参数错误，模拟资源表或传输契约拒绝。
                return Err(Error::new(
                    Errc::InvalidArgument,
                    "recording copy preflight failed",
                ));
            }
            // 关闭注入后允许合法 copy 继续执行。
            Ok(())
        }

        // 在任何 Device 原语前预检 texture move。
        fn preflight_texture_move(&self, _movement: TextureMove) -> Result<()> {
            // 注入失败时保持 Device 日志为空。
            if self.fail_move_preflight {
                // 返回共享参数错误，模拟资源表或传输契约拒绝。
                return Err(Error::new(
                    Errc::InvalidArgument,
                    "recording move preflight failed",
                ));
            }
            // 关闭注入后允许合法 move 继续执行。
            Ok(())
        }

        // 在任何 Device 原语前预检 Draw 真实 Buffer 资源。
        fn preflight_draw_resources(&self, _packet: DrawPacket) -> Result<()> {
            // 注入失败时保持 Device 日志为空。
            if self.fail_draw_preflight {
                // 返回共享参数错误，模拟资源表角色或容量拒绝。
                return Err(Error::new(
                    Errc::InvalidArgument,
                    "recording draw resource preflight failed",
                ));
            }
            // 关闭注入后允许合法 Draw 继续执行。
            Ok(())
        }

        // 在任何 Device 原语前预检 sampled binding 真实资源。
        fn preflight_sampled_binding(&self, _binding: SampledTextureBinding) -> Result<()> {
            // 注入失败时保持 Device 日志为空。
            if self.fail_sampled_preflight {
                // 返回共享参数错误，模拟纹理格式或 sampler 过滤拒绝。
                return Err(Error::new(
                    Errc::InvalidArgument,
                    "recording sampled resource preflight failed",
                ));
            }
            // 关闭注入后允许合法 sampled binding 继续执行。
            Ok(())
        }

        // 记录任何原生命令前必须发生的 owner-context 激活。
        fn activate(&mut self) -> Result<()> {
            // 记录统一计划执行器触发的激活边界。
            self.log.push_back("activate");
            // 返回成功，让测试继续观察健康检查与后续命令顺序。
            Ok(())
        }

        // 记录激活后必须发生的设备健康检查。
        fn maintain(&mut self) -> Result<()> {
            // 记录统一计划执行器触发的 health preflight。
            self.log.push_back("maintain");
            // 返回成功，让测试继续观察后续命令顺序。
            Ok(())
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
        fn bind_sampled_texture(&mut self, _binding: SampledTextureBinding) -> Result<()> {
            // 记录调用事件。
            self.log.push_back("bind_sampled_texture");
            // 返回成功。
            Ok(())
        }

        // 记录 buffer 上传，验证它位于 draw 前且不会被 adapter 忽略。
        fn update_buffer(&mut self, _upload: RhiBufferUpload<'_>) -> Result<()> {
            // 记录调用事件。
            self.log.push_back("update_buffer");
            // 返回成功。
            Ok(())
        }

        // 记录 draw packet。
        fn draw(&mut self, _packet: DrawPacket) -> Result<()> {
            // 记录调用事件。
            self.log.push_back("draw");
            // 在失败模式下模拟 Adapter 于 pass 内拒绝 draw。
            if self.fail_draw {
                // 返回稳定平台错误，供执行器证明主错误不会被 cleanup 覆盖。
                return Err(Error::new(Errc::PlatformError, "recording draw failed"));
            }
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
            Ok(SurfaceFrame::new(self.token))
        }

        // 推进代际并更新 extent。
        fn resize(&mut self, extent: RhiExtent) -> Result<SurfaceToken> {
            // 递增代际，模拟真实 surface 重建。
            self.token = SurfaceToken::new(self.token.generation + 1, extent);
            // 返回新 token。
            Ok(self.token)
        }

        // 记录最终 present，并在失败模式下返回 surface 错误。
        fn present(&mut self, _transaction: RhiPresentTransaction) -> Result<()> {
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

    // 组合记录型 device 与 surface，覆盖迁移期 context 执行入口。
    struct RecordingContext {
        // 保存所有 device 命令及提交结果。
        device: RecordingDevice,
        // 保存 acquire、generation 与最终 present 状态。
        surface: RecordingSurface,
    }

    // 让组合记录 context 通过同一个 GraphicsDevice 契约接收命令。
    impl GraphicsDevice for RecordingContext {
        // 返回内嵌 device 的事实能力。
        fn device_capabilities(&self) -> GraphicsDeviceCapabilities {
            // 不建立第二份测试能力来源。
            self.device.device_capabilities()
        }

        // 把普通 texture copy 预检委托给内嵌记录 device。
        fn preflight_texture_copy(&self, copy: TextureCopy) -> Result<()> {
            // 复用唯一测试资源预检路径。
            self.device.preflight_texture_copy(copy)
        }

        // 把 texture move 预检委托给内嵌记录 device。
        fn preflight_texture_move(&self, movement: TextureMove) -> Result<()> {
            // 复用唯一测试资源预检路径。
            self.device.preflight_texture_move(movement)
        }

        // 把 Draw 资源预检委托给内嵌记录 device。
        fn preflight_draw_resources(&self, packet: DrawPacket) -> Result<()> {
            // 复用唯一测试资源预检路径。
            self.device.preflight_draw_resources(packet)
        }

        // 把 sampled 资源预检委托给内嵌记录 device。
        fn preflight_sampled_binding(&self, binding: SampledTextureBinding) -> Result<()> {
            // 复用唯一测试资源预检路径。
            self.device.preflight_sampled_binding(binding)
        }

        // 把 owner-context 激活委托给内嵌记录 device。
        fn activate(&mut self) -> Result<()> {
            // surface 与 offscreen 模式必须观察同一激活边界。
            self.device.activate()
        }

        // 把设备健康检查委托给内嵌记录 device。
        fn maintain(&mut self) -> Result<()> {
            // surface 与 offscreen 模式必须观察同一 health preflight。
            self.device.maintain()
        }

        // 把 pass 开始委托给内嵌记录 device。
        fn begin_render_pass(
            &mut self,
            target: RenderTargetHandle,
            load: LoadAction,
        ) -> Result<()> {
            // 保持独立 device 与组合 context 的记录行为相同。
            self.device.begin_render_pass(target, load)
        }

        // 把 viewport 设置委托给内嵌记录 device。
        fn set_viewport(&mut self, viewport: RhiViewport) -> Result<()> {
            // 复用唯一测试记录路径。
            self.device.set_viewport(viewport)
        }

        // 把 scissor 设置委托给内嵌记录 device。
        fn set_scissor(&mut self, scissor: Option<RhiScissor>) -> Result<()> {
            // 复用唯一测试记录路径。
            self.device.set_scissor(scissor)
        }

        // 把局部清理委托给内嵌记录 device。
        fn clear_rect(&mut self, color: RhiColor, scissor: RhiScissor) -> Result<()> {
            // 复用唯一测试记录路径。
            self.device.clear_rect(color, scissor)
        }

        // 把采样资源绑定委托给内嵌记录 device。
        fn bind_sampled_texture(&mut self, binding: SampledTextureBinding) -> Result<()> {
            // 复用唯一测试记录路径。
            self.device.bind_sampled_texture(binding)
        }

        // 把不可变上传载荷委托给内嵌记录 device。
        fn update_buffer(&mut self, upload: RhiBufferUpload<'_>) -> Result<()> {
            // 复用唯一测试记录路径。
            self.device.update_buffer(upload)
        }

        // 把 draw packet 委托给内嵌记录 device。
        fn draw(&mut self, packet: DrawPacket) -> Result<()> {
            // 复用唯一测试记录路径。
            self.device.draw(packet)
        }

        // 把纹理复制委托给内嵌记录 device。
        fn copy_texture(&mut self, copy: TextureCopy) -> Result<()> {
            // 复用唯一测试记录路径。
            self.device.copy_texture(copy)
        }

        // 把重叠安全移动委托给内嵌记录 device。
        fn move_texture_region(&mut self, movement: TextureMove) -> Result<()> {
            // 复用唯一测试记录路径。
            self.device.move_texture_region(movement)
        }

        // 把 pass 结束委托给内嵌记录 device。
        fn end_render_pass(&mut self) -> Result<()> {
            // 复用唯一测试记录路径。
            self.device.end_render_pass()
        }

        // 把唯一提交委托给内嵌记录 device。
        fn submit(&mut self) -> Result<SubmissionHandle> {
            // 复用唯一测试记录路径。
            self.device.submit()
        }
    }

    // 让组合记录 context 通过同一个 GraphicsSurface 契约管理呈现状态。
    impl GraphicsSurface for RecordingContext {
        // 返回内嵌 surface 的当前代际。
        fn token(&self) -> SurfaceToken {
            // 不缓存或复制动态 surface 事实。
            self.surface.token()
        }

        // 从内嵌 surface 获取当前 image。
        fn acquire(&mut self) -> Result<SurfaceFrame> {
            // 复用独立 surface 的 acquire 语义。
            self.surface.acquire()
        }

        // 通过内嵌 surface 原子推进代际。
        fn resize(&mut self, extent: RhiExtent) -> Result<SurfaceToken> {
            // 复用独立 surface 的 resize 语义。
            self.surface.resize(extent)
        }

        // 把最终 present 委托给内嵌 surface。
        fn present(&mut self, transaction: RhiPresentTransaction) -> Result<()> {
            // 复用独立 surface 的成功与失败边界。
            self.surface.present(transaction)
        }
    }

    // 创建具备完整 GPU 基线的组合记录 context。
    fn recording_context(token: SurfaceToken) -> RecordingContext {
        // 组合唯一 device 与唯一 surface owner。
        RecordingContext {
            // 创建可记录全部命令的 device。
            device: RecordingDevice {
                // 初始尚未执行任何 native 原语。
                log: VecDeque::new(),
                // 允许计划进入正常 GPU 执行路径。
                capabilities: GraphicsDeviceCapabilities::full_gpu_baseline(),
                // 默认允许 submit 成功。
                fail_submit: false,
                // 默认允许 pass 内 draw 成功。
                fail_draw: false,
                // 默认允许目标能力解析成功。
                fail_target_resolution: false,
                // 默认允许普通 copy 预检成功。
                fail_copy_preflight: false,
                // 默认允许 texture move 预检成功。
                fail_move_preflight: false,
                // 默认允许 Draw 资源预检成功。
                fail_draw_preflight: false,
                // 默认允许 sampled 资源预检成功。
                fail_sampled_preflight: false,
            },
            // 创建与计划代际一致的 surface。
            surface: RecordingSurface {
                // 保存调用方提供的 surface token。
                token,
                // 初始尚未发生最终 present。
                present_count: 0,
                // 默认允许 present 成功。
                fail_present: false,
            },
        }
    }

    // 将 Surface 与 Offscreen 共用的最小计划构造器拆到独立测试支持文件。
    include!("frame_plan_test_support.rs");

    // 将组合 context 的多执行模式回归测试拆到独立载荷，保持核心文件小于上限。
    include!("frame_plan_context_tests.rs");

    // 将资源移动与 submit 失败边界拆到独立测试载荷，保持计划核心文件短小。
    include!("frame_plan_test_tail.rs");
}
