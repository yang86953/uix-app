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

    // 返回当前帧唯一允许写入的计划目标。
    pub(crate) const fn render_target(&self) -> RenderTargetRef {
        // 从封闭变体投影目标，不接收第二个选择器。
        match &self.role {
            // Surface 帧只能写入本次 acquire 的 image。
            RhiRendererFrameRole::Surface { .. } => RenderTargetRef::Surface,
            // Offscreen 帧只能写入自己保存的纹理目标。
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

    // 可变借用由当前帧唯一拥有的类型化计划。
    pub(crate) fn plan_mut(&mut self) -> &mut FramePlan {
        // 调用方只能追加当前帧最终会执行的同一份计划。
        &mut self.plan
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

// 验证 Renderer 帧执行资格在失败或重复调用时都保持一次性。
#[cfg(test)]
mod lifecycle_tests {
    // 引入单线程共享计数器的可变单元。
    use std::cell::Cell;
    // 引入测试与记录设备共享计数器所有权。
    use std::rc::Rc;

    // 引入统一错误类型和测试结果别名。
    use crate::core::error::{Errc, Result};
    // 引入 FramePlan 构造所需的最小类型化命令。
    use crate::draw::backend::frame_plan::{
        FramePlan, FramePlanCommand, RenderPassPlan, RenderTargetRef,
    };
    // 引入 RecordingDevice 所需的薄 RHI 原语。
    use crate::native::present::rhi::{
        DrawPacket, GraphicsDevice, GraphicsDeviceCapabilities, LoadAction, RenderTargetHandle,
        RhiColor, RhiScissor, SubmissionHandle, TextureCopy, TextureHandle, TextureMove,
    };
    // 引入待验证的 Renderer 帧 Component。
    use super::RhiRendererFrame;

    // 记录最小离屏执行器的 submit 次数。
    struct RecordingDevice {
        // 保存设备能力以允许计划进入执行器。
        capabilities: GraphicsDeviceCapabilities,
        // 与测试共享全部 Device 原语调用次数。
        operation_count: Rc<Cell<usize>>,
        // 保存真实提交次数以检测重复执行。
        submit_count: usize,
    }

    // 为 RecordingDevice 提供统一调用计数入口。
    impl RecordingDevice {
        // 记录一次 Device 契约访问。
        fn record_operation(&self) {
            // 单线程测试直接推进共享调用计数。
            self.operation_count.set(self.operation_count.get() + 1);
        }
    }

    // 提供测试所需的最小 GraphicsDevice 实现。
    impl GraphicsDevice for RecordingDevice {
        // 返回完整 GPU 基线，隔离本测试的生命周期断言。
        fn device_capabilities(&self) -> GraphicsDeviceCapabilities {
            // 能力读取也属于不得在第二次执行发生的 Device 调用。
            self.record_operation();
            // 返回构造时冻结的能力快照。
            self.capabilities
        }

        // 测试设备把离屏 fixture texture 提升为已验证目标。
        fn resolve_render_target(&self, texture: TextureHandle) -> Result<RenderTargetHandle> {
            // 记录这次执行前的目标能力解析。
            self.record_operation();
            // 测试 mock 直接返回稳定的可渲染目标身份。
            Ok(RenderTargetHandle::for_test(texture))
        }

        // 接受离屏 render pass 开始。
        fn begin_render_pass(
            &mut self,
            _target: RenderTargetHandle,
            _load: LoadAction,
        ) -> Result<()> {
            // 记录 render pass 开始调用。
            self.record_operation();
            // 该测试只关注执行门禁，不记录 pass 细节。
            Ok(())
        }

        // 接受计划中的局部清理原语。
        fn clear_rect(&mut self, _color: RhiColor, _scissor: RhiScissor) -> Result<()> {
            // 记录真实进入 pass 命令边界的调用。
            self.record_operation();
            // 测试设备不保存像素，只验证计划生命周期。
            Ok(())
        }

        // 接受计划中的 draw 原语。
        fn draw(&mut self, _packet: DrawPacket) -> Result<()> {
            // 记录意外 draw 调用。
            self.record_operation();
            // 本测试计划不包含 draw，此实现满足薄 Device 契约。
            Ok(())
        }

        // 接受 pass 外纹理复制。
        fn copy_texture(&mut self, _copy: TextureCopy) -> Result<()> {
            // 记录意外 copy 调用。
            self.record_operation();
            // 本测试计划不包含 copy。
            Ok(())
        }

        // 接受纹理区域移动。
        fn move_texture_region(&mut self, _movement: TextureMove) -> Result<()> {
            // 记录意外 move 调用。
            self.record_operation();
            // 本测试计划不包含 move。
            Ok(())
        }

        // 接受 render pass 结束。
        fn end_render_pass(&mut self) -> Result<()> {
            // 记录 render pass 收尾调用。
            self.record_operation();
            // 本测试只关注最终提交次数。
            Ok(())
        }

        // 记录一次成功 submit。
        fn submit(&mut self) -> Result<SubmissionHandle> {
            // 记录提交原语调用。
            self.record_operation();
            // 每次真实进入提交边界都递增计数。
            self.submit_count += 1;
            // 返回非零测试提交身份。
            Ok(SubmissionHandle::from_raw(self.submit_count as u64))
        }

        // 记录 owner-thread context 激活。
        fn activate(&mut self) -> Result<()> {
            // 第二次执行不得再次进入激活边界。
            self.record_operation();
            // RecordingDevice 不需要真实原生 context。
            Ok(())
        }

        // 记录设备健康维护。
        fn maintain(&mut self) -> Result<()> {
            // 第二次执行不得再次进入健康门禁。
            self.record_operation();
            // RecordingDevice 始终保持健康。
            Ok(())
        }
    }

    // 向 Renderer 帧唯一拥有的计划追加一个有效离屏 pass。
    fn append_offscreen_pass(frame: &mut RhiRendererFrame<'_>) {
        // 创建显式纹理目标的 render pass。
        let mut pass = RenderPassPlan::new(
            // 使用当前帧唯一投影出的类型化 texture 目标。
            frame.render_target(),
            // 使用有限的透明清理颜色。
            LoadAction::Clear(RhiColor::from_premultiplied_rgba([0.0, 0.0, 0.0, 1.0])),
        );
        // 追加一条最小合法命令，满足 FramePlan 不接受空 pass 的共享契约。
        pass.push(FramePlanCommand::ClearRect {
            // 使用有限的预乘颜色执行局部清理。
            color: RhiColor::from_premultiplied_rgba([0.0, 0.0, 0.0, 1.0]),
            // 使用非空且全部为有限整数的测试矩形。
            scissor: RhiScissor {
                // 从目标左上角开始。
                x: 0,
                // 从目标顶边开始。
                y: 0,
                // 清理一个像素宽度。
                width: 1,
                // 清理一个像素高度。
                height: 1,
            },
        });
        // 把唯一 pass 交给当前帧内部计划。
        frame.plan_mut().push_pass(pass);
    }

    // 第二次 Renderer 帧执行必须拒绝且不得再次 submit。
    #[test]
    fn renderer_frame_rejects_second_execution_without_resubmit() {
        // 创建可在帧借用期间独立观察的 Device 调用计数。
        let operation_count = Rc::new(Cell::new(0));
        // 创建具备完整 GPU 基线的 recording device。
        let mut device = RecordingDevice {
            // 使用共享基线能力，避免能力错误干扰生命周期断言。
            capabilities: GraphicsDeviceCapabilities::full_gpu_baseline(),
            // 与测试共享全部 Device 调用计数。
            operation_count: Rc::clone(&operation_count),
            // 初始尚未发生任何提交。
            submit_count: 0,
        };
        // 创建只允许写入显式纹理的 Renderer 帧。
        let mut frame = RhiRendererFrame::offscreen(
            // 借用唯一 recording device owner。
            &mut device,
            // 传入与计划匹配的纹理身份。
            crate::native::present::rhi::TextureHandle::from_raw(9),
        );
        // 向当前帧内部计划追加第一次执行使用的有效 pass。
        append_offscreen_pass(&mut frame);
        // 第一次执行必须成功并产生一次提交。
        frame
            // 执行当前帧唯一拥有的有效计划。
            .execute()
            // 失败时保留 typed error 作为测试诊断。
            .expect("owned offscreen FramePlan must execute");
        // 保存首轮执行完成后的全部 Device 调用次数。
        let operations_after_first_execution = operation_count.get();
        // 首轮有效计划必须真实进入 Device 执行边界。
        assert!(operations_after_first_execution > 0);
        // 第二次执行必须在 Device 调用前返回 InvalidState。
        let error = frame
            // 执行入口只能再次尝试同一帧内部计划。
            .execute()
            .expect_err("second execution must fail");
        // 第二次执行必须使用稳定的生命周期错误分类。
        assert_eq!(error.code(), Errc::InvalidState);
        // 第二次执行不得新增任何 Device 契约访问。
        assert_eq!(operation_count.get(), operations_after_first_execution);
        // 丢弃帧后重新取得 Device 统计，验证执行后清理借用仍可用。
        drop(frame);
        // 重复执行不得增加提交次数。
        assert_eq!(device.submit_count, 1);
    }

    // 首次失败的执行尝试也必须消费帧且不允许重试。
    #[test]
    fn failed_renderer_frame_execution_is_also_consumed() {
        // 创建可在帧借用期间独立观察的 Device 调用计数。
        let operation_count = Rc::new(Cell::new(0));
        // 创建具备完整 GPU 基线的 recording device。
        let mut device = RecordingDevice {
            // 能力事实保持有效，确保失败只来自空计划。
            capabilities: GraphicsDeviceCapabilities::full_gpu_baseline(),
            // 与测试共享全部 Device 调用计数。
            operation_count: Rc::clone(&operation_count),
            // 初始尚未发生任何提交。
            submit_count: 0,
        };
        // 创建只允许写入显式纹理的 Renderer 帧。
        let mut frame = RhiRendererFrame::offscreen(
            // 借用唯一 recording device owner。
            &mut device,
            // 传入稳定的离屏纹理身份。
            crate::native::present::rhi::TextureHandle::from_raw(9),
        );
        // 新建帧内部的空离屏计划会在任何 Device 访问前稳定验证失败。
        // 首轮失败必须保留计划参数错误。
        let first_error = frame
            // 尝试执行内部缺少 render pass 的计划。
            .execute()
            // 空计划必须失败而不是伪造提交。
            .expect_err("empty FramePlan must fail");
        // 首轮错误仍来自 FramePlan 自身验证。
        assert_eq!(first_error.code(), Errc::InvalidArgument);
        // 参数验证失败不得触碰任何 Device 原语。
        assert_eq!(operation_count.get(), 0);
        // 同一帧的第二次尝试必须被生命周期门禁拒绝。
        let second_error = frame
            // 即使内部计划仍然无效，也应先命中 Consumed 状态。
            .execute()
            // 第二次执行必须失败。
            .expect_err("failed execution must still consume the frame");
        // 第二次错误必须稳定升级为生命周期状态错误。
        assert_eq!(second_error.code(), Errc::InvalidState);
        // 第二次尝试同样不得触碰任何 Device 原语。
        assert_eq!(operation_count.get(), 0);
        // 丢弃帧后重新检查提交事实。
        drop(frame);
        // 两次失败均不得产生 submit。
        assert_eq!(device.submit_count, 0);
    }
}
