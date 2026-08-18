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
    // 合法 Surface 计划必须只 acquire 一次。
    assert_eq!(final_context.surface.acquire_count, 1);

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
    // 离屏目标作用域拒绝不应取得 Surface image。
    assert_eq!(rejected_context.surface.acquire_count, 0);

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
    // 离屏目标解析失败不应取得 Surface image。
    assert_eq!(unsupported_context.surface.acquire_count, 0);
}

// Surface scope 的 texture target 解析失败也必须发生在 acquire 前。
#[test]
fn surface_target_preflight_rejects_before_acquire() {
    // 创建稳定的 Surface generation。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 构造 Surface scope 下显式 texture target 的完整计划。
    let plan = test_plan_for_target(
        // 保留 Surface 代际和最终 damage 事务。
        FramePlan::new(token, crate::core::PresentDamage::Full),
        // 让资源预检必须解析真实 texture target。
        RenderTargetRef::Texture(TextureHandle::from_raw(21)),
    );
    // 注入 texture target 能力解析失败。
    let mut context = recording_context(token);
    // 只影响 target 资源预检，不改变 Surface 生命周期。
    context.device.fail_target_resolution = true;
    // 失败必须发生在 acquire 前。
    let error = plan
        // 通过 Surface 入口验证完整 scope 语义。
        .execute_on_context(&mut context)
        // target 预检必须拒绝不可渲染 texture。
        .expect_err("surface target preflight must fail before acquire");
    // 失败分类必须是共享参数错误。
    assert_eq!(error.code(), Errc::InvalidArgument);
    // 目标预检失败不得触碰 Device 原生命令。
    assert!(context.device.log.is_empty());
    // 目标预检失败必须保持 acquire 次数为零。
    assert_eq!(context.surface.acquire_count, 0);
    // 目标预检失败不得触发 present。
    assert_eq!(context.surface.present_count, 0);
}

// 未被 Draw 消费的 Buffer 上传也必须在 Surface acquire 前完成真实资源预检。
#[test]
fn unused_buffer_upload_preflight_wins_before_acquire() {
    // 创建稳定的 Surface generation。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 从包含完整有效 Draw 的计划开始，避免失败来自计划结构验证。
    let mut plan = test_plan(token);
    // 取得唯一 render pass 以追加一个不会被后续 Draw 消费的上传。
    let FramePlanStep::Pass(pass) = &mut plan.steps[0] else {
        // 测试基线漂移时立即失败。
        panic!("test plan must start with a render pass");
    };
    // 使用独立身份标记必须被资源预检扫描的尾部上传。
    let unused_buffer = BufferHandle::from_raw(99);
    // 在 Draw 之后追加合法类型化载荷，证明预检覆盖所有上传而非只覆盖 Draw 依赖。
    pass.push(FramePlanCommand::UploadVertex {
        // 使用不会出现在任何 DrawPacket 中的 Buffer 句柄。
        buffer: unused_buffer,
        // 载荷本身保持完整且有限，让失败唯一来自真实资源查询。
        data: FrameVertexPayload::position_f32x2([0.0, 0.0, 1.0, 1.0]),
    });
    // 创建同时能够注入资源错误和 Surface lost 的组合 context。
    let mut context = recording_context(token);
    // 为未消费顶点上传提供用途相同但步长为四的真实资源描述。
    context.device.upload_preflight_desc = Some((
        // 只让尾部未消费上传消费本次描述。
        unused_buffer,
        // 十六字节容量能被四字节步长整除，失败必须来自类型化 float2 步长错配。
        BufferDesc::vertex(16, 4),
    ));
    // 同时安排 acquire 失败，锁定共享参数错误的优先级。
    context.surface.fail_acquire = true;
    // 执行必须在取得 Surface image 前发现未消费上传的资源错误。
    let error = plan
        // 使用完整 Surface 入口覆盖 acquire 与 Device 激活边界。
        .execute_on_context(&mut context)
        // 未消费上传不得逃过只读预检。
        .expect_err("unused buffer upload must fail before acquire");
    // 顶点元素 ABI 错配必须优先返回稳定共享参数分类。
    assert_eq!(error.code(), Errc::InvalidArgument);
    // 诊断必须证明失败来自元素 ABI，而非后续 Surface lost。
    assert!(error.what().contains("element stride"));
    // 上传预检失败不得触发 Surface acquire。
    assert_eq!(context.surface.acquire_count, 0);
    // 上传预检失败不得进入 activate、pass 或 submit。
    assert!(context.device.log.is_empty());
    // 失败计划不得触发最终 present。
    assert_eq!(context.surface.present_count, 0);
}

