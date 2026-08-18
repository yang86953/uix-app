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
            // 所有 FramePlan 命令前必须先激活 owner context。
            "activate",
            // 激活后必须完成统一设备健康预检。
            "maintain",
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
    // 显式关闭裁剪，避免 ClearRect 测试依赖 Adapter 历史状态。
    pass.push(FramePlanCommand::SetScissor(None));
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
    // 追加与 SolidMesh 契约一致的类型化顶点上传。
    pass.push(FramePlanCommand::UploadVertex {
        // 使用稳定测试顶点 buffer。
        buffer: BufferHandle::from_raw(3),
        // 构造三个完整 position-float2 顶点。
        data: FrameVertexPayload::position_f32x2([0.0, 0.0, 1.0, 0.0, 0.0, 1.0]),
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
        PipelineBinding::for_test(PipelineHandle::from_raw(1), PipelineKind::SolidMesh),
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
            // 所有 FramePlan 命令前必须先激活 owner context。
            "activate",
            // 激活后必须完成统一设备健康预检。
            "maintain",
            "begin_pass",
            "viewport",
            "scissor",
            "clear_rect",
            "update_buffer",
            "update_buffer",
            "draw",
            "end_pass",
            "submit"
        ]
    );
    // 验证清理计划仍只触发一次最终 present。
    assert_eq!(context.surface.present_count, 1);
}

// 验证缺失类型化顶点上传时必须在进入 Adapter 前拒绝计划。
#[test]
fn rejects_draw_without_typed_vertex_upload_before_adapter() {
    // 创建可验证的第一代 surface。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 复用包含完整顶点上传的有效计划。
    let mut plan = test_plan(token);
    // 取得唯一 render pass 以移除顶点内容事实。
    let FramePlanStep::Pass(pass) = &mut plan.steps[0] else {
        // 测试基线漂移时立即失败。
        panic!("test plan must start with a render pass");
    };
    // 删除所有类型化顶点上传，保留 Uniform 与 Draw 顺序。
    pass.commands
        .retain(|command| !matches!(command, FramePlanCommand::UploadVertex { .. }));
    // 验证必须在触碰 Adapter 前返回稳定参数错误。
    let error = plan
        .validate()
        .expect_err("missing vertex upload must fail");
    // 缺失顶点内容属于稳定计划参数错误。
    assert_eq!(error.code(), Errc::InvalidArgument);
    // 诊断必须明确指出类型化顶点上传缺失。
    assert!(error.what().contains("typed vertex upload"));
}

