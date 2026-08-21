// 引入单线程共享计数器的可变单元。
use std::cell::Cell;
// 引入测试与记录设备共享计数器所有权。
use std::rc::Rc;

// 引入统一错误类型和测试结果别名。
use crate::core::error::{Errc, Result};
// 引入 Renderer pass 夹具所需的最小类型化命令。
use crate::draw::backend::frame_plan::FramePlanCommand;
// 引入 RecordingDevice 所需的薄 RHI 原语。
use crate::platform::presentation::rhi::{
    DrawPacket, GraphicsDevice, GraphicsDeviceCapabilities, LoadAction, RenderTargetHandle,
    RhiColor, RhiScissor, SubmissionHandle, TextureCopy, TextureHandle, TextureMove,
};
// 引入待验证的 Renderer 帧 Widget。
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
    fn begin_render_pass(&mut self, _target: RenderTargetHandle, _load: LoadAction) -> Result<()> {
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
    // 创建不携带纹理目标或加载动作的 Renderer pass 命令包。
    let mut pass = frame.new_pass();
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
    // 由当前 Frame owner 绑定唯一纹理目标与加载动作。
    frame.push_pass(
        // 使用有限的透明清理颜色。
        LoadAction::Clear(RhiColor::from_premultiplied_rgba([0.0, 0.0, 0.0, 1.0])),
        // 转移目标无关的命令包。
        pass,
    );
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
        crate::platform::presentation::rhi::TextureHandle::from_raw(9),
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
        crate::platform::presentation::rhi::TextureHandle::from_raw(9),
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