// indexed Draw 必须拥有当前 pass 的同 Buffer 类型化内容与完整范围。
#[test]
fn indexed_draw_requires_owned_index_upload_and_range() {
    // 创建稳定的 Surface generation。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 先构造一个内容和范围都合法的索引计划。
    let mut missing_upload = test_indexed_plan(token, [0, 1, 2], 3, 0);
    // 取得唯一 pass 以模拟只绑定索引 Buffer 却不交付内容的旧路径。
    let FramePlanStep::Pass(pass) = &mut missing_upload.steps[0] else {
        // fixture 结构漂移时立即失败。
        panic!("indexed test plan must contain a render pass");
    };
    // 移除类型化索引上传，保留 DrawRange 绑定。
    pass.commands
        // 只过滤新的索引内容命令。
        .retain(|command| !matches!(command, FramePlanCommand::UploadIndex { .. }));
    // 计划验证必须在进入 Device 前拒绝无内容的 indexed Draw。
    let missing_error = missing_upload
        // 直接调用共享 FramePlan 门禁。
        .validate()
        // 无类型化索引上传不得通过。
        .expect_err("indexed draw without typed upload must fail");
    // 错误分类保持为跨 Adapter 共享的参数错误。
    assert_eq!(missing_error.code(), Errc::InvalidArgument);
    // 诊断必须明确指向缺失的类型化索引内容。
    assert!(missing_error.what().contains("typed index upload"));

    // 构造 DrawRange 比已交付索引序列多读一项的计划。
    let range_overflow = test_indexed_plan(token, [0, 1, 2], 4, 0);
    // 越过类型化载荷的索引范围必须被拒绝。
    let range_error = range_overflow
        // 只使用共享计划验证，不进入任一 Adapter。
        .validate()
        // 越界范围必须生成稳定错误。
        .expect_err("indexed draw range beyond typed upload must fail");
    // 范围错误同样属于共享参数契约。
    assert_eq!(range_error.code(), Errc::InvalidArgument);
    // 诊断必须区分索引序列越界与顶点内容越界。
    assert!(range_error.what().contains("index range exceeds typed upload"));
}

// indexed Draw 选中的每个值都必须落在当前类型化顶点载荷内。
#[test]
fn indexed_draw_rejects_vertex_content_overflow() {
    // 创建稳定的 Surface generation。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 基线只有三个顶点，索引值三将读取第四个顶点。
    let vertex_overflow = test_indexed_plan(token, [0, 1, 3], 3, 0);
    // 计划验证必须从类型化顶点内容派生可访问上界。
    let error = vertex_overflow
        // 不借助 Adapter 资源表判断内容范围。
        .validate()
        // 越界顶点访问必须在 Device 前失败。
        .expect_err("indexed draw vertex beyond typed upload must fail");
    // 内容范围错误使用稳定参数分类。
    assert_eq!(error.code(), Errc::InvalidArgument);
    // 诊断必须指向 indexed Draw 的顶点越界。
    assert!(error.what().contains("indexed draw vertex exceeds typed upload"));
}

