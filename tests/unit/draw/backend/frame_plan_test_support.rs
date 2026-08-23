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
    // 追加一个完整类型化顶点上传，覆盖真实 renderer 的 vertex 顺序。
    pass.push(FramePlanCommand::UploadVertex {
        // 使用稳定的不透明顶点 buffer。
        buffer: BufferHandle::from_raw(3),
        // 从 buffer 起始位置完整覆盖。
        // 构造三个完整 position + coverage 顶点。
        data: FrameVertexPayload::position_coverage_f32([
            0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 1.0, 1.0,
        ]),
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
    // 一次构造绑定 SolidMesh、完整 Buffer 角色与非空范围的 packet。
    let packet = DrawPacket::new(
        // 测试句柄必须与共享 pipeline kind 一起进入 FramePlan。
        PipelineBinding::for_test(PipelineHandle::from_raw(1), PipelineKind::SolidMesh),
        // 原子绑定刚刚类型化上传的顶点与 Uniform buffer。
        DrawBufferBindings::new(BufferHandle::from_raw(3), BufferHandle::from_raw(4)),
        // SolidMesh 不读取纹理，显式选择无采样角色。
        DrawSamplingBinding::none(),
        // 将动态栅格事实与 Draw 一起冻结。
        crate::platform::presentation::rhi::DrawRasterState::new(
            // 使用与目标一致的物理 viewport。
            RhiViewport {
                width: 64.0,
                height: 64.0,
            },
            // 明确选择完整 viewport。
            None,
        ),
        // 当前载荷包含三个顶点。
        DrawRange::vertices(3),
    );
    // 追加完整绑定后的 draw packet。
    pass.push(FramePlanCommand::Draw(packet));
    // 追加唯一 render pass。
    plan.push_pass(pass);
    // 返回测试计划。
    plan
}

// 在标准顶点和 Uniform 计划上构造类型化索引 Draw。
fn test_indexed_plan<const N: usize>(
    // 接收必须与 Surface 保持一致的代际。
    token: SurfaceToken,
    // 接收由 FramePlan 获取不可变所有权的 u32 索引。
    indices: [u32; N],
    // 接收 DrawRange 声明的索引数量。
    count: u32,
    // 接收 DrawRange 声明的首索引位置。
    first: u32,
) -> FramePlan {
    // 从已经具有完整顶点、Uniform 和 raster state 的基线开始。
    let mut plan = test_plan(token);
    // 取得基线的唯一 render pass 以插入索引事实。
    let FramePlanStep::Pass(pass) = &mut plan.steps[0] else {
        // 基线结构漂移时立即失败。
        panic!("test plan must start with a render pass");
    };
    // 使用独立句柄避免与顶点或 Uniform 角色重叠。
    let index_buffer = BufferHandle::from_raw(5);
    // 定位应当消费索引载荷的唯一 Draw。
    let draw_index = pass
        // 只观察当前 pass 的有序命令。
        .commands
        // 以不可变方式搜索 Draw 位置。
        .iter()
        // 返回第一个也应是唯一一个 Draw 索引。
        .position(|command| matches!(command, FramePlanCommand::Draw(_)))
        // 基线若不再含 Draw，测试 fixture 必须显式失败。
        .expect("test plan must contain a draw");
    // 在 Draw 之前插入同一 pass 拥有的类型化索引内容。
    pass.commands.insert(
        // 保持索引上传先于 Draw 生效。
        draw_index,
        // 把句柄与不可变 u32 序列绑定为一条命令。
        FramePlanCommand::UploadIndex {
            // 上传和 DrawRange 共享同一索引 Buffer 身份。
            buffer: index_buffer,
            // 通过唯一构造器冻结 u32 格式。
            data: FrameIndexPayload::uint32(indices),
        },
    );
    // 取得插入后后移一位的 Draw 命令。
    let FramePlanCommand::Draw(packet) = &mut pass.commands[draw_index + 1] else {
        // 插入不得改变基线 Draw 的相对位置。
        panic!("typed index upload must precede the draw");
    };
    // 保留完整 pipeline 与 Buffer bindings，只替换索引范围。
    *packet = packet.with_range(DrawRange::indices(
        // 索引资源与元素格式始终作为不可拆事实。
        IndexBufferBinding::new(index_buffer, IndexFormat::Uint32),
        // 保留调用方指定的有符号范围长度。
        count,
        // 保留调用方指定的首索引位置。
        first,
    ));
    // 返回可以继续变异或执行的索引计划。
    plan
}