// 验证 Draw 缺失 viewport 或 scissor 时均在 Adapter 前拒绝。
#[test]
fn rejects_draw_without_explicit_raster_state_before_adapter() {
    // 创建可验证的第一代 surface。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 复用有效计划并删除 viewport 命令。
    let mut viewport_plan = test_plan(token);
    // 取得 viewport 变异计划的唯一 render pass。
    let FramePlanStep::Pass(pass) = &mut viewport_plan.steps[0] else {
        // 测试基线漂移时立即失败。
        panic!("test plan must start with a render pass");
    };
    // 找到唯一 viewport 命令，准备证明 Draw 后状态不能反向满足。
    let viewport_index = pass
        // 遍历当前 pass 的完整命令顺序。
        .commands
        // 查找显式 viewport 的原始位置。
        .iter()
        // 只匹配 raster viewport 命令。
        .position(|command| matches!(command, FramePlanCommand::SetViewport(_)))
        // 有效测试基线必须显式建立 viewport。
        .expect("test plan must set a viewport");
    // 从 Draw 前移除 viewport，但保留命令本身供后置反例使用。
    let viewport = pass.commands.remove(viewport_index);
    // 把 viewport 放到 Draw 后，禁止全 pass 搜索伪造前序状态。
    pass.commands.push(viewport);
    // 创建独立记录 context，观察验证前是否触碰 Adapter。
    let mut viewport_context = recording_context(token);
    // 执行缺失 viewport 的计划并要求稳定失败。
    let viewport_error = viewport_plan
        .execute_on_context(&mut viewport_context)
        .expect_err("missing viewport must fail before adapter");
    // 缺失 viewport 属于稳定计划参数错误。
    assert_eq!(viewport_error.code(), Errc::InvalidArgument);
    // 诊断必须明确指出 viewport 缺失。
    assert!(viewport_error.what().contains("viewport"));
    // 验证失败发生在 activate 之前，设备日志应保持为空。
    assert!(viewport_context.device.log.is_empty());
    // 验证失败计划不得进入最终 present。
    assert_eq!(viewport_context.surface.present_count, 0);

    // 复用有效计划并删除 scissor 命令。
    let mut scissor_plan = test_plan(token);
    // 取得 scissor 变异计划的唯一 render pass。
    let FramePlanStep::Pass(pass) = &mut scissor_plan.steps[0] else {
        // 测试基线漂移时立即失败。
        panic!("test plan must start with a render pass");
    };
    // 找到唯一 scissor 命令，准备证明 Draw 后状态不能反向满足。
    let scissor_index = pass
        // 遍历当前 pass 的完整命令顺序。
        .commands
        // 查找显式 scissor 的原始位置。
        .iter()
        // Some 与 None 都属于明确的 scissor 状态命令。
        .position(|command| matches!(command, FramePlanCommand::SetScissor(_)))
        // 有效测试基线必须显式建立 scissor。
        .expect("test plan must set a scissor");
    // 从 Draw 前移除 scissor，但保留命令本身供后置反例使用。
    let scissor = pass.commands.remove(scissor_index);
    // 把 scissor 放到 Draw 后，禁止全 pass 搜索伪造前序状态。
    pass.commands.push(scissor);
    // 创建独立记录 context，观察验证前是否触碰 Adapter。
    let mut scissor_context = recording_context(token);
    // 执行缺失 scissor 的计划并要求稳定失败。
    let scissor_error = scissor_plan
        .execute_on_context(&mut scissor_context)
        .expect_err("missing scissor must fail before adapter");
    // 缺失 scissor 属于稳定计划参数错误。
    assert_eq!(scissor_error.code(), Errc::InvalidArgument);
    // 诊断必须明确指出 scissor 缺失。
    assert!(scissor_error.what().contains("scissor"));
    // 验证失败发生在 activate 之前，设备日志应保持为空。
    assert!(scissor_context.device.log.is_empty());
    // 验证失败计划不得进入最终 present。
    assert_eq!(scissor_context.surface.present_count, 0);
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
    let error = plan
        .validate()
        .expect_err("vertex layout mismatch must fail");
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
    let error = plan
        .validate()
        .expect_err("uniform layout mismatch must fail");
    // 错配属于稳定计划参数错误。
    assert_eq!(error.code(), Errc::InvalidArgument);
    // 诊断必须指向 Uniform pipeline 布局错配。
    assert!(error.what().contains("uniform upload layout"));
}

