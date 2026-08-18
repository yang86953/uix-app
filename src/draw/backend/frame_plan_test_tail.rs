// 帧计划契约测试的外部载荷，由 frame_plan.rs 的 mod tests include 引入。
// 覆盖纹理移动/清理顺序、提交失败、代际拒绝与能力缺口分类。
// 验证纹理区域移动位于 pass 顺序中且仍只触发一次 submit/present。
#[test]
fn executes_texture_move_in_order() {
    // 创建第一代 surface。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 构造带最小绘制 pass 的计划。
    let mut plan = test_plan(token);
    // 在 pass 后追加一个同纹理重叠移动，模拟 retained framebuffer scroll。
    plan.push_move(TextureMove::new(
        // 从稳定测试纹理读取。
        TextureHandle::from_raw(11),
        // 写回同一个稳定测试纹理。
        TextureHandle::from_raw(11),
        // 使用不可拆分的源、目标与尺寸传输。
        RhiTextureTransfer::from_xy(0, 0, 1, 0, RhiExtent::new(2, 2)),
    ));
    // 创建原子拥有 device 与 surface 的记录型 context。
    let mut context = recording_context(token);
    // 执行计划并要求最终提交成功。
    assert!(plan.execute_on_context(&mut context).is_ok());
    // 验证移动发生在 pass 结束之后、唯一 submit 之前。
    assert_eq!(
        context.device.log.into_iter().collect::<Vec<_>>(),
        vec![
            "begin_pass",
            "viewport",
            "scissor",
            "update_buffer",
            "update_buffer",
            "draw",
            "end_pass",
            "move",
            "submit"
        ]
    );
    // 验证移动计划没有引入第二次 present。
    assert_eq!(context.surface.present_count, 1);
}

// 验证局部清理保留在 pass 内的原始命令顺序。
#[test]
fn executes_clear_rect_in_order() {
    // 创建第一代 surface。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 构造只包含 viewport、clear rect 和 draw 的计划。
    let mut pass = RenderPassPlan::new(RenderTargetRef::Surface, LoadAction::Load);
    // 追加物理 viewport。
    pass.push(FramePlanCommand::SetViewport(RhiViewport {
        width: 64.0,
        height: 64.0,
    }));
    // 追加透明局部清理。
    pass.push(FramePlanCommand::ClearRect {
        color: RhiColor::transparent(),
        scissor: RhiScissor {
            x: 2,
            y: 3,
            width: 10,
            height: 11,
        },
    });
    // 追加与 SolidMesh 契约一致的类型化 Uniform 上传。
    pass.push(FramePlanCommand::UploadUniform {
        // 使用稳定测试 Uniform buffer。
        buffer: BufferHandle::from_raw(4),
        // 构造与当前 viewport 一致的 Mesh 常量。
        data: FrameUniformPayload::Mesh(RhiMeshRasterParams::new(
            // 使用当前 pass 的物理 viewport。
            RhiViewport {
                // 保存测试宽度。
                width: 64.0,
                // 保存测试高度。
                height: 64.0,
            },
            // 使用确定不透明白色。
            [1.0, 1.0, 1.0, 1.0],
        )),
    });
    // 构造一个最小非空 draw packet。
    let mut packet = DrawPacket::triangles(
        // 句柄与 SolidMesh 共享语义必须不可拆地进入计划。
        PipelineBinding::new(PipelineHandle::from_raw(1), PipelineKind::SolidMesh),
        // 保留三个顶点的最小非空范围。
        3,
    );
    // 绑定稳定的静态顶点 buffer 身份。
    packet.vertex_buffer = BufferHandle::from_raw(3);
    // 绑定前序类型化上传的 Uniform buffer。
    packet.uniform_buffer = Some(BufferHandle::from_raw(4));
    // 追加完整绘制包。
    pass.push(FramePlanCommand::Draw(packet));
    // 创建并追加唯一 surface pass。
    let mut plan = FramePlan::new(token, crate::core::PresentDamage::Full);
    // 保持局部清理位于 draw 之前。
    plan.push_pass(pass);
    // 创建原子拥有 device 与 surface 的记录型 context。
    let mut context = recording_context(token);
    // 执行计划并要求最终提交成功。
    let result = plan.execute_on_context(&mut context);
    // 保留底层错误内容，便于区分计划验证和 mock adapter 失败。
    assert!(result.is_ok(), "clear rect plan failed: {result:?}");
    // 验证局部清理没有被提升到 pass 外或静默跳过。
    assert_eq!(
        context.device.log.into_iter().collect::<Vec<_>>(),
        vec![
            "begin_pass",
            "viewport",
            "clear_rect",
            "update_buffer",
            "draw",
            "end_pass",
            "submit"
        ]
    );
    // 验证清理计划仍只触发一次最终 present。
    assert_eq!(context.surface.present_count, 1);
}

