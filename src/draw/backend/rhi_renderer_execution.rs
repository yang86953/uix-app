//! FramePlan 在 surface 与 offscreen 目标上的统一执行边界。

// 引入统一执行结果与稳定参数错误。
use crate::core::error::{Errc, Error, Result};
// 引入只属于最终 Surface present 的 damage。
use crate::core::PresentDamage;
// 引入组合 context、正交 Device/Surface 角色与显式纹理目标。
use crate::native::present::rhi::{
    GraphicsContextRhi, GraphicsDevice, GraphicsSurface, TextureHandle,
};

// 引入已经完成 lowering 且自有执行作用域的帧计划与目标引用。
use super::super::frame_plan::{FramePlan, RenderTargetRef};

// 封闭 Renderer 一次执行所能拥有的 Surface 或 Offscreen 生命周期。
pub(crate) enum RhiRendererFrame<'a> {
    // 完整 Surface 帧原子拥有组合 context、damage 与可选观察钩子。
    Surface {
        // 借用唯一原生 context owner。
        context: &'a mut dyn GraphicsContextRhi,
        // 保存只由最终 present 消费的 damage。
        damage: PresentDamage,
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
        // 原子建立带观察边界的完整 Surface 生命周期。
        Self::Surface {
            // 保存组合 context 的唯一可变借用。
            context,
            // 保存最终 present damage。
            damage,
            // 观察钩子只能存在于 Surface 变体。
            before_present: Some(before_present),
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
        Self::Offscreen {
            // 保存 Device 独占借用。
            device,
            // 保存显式纹理目标。
            target,
        }
    }

    // 返回当前帧唯一允许写入的计划目标。
    pub(crate) const fn render_target(&self) -> RenderTargetRef {
        // 从封闭变体投影目标，不接收第二个选择器。
        match self {
            // Surface 帧只能写入本次 acquire 的 image。
            Self::Surface { .. } => RenderTargetRef::Surface,
            // Offscreen 帧只能写入自己保存的纹理目标。
            Self::Offscreen { target, .. } => RenderTargetRef::Texture(*target),
        }
    }

    // 可变借用当前帧允许使用的 Device 角色。
    pub(crate) fn device(&mut self) -> &mut dyn GraphicsDevice {
        // 两个变体都只暴露 Device 窄视图。
        match self {
            // Surface 帧经组合 context 取得可变 Device。
            Self::Surface { context, .. } => context.device(),
            // Offscreen 帧直接返回独立 Device。
            Self::Offscreen { device, .. } => &mut **device,
        }
    }

    // 由封闭帧事实构造匹配的类型化 FramePlan。
    pub(crate) fn plan(&self) -> FramePlan {
        // 计划作用域只能由当前帧变体决定。
        match self {
            // Surface 计划冻结当前代际与最终 damage。
            Self::Surface {
                context, damage, ..
            } => FramePlan::new(context.surface_ref().token(), damage.clone()),
            // Offscreen 计划不保存 SurfaceToken 或 PresentDamage。
            Self::Offscreen { .. } => FramePlan::offscreen(),
        }
    }

    // 按封闭帧角色执行已经完成 lowering 的计划。
    pub(crate) fn execute(&mut self, plan: &FramePlan) -> Result<()> {
        // Surface 与 Offscreen 进入互不重叠的执行边界。
        match self {
            // Surface 帧消费可选观察钩子并完成唯一最终 present。
            Self::Surface {
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
            Self::Offscreen { device, .. } => {
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
        // 禁止依赖 D3D11/OpenGL 对无 present surface 写入的偶然容忍。
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