// 构造只改变 Gradient radial outer radius 的完整 FramePlan。
fn gradient_plan_with_outer_radius(token: SurfaceToken, outer_radius: f32) -> FramePlan {
    // 复用包含完整 viewport、顶点上传和 draw 的最小计划。
    let mut plan = test_plan(token);
    // 取得唯一 render pass 以替换其共享 Gradient 事实。
    let FramePlanStep::Pass(pass) = &mut plan.steps[0] else {
        // 测试基线漂移时立即失败。
        panic!("test plan must start with a render pass");
    };
    // 遍历 pass 内命令并同时替换 Uniform 与 pipeline 身份。
    for command in &mut pass.commands {
        // 只替换测试计划中唯一的 Mesh Uniform。
        if let FramePlanCommand::UploadUniform { data, .. } = command {
            // 构造共享 Gradient 的径向参数。
            *data = FrameUniformPayload::Gradient(RhiGradientRasterParams::new(
                // 使用与测试 viewport 一致的物理尺寸。
                RhiViewport {
                    // 保存测试宽度。
                    width: 64.0,
                    // 保存测试高度。
                    height: 64.0,
                },
                // 使用非退化轴对齐四角。
                [[0.0, 0.0], [20.0, 0.0], [20.0, 20.0], [0.0, 20.0]],
                // 使用有限起始颜色。
                [0.0; 4],
                // 使用有限结束颜色。
                [1.0; 4],
                // 使用当前测试的径向模式、内半径和外半径。
                [1.0, 0.1, outer_radius, 0.0],
            ));
        }
        // 只替换测试计划中唯一 Draw 的 pipeline 语义。
        if let FramePlanCommand::Draw(packet) = command {
            // 让 draw 与新上传的 Gradient ABI 成为不可拆的共享事实。
            packet.pipeline = PipelineBinding::for_test(
                // 保留测试用的 opaque pipeline handle。
                PipelineHandle::from_raw(1),
                // 选择共享 GradientRect 契约。
                PipelineKind::GradientRect,
            );
        }
    }
    // 返回已经绑定 Gradient 语义的完整计划。
    plan
}

// FramePlan 必须在 Adapter 前拒绝非正径向外半径并接受正值。
#[test]
fn gradient_frame_plan_validates_radial_outer_radius_before_adapter() {
    // 创建稳定的第一代 Surface token。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 正径向外半径必须通过完整 FramePlan 验证。
    assert!(
        gradient_plan_with_outer_radius(token, 0.5)
            .validate()
            .is_ok()
    );
    // 零外半径必须在 Adapter 前被统一拒绝。
    let zero_error = gradient_plan_with_outer_radius(token, 0.0)
        // 执行共享计划门禁。
        .validate()
        // 测试必须观察到参数错误而不是成功。
        .expect_err("zero radial outer radius must fail");
    // 零半径失败必须分类为 InvalidArgument。
    assert_eq!(zero_error.code(), Errc::InvalidArgument);
    // 负外半径必须在 Adapter 前被统一拒绝。
    let negative_error = gradient_plan_with_outer_radius(token, -0.5)
        // 执行共享计划门禁。
        .validate()
        // 测试必须观察到参数错误而不是成功。
        .expect_err("negative radial outer radius must fail");
    // 负半径失败必须分类为 InvalidArgument。
    assert_eq!(negative_error.code(), Errc::InvalidArgument);
}