// 验证类型化顶点上传不能进入不匹配的 pipeline 顶点 ABI。
#[test]
fn rejects_vertex_upload_layout_mismatch_before_adapter() {
    // 创建可验证的第一代 surface。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 复用完整有效计划作为最小变异基线。
    let mut plan = test_plan(token);
    // 取得唯一 render pass 以注入布局错配。
    let FramePlanStep::Pass(pass) = &mut plan.steps[0] else {
        // 测试基线漂移时立即失败。
        panic!("test plan must start with a render pass");
    };
    // 找到当前 draw 实际引用的顶点上传。
    let upload = pass
        // 遍历 pass 内可变命令。
        .commands
        // 创建顺序可变迭代器。
        .iter_mut()
        // 选择唯一顶点上传。
        .find(|command| matches!(command, FramePlanCommand::UploadVertex { .. }))
        // 有效测试基线必须包含顶点上传。
        .expect("test plan must upload vertices");
    // 把 SolidMesh 需要的 float2 故意替换为完整 float8 顶点。
    let FramePlanCommand::UploadVertex { data, .. } = upload else {
        // 上方筛选已经保证命令类型。
        unreachable!();
    };
    // 保存一个有限且 stride 完整、但语义错误的 float8 顶点。
    *data = FrameVertexPayload::position_uv_color_f32([0.0; 8]);
    // 验证必须在触碰 mock Adapter 前失败。
    let error = plan.validate().expect_err("vertex layout mismatch must fail");
    // 错配属于稳定计划参数错误。
    assert_eq!(error.code(), Errc::InvalidArgument);
    // 诊断必须指向 pipeline 布局错配而不是泛化资源失败。
    assert!(error.what().contains("vertex upload layout"));
}

// 验证类型化 Uniform 上传不能进入不匹配的 pipeline 常量 ABI。
#[test]
fn rejects_uniform_upload_layout_mismatch_before_adapter() {
    // 创建可验证的第一代 surface。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 复用完整有效计划作为最小变异基线。
    let mut plan = test_plan(token);
    // 取得唯一 render pass 以注入 Uniform 语义错配。
    let FramePlanStep::Pass(pass) = &mut plan.steps[0] else {
        // 测试基线漂移时立即失败。
        panic!("test plan must start with a render pass");
    };
    // 找到当前 draw 实际引用的 Uniform 上传。
    let upload = pass
        // 遍历 pass 内可变命令。
        .commands
        // 创建顺序可变迭代器。
        .iter_mut()
        // 选择唯一 Uniform 上传。
        .find(|command| matches!(command, FramePlanCommand::UploadUniform { .. }))
        // 有效测试基线必须包含 Uniform 上传。
        .expect("test plan must upload a uniform");
    // 把 SolidMesh 需要的 Mesh 常量故意替换为 Sampled 常量。
    let FramePlanCommand::UploadUniform { data, .. } = upload else {
        // 上方筛选已经保证命令类型。
        unreachable!();
    };
    // 保存字段有限但语义属于另一个 pipeline 的常量值。
    *data = FrameUniformPayload::Sampled(RhiSampledRasterParams::new(RhiViewport {
        // 保存测试宽度。
        width: 64.0,
        // 保存测试高度。
        height: 64.0,
    }));
    // 验证必须在字节编码和 Adapter 调用前失败。
    let error = plan.validate().expect_err("uniform layout mismatch must fail");
    // 错配属于稳定计划参数错误。
    assert_eq!(error.code(), Errc::InvalidArgument);
    // 诊断必须指向 Uniform pipeline 布局错配。
    assert!(error.what().contains("uniform upload layout"));
}

// 验证 submit 失败时不会进入最终 present。
#[test]
fn failed_submit_does_not_present() {
    // 创建第一代 surface。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 创建原子拥有 device 与 surface 的记录型 context。
    let mut context = recording_context(token);
    // 安排组合 context 的唯一 device submit 失败。
    context.device.fail_submit = true;
    // 执行计划并要求得到失败。
    let result = test_plan(token).execute_on_context(&mut context);
    // 验证错误保持 device lost 分类。
    let error = match result {
        // 失败分类必须在 submit 边界稳定可见。
        Err(error) => error,
        // 提交必须失败是测试的前提，不可静默通过。
        Ok(_) => panic!("submit must fail"),
    };
    assert_eq!(error.code(), Errc::GraphicsDeviceLost);
    // 验证失败 submit 没有触发 present。
    assert_eq!(context.surface.present_count, 0);
}

