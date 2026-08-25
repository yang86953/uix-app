//! FramePlan 在 surface 与 offscreen 目标上的统一执行边界。

// 引入统一执行结果与稳定参数错误。
use crate::core::error::{Errc, Error, Result};
// 引入只属于最终 Surface present 的 damage。
use crate::core::PresentDamage;
// 引入组合 context、正交 Device/Surface 角色与显式纹理目标。
use crate::platform::presentation::rhi::{
    GraphicsContextRhi, GraphicsDevice, GraphicsSurface, LoadAction, TextureHandle,
};

// 引入已经完成 lowering 且自有执行作用域的帧计划与目标引用。
use super::super::frame_plan::{FramePlan, FramePlanCommand, RenderPassPlan, RenderTargetRef};

// 封闭 Renderer 帧可进入的 Surface 或 Offscreen 角色。
enum RhiRendererFrameRole<'a> {
    // 完整 Surface 帧原子拥有组合 context 与可选观察钩子。
    Surface {
        // 借用唯一原生 context owner。
        context: &'a mut dyn GraphicsContextRhi,
        // 保存 submit 后、present 前只能取得 Surface 的观察钩子。
        before_present: Option<&'a mut dyn FnMut(&mut dyn GraphicsSurface)>,
    },
    // Offscreen 帧只拥有 Device 与显式纹理目标。
    Offscreen {
        // 借用资源、命令与 submit 所需的 Device 角色。
        device: &'a mut dyn GraphicsDevice,
        // 保存无法表示主 Surface 的类型化纹理目标。
        target: TextureHandle,
    },
}

// 描述 Renderer 帧是否仍可进入唯一执行边界。
#[derive(Clone, Copy, PartialEq, Eq)]
enum RhiRendererFrameExecutionState {
    // 新建帧尚未触碰任何 Device 或 Surface 原语。
    Pending,
    // 执行尝试已经消费帧，即使后续验证或 Adapter 调用失败。
    Consumed,
}

// 保存尚未绑定 target 与 load 的 Renderer pass 命令包。
pub(crate) struct RhiRendererPass {
    // 保存 producer 已经按 painter order lower 的命令。
    commands: Vec<FramePlanCommand>,
}

// 为目标无关的 Renderer pass 提供封闭构造与有序追加入口。
impl RhiRendererPass {
    // 创建不携带任何 target 或 load 选择器的空命令包。
    fn new() -> Self {
        // 返回等待 producer 追加命令的私有载荷。
        Self {
            // 新建命令包保持空顺序。
            commands: Vec::new(),
        }
    }

    // 追加一个已经完成 Drawing lowering 的类型化命令。
    pub(crate) fn push(&mut self, command: FramePlanCommand) {
        // 只保存命令顺序，不允许 producer 绑定执行目标。
        self.commands.push(command);
    }
}

// 原子持有封闭帧角色与唯一执行生命周期。
pub(crate) struct RhiRendererFrame<'a> {
    // 保存 Surface 或 Offscreen 的不可拆角色事实。
    role: RhiRendererFrameRole<'a>,
    // 保存由当前帧作用域唯一创建并执行的类型化计划。
    plan: FramePlan,
    // 保存本帧一次性执行状态，不影响执行后的 Device 清理借用。
    execution_state: RhiRendererFrameExecutionState,
}

// 为封闭 Renderer 帧提供唯一组合与分阶段执行入口。
impl<'a> RhiRendererFrame<'a> {
    // 创建带 submit-before-present 观察钩子的 Surface 帧。
    pub(crate) fn surface_with_present_hook(
        // 借用唯一原生 context owner。
        context: &'a mut dyn GraphicsContextRhi,
        // 接收最终 present damage。
        damage: PresentDamage,
        // 借用只在最终 present 前取得 Surface 的观察钩子。
        before_present: &'a mut dyn FnMut(&mut dyn GraphicsSurface),
        // 返回仍由同一 context 完成最终 present 的 Surface 帧。
    ) -> Self {
        // 在借入组合 context 前冻结当前 Surface 代际与最终 damage。
        let plan = FramePlan::new(context.surface_ref().token(), damage);
        // 原子建立带观察边界的完整 Surface 生命周期。
        Self {
            // 原子保存完整 Surface 生命周期角色。
            role: RhiRendererFrameRole::Surface {
                // 保存组合 context 的唯一可变借用。
                context,
                // 观察钩子只能存在于 Surface 角色。
                before_present: Some(before_present),
            },
            // 保存与 Surface 角色同时建立的唯一计划。
            plan,
            // 新建 Surface 帧必须从 Pending 状态开始。
            execution_state: RhiRendererFrameExecutionState::Pending,
        }
    }