// 合法索引内容必须且只能在 Device 边界编码，并在 acquire 前完成资源预检。
#[test]
fn indexed_upload_executes_and_preflights_before_acquire() {
    // 创建稳定的 Surface generation。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 构造只读取三个已交付顶点的合法索引计划。
    let plan = test_indexed_plan(token, [0, 1, 2], 3, 0);
    // 创建允许所有资源查询与执行的组合 context。
    let mut accepted = recording_context(token);
    // 合法计划必须完成唯一 Surface 事务。
    assert!(plan.execute_on_context(&mut accepted).is_ok());
    // 顶点、索引与 Uniform 三次上传必须保持 FramePlan 命令顺序。
    assert_eq!(
        // 收集记录型 Device 的完整原语执行轨迹。
        accepted.device.log.iter().copied().collect::<Vec<_>>(),
        // 索引上传必须位于 Draw 之前的第二个 Buffer 更新位置。
        vec![
            // 先激活唯一 Device owner。
            "activate",
            // 激活后执行设备健康检查。
            "maintain",
            // 只在资源预检完成后开始 pass。
            "begin_pass",
            // 先交付已冻结的 viewport。
            "viewport",
            // 再交付已冻结的 scissor。
            "scissor",
            // 第一次更新交付类型化顶点。
            "update_buffer",
            // 第二次更新交付类型化索引。
            "update_buffer",
            // 第三次更新交付类型化 Uniform。
            "update_buffer",
            // 三类内容都生效后才允许 Draw。
            "draw",
            // Draw 完成后收束当前 pass。
            "end_pass",
            // 全部步骤完成后唯一提交。
            "submit"
        ]
    );
    // 合法索引帧必须只取得一次 Surface image。
    assert_eq!(accepted.surface.acquire_count, 1);
    // 合法索引帧必须只呈现一次。
    assert_eq!(accepted.surface.present_count, 1);

    // 创建同时注入索引资源错误和 Surface acquire 错误的 context。
    let mut rejected = recording_context(token);
    // 为身份五的索引上传提供用途相同但两字节步长的真实描述。
    rejected.device.upload_preflight_desc = Some((
        // 匹配索引 fixture 的 Buffer 身份。
        BufferHandle::from_raw(5),
        // 十二字节容量对两字节步长仍然对齐，但与 Uint32 格式不一致。
        BufferDesc::index(12, 2),
    ));
    // 同时让 Surface acquire 失败，锁定资源预检的优先级。
    rejected.surface.fail_acquire = true;
    // 索引资源错误必须在取得 Surface image 前返回。
    let error = plan
        // 通过完整 Surface 入口覆盖只读预检和生命周期边界。
        .execute_on_context(&mut rejected)
        // 真实索引 Buffer 不满足契约时必须失败。
        .expect_err("index upload preflight must fail before acquire");
    // 索引元素 ABI 错配必须优先于 Surface lost。
    assert_eq!(error.code(), Errc::InvalidArgument);
    // 诊断必须证明 Uint32 payload 与真实索引资源的元素 ABI 错配。
    assert!(error.what().contains("element stride"));
    // 预检失败后不得调用 acquire。
    assert_eq!(rejected.surface.acquire_count, 0);
    // 预检失败后不得激活 Device 或执行原生命令。
    assert!(rejected.device.log.is_empty());
    // 预检失败后不得呈现任何帧。
    assert_eq!(rejected.surface.present_count, 0);
}

// Copy preflight 失败必须发生在 Device activate 前且不留下日志。
#[test]
fn texture_copy_preflight_rejects_before_device() {
    // 创建稳定的 Surface generation。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 构造包含完整 render pass 与普通 copy 的计划。
    let mut plan = test_plan(token);
    // 追加两个不同纹理之间的合法传输几何。
    plan.push_copy(TextureCopy::new(
        // 使用稳定的源纹理句柄。
        TextureHandle::from_raw(11),
        // 使用稳定的目标纹理句柄。
        TextureHandle::from_raw(12),
        // 使用四乘四的完整传输区域。
        RhiTextureTransfer::from_xy(0, 0, 0, 0, RhiExtent::new(4, 4)),
    ));
    // 注入 copy 预检失败但不改变其它 Device 行为。
    let mut rejected = recording_context(token);
    rejected.device.fail_copy_preflight = true;
    // 失败必须发生在任何 Device 原语之前。
    let error = plan
        .execute_on_context(&mut rejected)
        .expect_err("copy preflight must fail before device activation");
    // 失败分类必须是共享参数错误。
    assert_eq!(error.code(), Errc::InvalidArgument);
    // preflight 失败不得留下 activate、pass 或 submit 日志。
    assert!(rejected.device.log.is_empty());
    // preflight 失败不得触发 present。
    assert_eq!(rejected.surface.present_count, 0);
    // Surface 资源预检失败必须早于 acquire。
    assert_eq!(rejected.surface.acquire_count, 0);
    // 关闭失败注入后，同一合法计划的 preflight 必须放行。
    let mut accepted = recording_context(token);
    assert!(plan.execute_on_context(&mut accepted).is_ok());
}