// 构造由多个采样绑定依次覆盖的 coverage FramePlan。
fn coverage_plan_with_binding_kinds(
    // 冻结测试 Surface 的代际与物理范围。
    token: SurfaceToken,
    // 按命令顺序提供每次绑定的 pipeline 语义。
    binding_kinds: &[PipelineKind],
) -> FramePlan {
    // 复用具备完整 viewport、上传和 draw 的最小计划。
    let mut plan = test_plan(token);
    // 取得唯一 render pass 以替换为 coverage ABI。
    let FramePlanStep::Pass(pass) = &mut plan.steps[0] else {
        // 测试基线漂移时立即失败。
        panic!("test plan must start with a render pass");
    };
    // 遍历命令并把 SolidMesh 载荷收敛为 coverage 共享契约。
    for command in &mut pass.commands {
        // 按命令类型替换唯一相关事实。
        match command {
            // coverage 使用 position/uv/color float8 顶点。
            FramePlanCommand::UploadVertex { data, .. } => {
                // 三个完整顶点足以验证布局，不依赖真实光栅结果。
                *data = FrameVertexPayload::position_uv_color_f32([0.0; 24]);
            }
            // coverage 与普通 sampled quad 共用 viewport uniform。
            FramePlanCommand::UploadUniform { data, .. } => {
                // 构造与 pass viewport 一致的 sampled 常量。
                *data = FrameUniformPayload::Sampled(RhiSampledRasterParams::new(RhiViewport {
                    // 保存测试宽度。
                    width: 64.0,
                    // 保存测试高度。
                    height: 64.0,
                }));
            }
            // draw 必须冻结 coverage pipeline 语义。
            FramePlanCommand::Draw(packet) => {
                // 保留不透明句柄并替换为 coverage ABI。
                packet.pipeline = PipelineBinding::for_test(
                    // 使用稳定且独立的 draw pipeline 句柄。
                    PipelineHandle::from_raw(10),
                    // 选择 R8 最近点采样语义。
                    PipelineKind::GlyphCoverageQuad,
                );
            }
            // viewport 与 scissor 不参与本测试的采样语义。
            _ => {}
        }
    }
    // 定位唯一 draw，使全部测试绑定严格位于其前方。
    let draw_index = pass
        // 只读遍历当前命令顺序。
        .commands
        // 查找唯一 draw 命令。
        .iter()
        // 返回命令索引供稳定插入。
        .position(|command| matches!(command, FramePlanCommand::Draw(_)))
        // 测试基线必须继续包含 draw。
        .expect("test plan must contain a draw");
    // 按调用方顺序插入全部采样绑定。
    for (offset, kind) in binding_kinds.iter().copied().enumerate() {
        // 每个测试 pipeline 使用独立不透明句柄，避免伪造陈旧身份。
        let pipeline = PipelineBinding::for_test(
            // 从稳定基值生成互不重复的句柄。
            PipelineHandle::from_raw(20 + offset as u64),
            // 使用调用方指定的封闭采样语义。
            kind,
        );
        // 把绑定插入 draw 前，并保持调用方提供的先后顺序。
        pass.commands.insert(
            // 后续插入点随已插入命令向后移动。
            draw_index + offset,
            // 构造完整的类型化采样绑定命令。
            FramePlanCommand::BindSampledTexture(SampledTextureBinding::for_pipeline(
                // 使用稳定的非目标纹理身份。
                TextureHandle::from_raw(30),
                // 使用稳定 sampler 身份，实际描述由 Device 边界验证。
                SamplerHandle::from_raw(31),
                // 采样语义只能从本次 pipeline 身份派生。
                pipeline,
            )),
        );
    }
    // 返回已经按顺序冻结采样绑定的完整计划。
    plan
}

// FramePlan 必须只接受 draw 前最近且语义匹配的采样绑定。
#[test]
fn sampled_draw_validates_the_latest_binding_before_device() {
    // 创建稳定的第一代 Surface token。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 较旧错配绑定被后续 coverage 绑定覆盖时计划必须有效。
    let valid = coverage_plan_with_binding_kinds(
        // 复用同一 Surface 事实。
        token,
        // 最近绑定使用 coverage 语义。
        &[PipelineKind::TexturedQuad, PipelineKind::GlyphCoverageQuad],
    );
    // 完整共享门禁必须接受最近绑定匹配的计划。
    assert!(valid.validate().is_ok());
    // 最近绑定被颜色语义覆盖时必须在 Device 前失败。
    let invalid = coverage_plan_with_binding_kinds(
        // 复用同一 Surface 事实。
        token,
        // 最近绑定故意使用 premultiplied color 语义。
        &[PipelineKind::GlyphCoverageQuad, PipelineKind::TexturedQuad],
    );
    // 执行共享 FramePlan 门禁并取得稳定错误。
    let error = invalid
        // 不进入任何 RecordingDevice 方法。
        .validate()
        // 错配必须显式失败。
        .expect_err("latest sampled binding mismatch must fail");
    // 采样语义错配属于共享参数错误。
    assert_eq!(error.code(), Errc::InvalidArgument);
    // 诊断必须明确指向绑定与 pipeline contract。
    assert!(error.what().contains("sampled binding does not match"));
}

