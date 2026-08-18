// 构造一个包含 viewport、scissor 和 draw 的最小计划。
fn test_plan(token: SurfaceToken) -> FramePlan {
    // 使用带代际的 Surface 作用域构造常规呈现计划。
    test_plan_for_target(
        // Surface 测试计划必须显式冻结代际。
        FramePlan::new(token, crate::core::PresentDamage::Full),
        // pass 目标在 acquire 后解析为当前 image。
        RenderTargetRef::Surface,
    )
}

// 构造一个完全不依赖 SurfaceToken 的离屏测试计划。
fn test_offscreen_plan(target: TextureHandle) -> FramePlan {
    // Device-only 作用域与显式 texture pass 必须同时构造。
    test_plan_for_target(
        // 离屏计划从类型上不持有任何 Surface 生命周期或 present damage。
        FramePlan::offscreen(),
        // 把调用方给出的类型化纹理句柄固定为 pass 目标。
        RenderTargetRef::Texture(target),
    )
}

// 为已经冻结作用域的计划构造包含 viewport、scissor 和 draw 的最小 pass。
fn test_plan_for_target(mut plan: FramePlan, target: RenderTargetRef) -> FramePlan {
    // 创建带完整清理的目标 pass。
    let mut pass = RenderPassPlan::new(
        // 直接使用调用方声明的 API 无关目标。
        target,
        // 使用确定的预乘透明黑清理初始内容。
        LoadAction::Clear(RhiColor::from_premultiplied_rgba([0.0, 0.0, 0.0, 1.0])),
    );
    // 追加 viewport 设置。
    pass.push(FramePlanCommand::SetViewport(RhiViewport {
        // 固定测试物理宽度。
        width: 64.0,
        // 固定测试物理高度。
        height: 64.0,
    }));
    // 追加 scissor 设置。
    pass.push(FramePlanCommand::SetScissor(None));
    // 追加一个完整类型化顶点上传，覆盖真实 renderer 的 vertex 顺序。
    pass.push(FramePlanCommand::UploadVertex {
        // 使用稳定的不透明顶点 buffer。
        buffer: BufferHandle::from_raw(3),
        // 从 buffer 起始位置完整覆盖。
        // 构造三个完整 position-float2 顶点。
        data: FrameVertexPayload::position_f32x2([0.0, 0.0, 1.0, 0.0, 0.0, 1.0]),
    });
    // 追加与 SolidMesh 契约一致的类型化 Uniform 上传。
    pass.push(FramePlanCommand::UploadUniform {
        // 使用独立测试 Uniform buffer。
        buffer: BufferHandle::from_raw(4),
        // 构造包含相同 viewport 与有限颜色的 Mesh 常量。
        data: FrameUniformPayload::Mesh(RhiMeshRasterParams::new(
            // 保持与 pass 设置相同的物理 viewport。
            RhiViewport {
                // 保存测试宽度。
                width: 64.0,
                // 保存测试高度。
                height: 64.0,
            },
            // 使用不透明白色避免颜色域干扰。
            [1.0, 1.0, 1.0, 1.0],
        )),
    });
    // 构造绑定 SolidMesh 语义的非空三角形 packet。
    let mut packet = DrawPacket::triangles(
        // 测试句柄必须与共享 pipeline kind 一起进入 FramePlan。
        PipelineBinding::for_test(PipelineHandle::from_raw(1), PipelineKind::SolidMesh),
        // 当前载荷包含三个顶点。
        3,
    );
    // 让 draw 引用刚刚类型化上传的顶点 buffer。
    packet.vertex_buffer = BufferHandle::from_raw(3);
    // 让 draw 引用刚刚类型化上传的 Uniform buffer。
    packet.uniform_buffer = Some(BufferHandle::from_raw(4));
    // 追加完整绑定后的 draw packet。
    pass.push(FramePlanCommand::Draw(packet));
    // 追加唯一 render pass。
    plan.push_pass(pass);
    // 返回测试计划。
    plan
}
