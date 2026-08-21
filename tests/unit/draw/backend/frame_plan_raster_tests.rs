// FramePlan 的 DrawPacket 动态栅格契约回归测试，由 frame_plan.rs 的测试模块引入。

// 验证 DrawPacket 的非法 viewport 或 scissor 均在 Adapter 前拒绝。
#[test]
fn rejects_invalid_draw_raster_state_before_adapter() {
    // 创建可验证的第一代 surface。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 复用有效计划并注入无法共同量化的 viewport。
    let mut viewport_plan = test_plan(token);
    // 取得 viewport 变异计划的唯一 render pass。
    let FramePlanStep::Pass(pass) = &mut viewport_plan.steps[0] else {
        // 测试基线漂移时立即失败。
        panic!("test plan must start with a render pass");
    };
    // 找到唯一 DrawPacket 以替换其原子栅格事实。
    let packet = pass
        // 遍历当前 pass 的完整命令顺序。
        .commands
        // 可变搜索唯一 Draw 命令。
        .iter_mut()
        // 只投影当前 DrawPacket。
        .find_map(|command| match command {
            // 返回 DrawPacket 的独占借用。
            FramePlanCommand::Draw(packet) => Some(packet),
            // 其它命令不拥有栅格事实。
            _ => None,
        })
        // 有效测试基线必须包含 DrawPacket。
        .expect("test plan must contain a draw");
    // 保留 pipeline、资源和范围，只替换为半像素 viewport。
    *packet = DrawPacket::new(
        // 保留原 pipeline 身份。
        packet.pipeline(),
        // 保留原 Buffer 角色。
        packet.buffers(),
        // 保留原条件采样角色。
        packet.sampling(),
        // 注入两个 Adapter 都不得自行量化的完整栅格状态。
        crate::platform::presentation::rhi::DrawRasterState::new(
            // 使用半像素宽度制造共同值域违例。
            RhiViewport {
                // 非整像素宽度必须被共享层拒绝。
                width: 63.5,
                // 高度保持合法。
                height: 64.0,
            },
            // 明确关闭裁剪，隔离 viewport 失败。
            None,
        ),
        // 保留原绘制范围。
        packet.range(),
    );
    // 创建独立记录 context，观察验证前是否触碰 Adapter。
    let mut viewport_context = recording_context(token);
    // 执行非法 viewport 的计划并要求稳定失败。
    let viewport_error = viewport_plan
        .execute_on_context(&mut viewport_context)
        .expect_err("invalid viewport must fail before adapter");
    // 非共同值域 viewport 属于稳定计划参数错误。
    assert_eq!(viewport_error.code(), Errc::InvalidArgument);
    // 诊断必须明确指出 Draw 的栅格契约无效。
    assert!(viewport_error.what().contains("raster"));
    // 验证失败发生在 activate 之前，设备日志应保持为空。
    assert!(viewport_context.device.log.is_empty());
    // 验证失败计划不得进入最终 present。
    assert_eq!(viewport_context.surface.present_count, 0);

    // 复用有效计划并注入负起点 scissor。
    let mut scissor_plan = test_plan(token);
    // 取得 scissor 变异计划的唯一 render pass。
    let FramePlanStep::Pass(pass) = &mut scissor_plan.steps[0] else {
        // 测试基线漂移时立即失败。
        panic!("test plan must start with a render pass");
    };
    // 找到唯一 DrawPacket 以替换其原子栅格事实。
    let packet = pass
        // 遍历当前 pass 的完整命令顺序。
        .commands
        // 可变搜索唯一 Draw 命令。
        .iter_mut()
        // 只投影当前 DrawPacket。
        .find_map(|command| match command {
            // 返回 DrawPacket 的独占借用。
            FramePlanCommand::Draw(packet) => Some(packet),
            // 其它命令不拥有栅格事实。
            _ => None,
        })
        // 有效测试基线必须包含 DrawPacket。
        .expect("test plan must contain a draw");
    // 保留其它 packet 事实，只替换为不可编码的显式 scissor。
    *packet = DrawPacket::new(
        // 保留原 pipeline 身份。
        packet.pipeline(),
        // 保留原 Buffer 角色。
        packet.buffers(),
        // 保留原条件采样角色。
        packet.sampling(),
        // 使用合法 viewport 与非法 scissor 组成完整栅格状态。
        crate::platform::presentation::rhi::DrawRasterState::new(
            // 复用原 packet 的合法 viewport。
            packet.raster().viewport(),
            // 注入负起点裁剪。
            Some(RhiScissor {
                // 负起点不能进入任一原生 API。
                x: -1,
                // 顶部保持合法。
                y: 0,
                // 宽度保持正数。
                width: 1,
                // 高度保持正数。
                height: 1,
            }),
        ),
        // 保留原绘制范围。
        packet.range(),
    );
    // 创建独立记录 context，观察验证前是否触碰 Adapter。
    let mut scissor_context = recording_context(token);
    // 执行非法 scissor 的计划并要求稳定失败。
    let scissor_error = scissor_plan
        .execute_on_context(&mut scissor_context)
        .expect_err("invalid scissor must fail before adapter");
    // 不可编码 scissor 属于稳定计划参数错误。
    assert_eq!(scissor_error.code(), Errc::InvalidArgument);
    // 诊断必须明确指出 Draw 的栅格契约无效。
    assert!(scissor_error.what().contains("raster"));
    // 验证失败发生在 activate 之前，设备日志应保持为空。
    assert!(scissor_context.device.log.is_empty());
    // 验证失败计划不得进入最终 present。
    assert_eq!(scissor_context.surface.present_count, 0);
}