// 验证最终 present 的受控 surface lost 不会产生成功提交。
#[test]
fn failed_present_returns_surface_lost_without_commit() {
    // 创建第一代 surface。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 创建能够完成全部设备命令的组合记录 context。
    let mut context = recording_context(token);
    // 安排唯一 surface owner 在最终 present 返回 surface lost。
    context.surface.fail_present = true;
    // 执行完整计划并取得最终失败结果。
    let result = test_plan(token).execute_on_context(&mut context);
    // 提取受控 surface lost 错误。
    let error = match result {
        // 返回错误时保留其 typed 分类。
        Err(error) => error,
        // present 失败不能被包装为成功 FrameCommit。
        Ok(_) => panic!("present must fail"),
    };
    // 最终错误必须保持 surface lost 分类。
    assert_eq!(error.code(), Errc::GraphicsSurfaceLost);
    // 设备命令必须已经完成唯一一次 submit。
    assert_eq!(context.device.log.back(), Some(&"submit"));
    // 受控故障必须恰好发生在一次最终 present 边界。
    assert_eq!(context.surface.present_count, 1);
}

// 验证 surface resize 会拒绝旧计划并允许新代际继续提交。
#[test]
fn resize_rejects_old_plan_and_accepts_new_generation() {
    // 创建旧一代 token。
    let old_token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 创建尚未 resize 的第一代组合 context。
    let mut context = recording_context(old_token);
    // 通过 GraphicsSurface 契约执行一次受控 resize。
    let new_token = context
        .resize(RhiExtent::new(96, 80))
        .expect("recording surface resize must succeed");
    // resize 必须推进一代 surface generation。
    assert_eq!(new_token.generation, old_token.generation + 1);
    // resize 必须保存新的物理 extent。
    assert_eq!(new_token.extent, RhiExtent::new(96, 80));
    // 执行旧计划并要求得到 surface lost。
    let result = test_plan(old_token).execute_on_context(&mut context);
    // 验证错误分类稳定。
    let error = match result {
        // 旧计划必须失败，分类由统一边界负责。
        Err(error) => error,
        // 过期计划成功执行会破坏代际隔离语义。
        Ok(_) => panic!("stale plan must fail"),
    };
    assert_eq!(error.code(), Errc::GraphicsSurfaceLost);
    // 验证旧计划没有开始任何 device 工作。
    assert!(context.device.log.is_empty());
    // 验证旧计划没有进入最终 present。
    assert_eq!(context.surface.present_count, 0);
    // 使用 resize 返回的新 token 生成下一帧计划。
    let new_result = test_plan(new_token).execute_on_context(&mut context);
    // 新代际计划必须恢复正常提交。
    assert!(
        new_result.is_ok(),
        "new generation plan failed: {new_result:?}"
    );
    // 新代际计划只能触发一次最终 present。
    assert_eq!(context.surface.present_count, 1);
}

// 验证缺少 GPU 基线时在 acquire 前被拒绝。
#[test]
fn missing_gpu_baseline_is_rejected_before_acquire() {
    // 创建第一代 surface。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 创建缺少纹理上传能力的 capability 快照。
    let mut capabilities = GraphicsDeviceCapabilities::full_gpu_baseline();
    // 显式关闭一项 GPU 基线能力。
    capabilities.texture_upload = false;
    // 创建原子拥有 device 与 surface 的记录型 context。
    let mut context = recording_context(token);
    // 让唯一 device owner 报告能力缺口。
    context.device.capabilities = capabilities;
    // 执行计划并要求得到 capability 错误。
    let result = test_plan(token).execute_on_context(&mut context);
    // 验证缺口保持 NotImplemented 分类。
    let error = match result {
        // 缺口必须被拒绝，不允许降级执行。
        Err(error) => error,
        // 缺失基线的计划成功是能力门禁的失效。
        Ok(_) => panic!("missing baseline must fail"),
    };
    assert_eq!(error.code(), Errc::NotImplemented);
    // 验证缺口发生在 acquire 之前。
    assert!(context.device.log.is_empty());
    // 验证没有产生最终 present。
    assert_eq!(context.surface.present_count, 0);
}