// FramePlan 必须在 Device 前拒绝离屏目标与采样纹理相同的反馈环。
#[test]
fn sampled_binding_rejects_feedback_loop_before_device() {
    // 创建稳定的测试 Surface token 以复用完整计划 fixture。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 构造绑定纹理为三十的完整 sampled plan。
    let mut invalid = coverage_plan_with_binding_kinds(token, &[PipelineKind::GlyphCoverageQuad]);
    // 把计划作用域切换为只允许显式离屏目标的 Device 事务。
    invalid.scope = FramePlanScope::Offscreen;
    // 让目标与 BindSampledTexture 使用同一个纹理身份。
    {
        // 只在本作用域内借用计划中的 render pass。
        let FramePlanStep::Pass(pass) = &mut invalid.steps[0] else {
            // 测试 fixture 漂移时立即失败。
            panic!("sampled plan must start with a render pass");
        };
        // 目标纹理三十与 fixture 绑定纹理相同，形成确定反馈环。
        pass.target = RenderTargetRef::Texture(TextureHandle::from_raw(30));
    }
    // 创建不会触碰真实图形 API 的记录 context。
    let mut context = recording_context(token);
    // 执行入口必须在 activate 前返回共享参数错误。
    let error = invalid
        .execute_offscreen_on_device(&mut context.device)
        .expect_err("feedback loop must fail before device execution");
    // 反馈环属于稳定 InvalidArgument。
    assert_eq!(error.code(), Errc::InvalidArgument);
    // 诊断必须明确指出反馈环。
    assert!(error.what().contains("feedback loop"));
    // 任何 Device 原语都不得在 FramePlan 失败后被触碰。
    assert!(context.device.log.is_empty());
    // 使用不同目标纹理的同类计划必须通过共享验证。
    {
        // 重新取得独立的短期 pass 借用，构造合法目标反例。
        let FramePlanStep::Pass(pass) = &mut invalid.steps[0] else {
            // 测试 fixture 漂移时立即失败。
            panic!("sampled plan must start with a render pass");
        };
        // 不同目标与绑定纹理不构成反馈环。
        pass.target = RenderTargetRef::Texture(TextureHandle::from_raw(31));
    }
    // 不同目标与绑定纹理不构成反馈环。
    assert!(invalid.validate().is_ok());
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

// 验证 pass 内命令失败时仍会收尾且不会进入 submit 或 present。
#[test]
fn failed_draw_ends_the_render_pass_before_returning() {
    // 创建第一代 surface。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 创建会记录全部薄 RHI 命令的组合 context。
    let mut context = recording_context(token);
    // 安排 pass 内唯一 draw 返回主错误。
    context.device.fail_draw = true;
    // 执行完整 Surface 计划并取得失败。
    let result = test_plan(token).execute_on_context(&mut context);
    // 提取执行器必须原样返回的 draw 错误。
    let error = match result {
        // 失败值必须保持原始 Adapter 错误。
        Err(error) => error,
        // draw 失败不得被 cleanup 伪装成成功提交。
        Ok(_) => panic!("draw must fail"),
    };
    // cleanup 不得覆盖主错误分类。
    assert_eq!(error.code(), Errc::PlatformError);
    // 主错误诊断必须仍包含最初失败的 draw 文本。
    assert!(error.what().contains("recording draw failed"));
    // pass 开始后必须确实到达失败 draw。
    assert!(context.device.log.contains(&"draw"));
    // 失败路径最后一条 Device 命令必须是 pass 收尾。
    assert_eq!(context.device.log.back().copied(), Some("end_pass"));
    // 共同收尾只能执行一次，禁止正常路径再次结束。
    assert_eq!(
        // 统计记录中的 pass 结束次数。
        context
            // 访问记录型 Device。
            .device
            // 借用完整命令日志。
            .log
            // 遍历全部记录项。
            .iter()
            // 只保留 pass 结束记录。
            .filter(|entry| **entry == "end_pass")
            // 计算收尾调用次数。
            .count(),
        // 失败路径必须且只能收尾一次。
        1,
    );
    // 中途失败后不得提交任何 Device 命令。
    assert!(!context.device.log.contains(&"submit"));
    // 没有成功 submit 时 Surface 不得进入 present。
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