// Move preflight 失败必须发生在 Device activate 前且不留下日志。
#[test]
fn texture_move_preflight_rejects_before_device() {
    // 创建稳定的 Surface generation。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 构造包含完整 render pass 与重叠安全 move 的计划。
    let mut plan = test_plan(token);
    // 追加同一纹理上的合法移动区域。
    plan.push_move(TextureMove::new(
        // 使用稳定的源纹理句柄。
        TextureHandle::from_raw(13),
        // 使用相同句柄表达 memmove 语义。
        TextureHandle::from_raw(13),
        // 使用四乘四的完整移动区域。
        RhiTextureTransfer::from_xy(0, 0, 0, 0, RhiExtent::new(4, 4)),
    ));
    // 注入 move 预检失败但不改变其它 Device 行为。
    let mut rejected = recording_context(token);
    rejected.device.fail_move_preflight = true;
    // 失败必须发生在任何 Device 原语之前。
    let error = plan
        .execute_on_context(&mut rejected)
        .expect_err("move preflight must fail before device activation");
    // 失败分类必须是共享参数错误。
    assert_eq!(error.code(), Errc::InvalidArgument);
    // preflight 失败不得留下 activate、pass 或 submit 日志。
    assert!(rejected.device.log.is_empty());
    // preflight 失败不得触发 present。
    assert_eq!(rejected.surface.present_count, 0);
    // Surface 资源预检失败必须早于 acquire。
    assert_eq!(rejected.surface.acquire_count, 0);
    // 关闭失败注入后，同一合法计划的 preflight 必须放行。
    let mut accepted = recording_context(token);
    assert!(plan.execute_on_context(&mut accepted).is_ok());
}

// Draw 资源预检失败必须发生在 Device activate 前且不留下日志。
#[test]
fn draw_resource_preflight_rejects_before_device() {
    // 创建稳定的 Surface generation。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 构造包含完整 Draw 的合法计划。
    let plan = test_plan(token);
    // 注入 Draw 真实资源预检失败。
    let mut rejected = recording_context(token);
    // 只影响共享资源预检，不改变其它 Device 行为。
    rejected.device.fail_draw_preflight = true;
    // 失败必须发生在任何 Device 原语之前。
    let error = plan
        // 通过普通 Surface 入口验证 acquire 前的资源边界。
        .execute_on_context(&mut rejected)
        // Draw 资源预检必须在 activate 前失败。
        .expect_err("draw resource preflight must fail before device activation");
    // 失败分类必须是共享参数错误。
    assert_eq!(error.code(), Errc::InvalidArgument);
    // preflight 失败不得留下 activate、pass 或 submit 日志。
    assert!(rejected.device.log.is_empty());
    // preflight 失败不得触发 present。
    assert_eq!(rejected.surface.present_count, 0);
    // Surface 资源预检失败必须早于 acquire。
    assert_eq!(rejected.surface.acquire_count, 0);
    // 关闭失败注入后，同一合法计划必须通过预检并完成执行。
    let mut accepted = recording_context(token);
    // 合法资源描述由测试 Device 预检放行。
    assert!(plan.execute_on_context(&mut accepted).is_ok());
}
