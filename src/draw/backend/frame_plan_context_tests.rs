// 验证 final 与 offscreen 两种合法入口复用同一命令顺序。
#[test]
fn context_execution_modes_share_one_ordered_device_path() {
    // 创建所有执行模式共享的 surface generation。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 创建执行最终 present 的组合 context。
    let mut final_context = recording_context(token);
    // 通过组合入口执行常规 surface 计划。
    let final_result = test_plan(token).execute_on_context(&mut final_context);
    // 最终模式必须形成可消费的 FrameCommit。
    assert!(final_result.is_ok());
    // 最终模式必须保持唯一执行器定义的命令顺序。
    assert_eq!(
        final_context.device.log.iter().copied().collect::<Vec<_>>(),
        vec![
            "activate",
            "maintain",
            "begin_pass",
            "viewport",
            "scissor",
            "update_buffer",
            "update_buffer",
            "draw",
            "end_pass",
            "submit"
        ]
    );
    // 只有最终模式允许触发一次 present。
    assert_eq!(final_context.surface.present_count, 1);

    // 创建只提交离屏纹理计划的组合 context。
    let mut offscreen_context = recording_context(token);
    // 构造不接收 SurfaceToken 的显式 texture 计划。
    let offscreen_plan = test_offscreen_plan(TextureHandle::from_raw(9));
    // 计划自身必须证明它不会进入 acquire 或 present。
    assert!(!offscreen_plan.targets_surface());
    // 执行离屏计划并仅取得 submission。
    let offscreen_result =
        offscreen_plan.execute_offscreen_on_device(&mut offscreen_context.device);
    // 合法离屏计划必须提交成功。
    assert!(offscreen_result.is_ok());
    // 离屏模式必须继续复用相同的命令执行顺序。
    assert_eq!(
        offscreen_context
            .device
            .log
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![
            "activate",
            "maintain",
            "begin_pass",
            "viewport",
            "scissor",
            "update_buffer",
            "update_buffer",
            "draw",
            "end_pass",
            "submit"
        ]
    );
    // 离屏模式不得触发最终 present。
    assert_eq!(offscreen_context.surface.present_count, 0);

    // 创建用于验证离屏目标前置拒绝的组合 context。
    let mut rejected_context = recording_context(token);
    // 故意把逻辑 Surface 计划交给 offscreen 模式。
    let rejected_result =
        test_plan(token).execute_offscreen_on_device(&mut rejected_context.device);
    // 非法目标必须返回 typed failure。
    assert!(rejected_result.is_err());
    // 前置目标校验必须保证没有任何 device 副作用。
    assert!(rejected_context.device.log.is_empty());
    // 被拒绝的离屏计划也不得触发 present。
    assert_eq!(rejected_context.surface.present_count, 0);

    // 创建用于验证 texture 目标能力前置拒绝的组合 context。
    let mut unsupported_context = recording_context(token);
    // 安排共享资源表在目标解析阶段返回稳定失败。
    unsupported_context.device.fail_target_resolution = true;
    // 构造携带显式 texture target 的离屏计划。
    let unsupported_plan = test_offscreen_plan(TextureHandle::from_raw(10));
    // 执行必须把目标能力错误直接返回调用方。
    let unsupported_result =
        unsupported_plan.execute_offscreen_on_device(&mut unsupported_context.device);
    // 不可渲染 texture 必须在进入任何原生命令前失败。
    assert_eq!(
        unsupported_result.expect_err("unsupported target must fail before activation").code(),
        Errc::InvalidArgument,
    );
    // 目标解析失败不得触发 activate、maintain 或任何 pass 命令。
    assert!(unsupported_context.device.log.is_empty());
    // 目标解析失败也不得触发 Surface present。
    assert_eq!(unsupported_context.surface.present_count, 0);
}