    // 创建只允许写入显式纹理的 Offscreen 帧。
    pub(crate) fn offscreen(
        // 借用独立 Device 角色。
        device: &'a mut dyn GraphicsDevice,
        // 接收类型化纹理目标。
        target: TextureHandle,
        // 返回不含任何 Surface 能力或 damage 的帧。
    ) -> Self {
        // 原子建立 Device-only 生命周期。
        Self {
            // 原子保存 Device-only 离屏角色。
            role: RhiRendererFrameRole::Offscreen {
                // 保存 Device 独占借用。
                device,
                // 保存显式纹理目标。
                target,
            },
            // 保存不含任何 Surface 生命周期的唯一离屏计划。
            plan: FramePlan::offscreen(),
            // 新建 Offscreen 帧必须从 Pending 状态开始。
            execution_state: RhiRendererFrameExecutionState::Pending,
        }
    }

    // 返回当前帧普通 pass 默认写入的计划目标。
    const fn render_target(&self) -> RenderTargetRef {
        // 从封闭变体投影默认目标，不向 producer 暴露选择器。
        match &self.role {
            // Surface 帧只能写入本次 acquire 的 image。
            RhiRendererFrameRole::Surface { .. } => RenderTargetRef::Surface,
            // Offscreen 帧的普通 pass 写入自己保存的最终纹理目标。
            RhiRendererFrameRole::Offscreen { target, .. } => RenderTargetRef::Texture(*target),
        }
    }

    // 可变借用当前帧允许使用的 Device 角色。
    pub(crate) fn device(&mut self) -> &mut dyn GraphicsDevice {
        // 两个变体都只暴露 Device 窄视图。
        match &mut self.role {
            // Surface 帧经组合 context 取得可变 Device。
            RhiRendererFrameRole::Surface { context, .. } => context.device(),
            // Offscreen 帧直接返回独立 Device。
            RhiRendererFrameRole::Offscreen { device, .. } => &mut **device,
        }
    }

    // 创建一个不携带 target 与 load 的 Renderer pass 命令包。
    pub(crate) fn new_pass(&self) -> RhiRendererPass {
        // 只有当前帧入口可以创建 producer 使用的命令包。
        RhiRendererPass::new()
    }

    // 接管命令包并绑定当前帧默认 target 与本次 load。
    pub(crate) fn push_pass(&mut self, load: LoadAction, pass: RhiRendererPass) {
        // 先取得封闭帧角色拥有的默认目标，避免 producer 参与目标选择。
        let target = self.render_target();
        // 统一由 Frame owner 构造真实 pass 并追加到唯一计划。
        self.push_targeted_pass(target, load, pass);
    }

    // 为 Offscreen 帧接管一个写入显式纹理的多目标 pass。
    pub(crate) fn push_offscreen_pass(
        &mut self,
        target: TextureHandle,
        load: LoadAction,
        pass: RhiRendererPass,
    ) -> Result<()> {
        // Surface 帧不得在完整 present 事务内注入第二个离屏目标。
        if !matches!(&self.role, RhiRendererFrameRole::Offscreen { .. }) {
            // 在修改计划前返回稳定的作用域错误。
            return Err(Error::new(
                Errc::InvalidArgument,
                "offscreen texture pass requires an Offscreen RhiRendererFrame",
            ));
        }
        // 只有通过角色门禁后才把显式纹理绑定到 Frame 自有计划。
        self.push_targeted_pass(RenderTargetRef::Texture(target), load, pass);
        // 多目标 pass 已由唯一 Frame owner 接管。
        Ok(())
    }

    // 把目标无关命令包绑定为 Frame 自有的真实 pass。
    fn push_targeted_pass(
        &mut self,
        target: RenderTargetRef,
        load: LoadAction,
        pass: RhiRendererPass,
    ) {
        // Frame owner 原子绑定 target/load，并直接接管 producer 已排序的命令缓冲区。
        let plan = RenderPassPlan::from_commands(target, load, pass.commands);
        // 把已经绑定当前帧 target 的 pass 追加到唯一计划。
        self.plan.push_pass(plan);
    }

    // 在任何 Device 或 Surface 调用前消费一次性执行资格。
    fn begin_execution(&mut self) -> Result<()> {
        // 第二次执行必须在任何原生调用前稳定拒绝。
        if self.execution_state == RhiRendererFrameExecutionState::Consumed {
            // 返回生命周期错误而不是重复提交同一帧。
            return Err(Error::new(
                // 已消费帧属于当前执行状态错误。
                Errc::InvalidState,
                // 保持跨 Surface 与 Offscreen 的稳定诊断。
                "RhiRendererFrame can only be executed once",
            ));
        }
        // 首次执行尝试立即消费资格，失败后也不得重试。
        self.execution_state = RhiRendererFrameExecutionState::Consumed;
        // 执行资格已成功转移到当前调用。
        Ok(())
    }

