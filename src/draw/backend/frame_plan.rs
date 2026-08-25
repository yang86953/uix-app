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
use crate::platform::presentation::rhi::{
    DrawPacket, GraphicsContextRhi, GraphicsDevice, LoadAction, RhiColor, RhiScissor,
    SubmissionHandle, SurfaceToken, TextureCopy, TextureHandle, TextureMove,
};
// 将不触发 surface present 的离屏执行边界拆到独立文件。
#[path = "frame_plan_execution.rs"]
mod execution;
// 将顶点、索引与 Uniform 类型化载荷拆到独立契约文件。
#[path = "frame_plan_upload.rs"]
mod upload;
// 将 draw 与前序类型化上传的布局核对拆到独立验证组件。
#[path = "frame_plan_validation.rs"]
mod validation;
// 让通用 renderer 只能构造 FramePlan 明确允许的上传闭集。
pub(crate) use upload::{FrameIndexPayload, FrameUniformPayload, FrameVertexPayload};
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
    // 在 pass 内按明确物理矩形清理 premultiplied-alpha 颜色。
    ClearRect {
        // 保存局部清理颜色。
        color: RhiColor,
        // 保存左上原点的物理清理区域。
        scissor: RhiScissor,
    },
    // 在 pass 内按 painter order 上传一个类型化顶点流。
    UploadVertex {
        // 保存目标顶点 buffer。
        buffer: crate::platform::presentation::rhi::BufferHandle,
        // 保存声明布局和有限浮点值组成的顶点载荷。
        data: FrameVertexPayload,
    },
    // 在 pass 内按 painter order 上传一个类型化索引流。
    UploadIndex {
        // 保存目标索引 buffer。
        buffer: crate::platform::presentation::rhi::BufferHandle,
        // 保存不可拆分的索引格式与元素载荷。
        data: FrameIndexPayload,
    },
    // 在 pass 内完整上传一个类型化 Uniform 值对象。
    UploadUniform {
        // 保存目标 Uniform buffer。
        buffer: crate::platform::presentation::rhi::BufferHandle,
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

    // 接管 Renderer 已按 painter order 建立的命令缓冲区，避免绑定目标时二次扩容。
    pub(crate) fn from_commands(
        target: RenderTargetRef,
        load: LoadAction,
        commands: Vec<FramePlanCommand>,
    ) -> Self {
        // target/load 仍只由 Frame owner 绑定，producer 只交出命令所有权。
        Self {
            target,
            load,
            commands,
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
        // 真正空计划不能伪造一次 Device submit 或 Surface present。
        if self.steps.is_empty() {
            // 调用方必须至少显式交付一个 pass、copy 或 move。
            return Err(Error::new(
                Errc::InvalidArgument,
                "FramePlan must contain at least one command",
            ));
        }
        // Surface 计划必须包含可呈现 pass；离屏计划允许纯 Copy/Move 事务。
        if matches!(&self.scope, FramePlanScope::Surface { .. })
            // 只读扫描封闭步骤，不建立任何 Device 状态。
            && !self
                .steps
                .iter()
                .any(|step| matches!(step, FramePlanStep::Pass(_)))
        {
            // 没有 pass 的 Surface 计划不得 acquire 并呈现未写入 image。
            return Err(Error::new(
                Errc::InvalidArgument,
                "surface FramePlan must contain at least one render pass",
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
                            // 当前 Draw 自身的采样输入必须与离屏输出保持资源分离。
                            validation::validate_draw_sampling_target(
                                // 使用当前 pass 已冻结的逻辑目标。
                                pass.target,
                                // 只读取 packet 原子拥有的条件采样角色。
                                packet.sampling(),
                            )?;
                            // 句柄绑定、前序上传布局和采样角色必须匹配共享 pipeline 契约。
                            validation::validate_draw_uploads(pass, command_index, *packet)?;
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
                        // 验证类型化索引载荷非空且数量可由 DrawRange 表达。
                        if let FramePlanCommand::UploadIndex { data, .. } = command {
                            // 索引格式和元素值必须保持在 FramePlan 封闭契约内。
                            if !data.is_valid() {
                                // 返回稳定参数错误，禁止空或不可表示索引流进入 Adapter。
                                return Err(Error::new(
                                    Errc::InvalidArgument,
                                    "FramePlan index upload must contain representable indices",
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
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../tests/unit/draw/backend/frame_plan__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