    // 按封闭帧角色执行自己唯一拥有的计划。
    pub(crate) fn execute(&mut self) -> Result<()> {
        // 先消费一次性资格，再进入 Surface/Offscreen 角色分派。
        self.begin_execution()?;
        // 只读借用当前帧内部计划，禁止执行入口接收第二份选择器。
        let plan = &self.plan;
        // Surface 与 Offscreen 进入互不重叠的执行边界。
        match &mut self.role {
            // Surface 帧消费可选观察钩子并完成唯一最终 present。
            RhiRendererFrameRole::Surface {
                context,
                before_present,
                ..
            } => {
                // 每个封闭帧只能执行一次，先移出可选观察钩子解除嵌套借用。
                let before_present = before_present.take();
                // 在同一组合 context 上完成 acquire、submit、观察与 present。
                execute_surface_plan_with_present_hook(
                    // 传入同一组合 context owner。
                    &mut **context,
                    // 借用作用域匹配的 Surface 计划。
                    plan,
                    // 观察钩子不能出现在 Offscreen 变体。
                    before_present,
                )
            }
            // Offscreen 帧只提交 Device 命令。
            RhiRendererFrameRole::Offscreen { device, .. } => {
                // 进入拒绝 Surface 计划的 Device-only 边界。
                execute_plan_without_present(&mut **device, plan)
            }
        }
    }
}

// 执行 Surface 计划，并允许最终 present 前运行一次只读钩子。
pub(super) fn execute_surface_plan_with_present_hook(
    // 借用组合 RHI context。
    context: &mut dyn GraphicsContextRhi,
    // 借用已经完成 lowering 的 FramePlan。
    plan: &FramePlan,
    // 可选钩子只在 Surface submit 成功后取得窄 Surface 角色。
    before_present: Option<&mut dyn FnMut(&mut dyn GraphicsSurface)>,
    // 返回执行或最终 present 的真实结果。
) -> Result<()> {
    // Surface 执行入口不能替 Offscreen 计划猜测生命周期。
    if !plan.targets_surface() {
        // 使用稳定参数错误拒绝作用域错配。
        return Err(Error::new(
            // 目标生命周期错配属于调用参数错误。
            Errc::InvalidArgument,
            // 指导调用方使用 Device-only 执行边界。
            "surface execution requires a Surface FramePlan",
        ));
    }
    // 该日志只在已经取得组合 RHI context 后触发，可用于区分真实 RHI 提交与兼容路径。
    tracing::debug!("Graphics RHI FramePlan submit: scope=surface");
    // 有观察者时使用显式的 submit-before-present 边界。
    if let Some(before_present) = before_present {
        // 丢弃成功提交的 FrameCommit，调用方只关心 lowering 是否成功。
        plan.execute_on_context_with_before_present(context, before_present)?;
    } else {
        // 普通 surface 计划保持原有唯一 acquire/submit/present 入口。
        plan.execute_on_context(context)?;
    }
    // 返回统一的 lowering 成功结果。
    Ok(())
}

// 执行不触发 present 的离屏计划，并拒绝未被事务持有的 surface image。
pub(super) fn execute_plan_without_present(
    // 只借用资源、命令和 submit 所需的 Device 角色。
    device: &mut dyn GraphicsDevice,
    // 借用已经完成 lowering 的 FramePlan。
    plan: &FramePlan,
    // 返回离屏 submit 的真实结果。
) -> Result<()> {
    // 通用 acquire 语义不保证可在 present 前重复取得同一 image。
    if plan.targets_surface() {
        // 禁止依赖任何原生 adapter 对无 present surface 写入的偶然容忍。
        return Err(Error::new(
            // 目标生命周期错配属于调用参数错误。
            Errc::InvalidArgument,
            // 指导调用方使用完整 surface 事务或显式离屏纹理。
            "no-present FramePlan requires an offscreen texture target",
        ));
    }
    // texture target 只经过 device-only 执行边界，不触发主 surface acquire。
    plan.execute_offscreen_on_device(device)?;
    // 返回统一的 lowering 成功结果。
    Ok(())
}

// 验证 Renderer 帧执行资格在失败或重复调用时都保持一次性。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../tests/unit/draw/backend/rhi_renderer_execution__lifecycle_tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod lifecycle_tests;
