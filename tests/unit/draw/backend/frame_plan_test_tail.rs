// 帧计划契约测试的外部载荷，由 frame_plan.rs 的 mod tests include 引入。
// 验证 submit-before-present 观察器只能取得 Surface 角色。
#[test]
fn before_present_hook_receives_only_surface_role() {
    // 创建稳定的一代 Surface。
    let token = SurfaceToken::new(3, RhiExtent::new(48, 32));
    // 创建同时拥有记录型 Device 与 Surface 的组合根。
    let mut context = recording_context(token);
    // 记录钩子是否观察到当前 Surface 代际。
    let mut observed_surface = false;
    // 执行计划并在唯一 submit 与 present 之间观察窄 Surface。
    let commit = test_plan(token).execute_on_context_with_before_present(
        // 完整事务仍由组合 context 驱动。
        &mut context,
        // 回调参数由类型系统限制为 GraphicsSurface。
        &mut |surface| {
            // 观察当前代际，不取得任何 Device 命令能力。
            observed_surface = surface.token() == token;
        },
    );
    // 观察动作不得改变最终提交成功语义。
    assert!(commit.is_ok());
    // 钩子必须在同一 Surface 上真实执行一次。
    assert!(observed_surface);
    // 最终 present 仍只能发生一次。
    assert_eq!(context.surface.present_count, 1);
}

// 验证低层命令保持顺序且只触发一次最终 present。
#[test]
fn executes_in_order_and_presents_once() {
    // 创建第一代 surface。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 创建原子拥有 device 与 surface 的记录型 context。
    let mut context = recording_context(token);
    // 执行计划并要求最终提交成功。
    let commit = test_plan(token).execute_on_context(&mut context);
    // 验证得到可消费 damage 的 commit。
    assert!(commit.is_ok());
    // 验证底层顺序以一次 submit 结束。
    assert_eq!(
        context.device.log.into_iter().collect::<Vec<_>>(),
        vec![
            // 所有 FramePlan 命令前必须先激活 owner context。
            "activate",
            // 激活后必须完成统一设备健康预检。
            "maintain",
            "begin_pass",
            "update_buffer",
            "update_buffer",
            "draw",
            "end_pass",
            "submit"
        ]
    );
    // 验证 surface 只收到一次最终 present。
    assert_eq!(context.surface.present_count, 1);
}

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
        // 构造三个完整 position + coverage 顶点。
        data: FrameVertexPayload::position_coverage_f32([
            0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 1.0, 1.0,
        ]),
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
    // 一次构造完整 pipeline、Buffer 角色与非空范围的 draw packet。
    let packet = DrawPacket::new(
        // 句柄与 SolidMesh 共享语义必须不可拆地进入计划。
        PipelineBinding::for_test(PipelineHandle::from_raw(1), PipelineKind::SolidMesh),
        // 原子绑定前序类型化上传的顶点与 Uniform buffer。
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
        // 保留三个顶点的最小非空范围。
        DrawRange::vertices(3),
    );
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

// 验证非索引 DrawRange 越过类型化顶点上传时必须在 Adapter 前拒绝。
#[test]
fn rejects_draw_range_beyond_typed_upload_before_adapter() {
    // 创建可验证的第一代 surface。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 复用包含三个顶点的有效计划作为越界基线。
    let mut invalid_plan = test_plan(token);
    // 只在作用域内借用 pass，确保执行调用前借用已经结束。
    {
        // 取得唯一 render pass 以修改其 draw range。
        let FramePlanStep::Pass(pass) = &mut invalid_plan.steps[0] else {
            // 测试基线漂移时立即失败。
            panic!("test plan must start with a render pass");
        };
        // 找到唯一 draw packet 并注入超过三个顶点的非索引范围。
        let packet = pass
            // 遍历当前 pass 内的命令。
            .commands
            // 借出可变命令以修改封闭 draw packet。
            .iter_mut()
            // 只选择 Draw 命令。
            .find_map(|command| match command {
                // 返回 draw packet 的可变借用。
                FramePlanCommand::Draw(packet) => Some(packet),
                // 其它命令不拥有 DrawRange。
                _ => None,
            })
            // 有效测试基线必须包含 draw。
            .expect("test plan must contain a draw");
        // 保留完整 pipeline 与 Buffer bindings，只替换四顶点范围。
        *packet = packet.with_range(DrawRange::vertices(4));
    }
    // 创建独立记录 context，观察验证前是否触碰 Adapter。
    let mut invalid_context = recording_context(token);
    // 执行越界计划并要求稳定失败。
    let error = invalid_plan
        // 执行入口必须在 activate 前完成范围验证。
        .execute_on_context(&mut invalid_context)
        // 越界范围必须返回错误而不是截断。
        .expect_err("draw range beyond typed upload must fail");
    // 越界范围属于稳定计划参数错误。
    assert_eq!(error.code(), Errc::InvalidArgument);
    // 诊断必须明确指出类型化上传范围越界。
    assert!(error.what().contains("exceeds typed upload"));
    // 验证失败发生在 activate 之前，设备日志应保持为空。
    assert!(invalid_context.device.log.is_empty());
    // 验证失败计划不得进入最终 present。
    assert_eq!(invalid_context.surface.present_count, 0);
    // 创建第二个记录 context，证明原始合法计划仍可执行。
    let mut valid_context = recording_context(token);
    // 合法的三顶点计划必须通过同一验证入口。
    assert!(
        test_plan(token)
            .execute_on_context(&mut valid_context)
            .is_ok()
    );
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
            // 保留完整 Buffer bindings 与范围，只替换 Gradient pipeline。
            *packet = packet.with_pipeline(PipelineBinding::for_test(
                // 保留测试用的 opaque pipeline handle。
                PipelineHandle::from_raw(1),
                // 选择共享 GradientRect 契约。
                PipelineKind::GradientRect,
            ));
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

// 构造由当前 DrawPacket 自身拥有条件采样资源的 coverage FramePlan。
fn coverage_plan_with_sampling_kind(
    // 冻结测试 Surface 的代际与物理范围。
    token: SurfaceToken,
    // 提供完整绑定使用的 pipeline 语义，None 用于验证缺失角色。
    binding_kind: Option<PipelineKind>,
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
            // draw 必须同时冻结 coverage pipeline 与条件采样资源。
            FramePlanCommand::Draw(packet) => {
                // 创建当前 Draw 唯一的 coverage pipeline 身份。
                let pipeline = PipelineBinding::for_test(
                    // 使用稳定且独立的 draw pipeline 句柄。
                    PipelineHandle::from_raw(10),
                    // 选择 R8 最近点采样语义。
                    PipelineKind::GlyphCoverageQuad,
                );
                // 按测试输入构造完整采样资源或明确缺失角色。
                let sampling = match binding_kind {
                    // 有绑定时从指定 pipeline 语义派生不可拆资源事实。
                    Some(kind) => DrawSamplingBinding::sampled(
                        // 一次绑定纹理、sampler 与采样语义。
                        SampledTextureBinding::for_pipeline(
                            // 使用稳定的非目标纹理身份。
                            TextureHandle::from_raw(30),
                            // 使用稳定 sampler 身份。
                            SamplerHandle::from_raw(31),
                            // 使用独立身份冻结调用方指定的采样语义。
                            PipelineBinding::for_test(PipelineHandle::from_raw(20), kind),
                        ),
                    ),
                    // 缺失分支显式构造无采样角色供共享门禁拒绝。
                    None => DrawSamplingBinding::none(),
                };
                // 保留 Buffer 与范围，只替换完整 pipeline 和采样事实。
                *packet = packet.with_pipeline(pipeline).with_sampling(sampling);
            }
            // viewport 与 scissor 不参与本测试的采样语义。
            _ => {}
        }
    }
    // 返回采样资源已经原子收归唯一 DrawPacket 的完整计划。
    plan
}

// FramePlan 必须只接受当前 DrawPacket 自身完整且语义匹配的采样绑定。
#[test]
fn sampled_draw_validates_packet_owned_binding_before_device() {
    // 创建稳定的第一代 Surface token。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 当前 packet 拥有 coverage 绑定时计划必须有效。
    let valid = coverage_plan_with_sampling_kind(
        // 复用同一 Surface 事实。
        token,
        // packet 绑定使用 coverage 语义。
        Some(PipelineKind::GlyphCoverageQuad),
    );
    // 完整共享门禁必须接受最近绑定匹配的计划。
    assert!(valid.validate().is_ok());
    // 当前 packet 携带颜色采样语义时必须在 Device 前失败。
    let invalid = coverage_plan_with_sampling_kind(
        // 复用同一 Surface 事实。
        token,
        // packet 绑定故意使用 premultiplied color 语义。
        Some(PipelineKind::TexturedQuad),
    );
    // 执行共享 FramePlan 门禁并取得稳定错误。
    let error = invalid
        // 不进入任何 RecordingDevice 方法。
        .validate()
        // 错配必须显式失败。
        .expect_err("packet sampled binding mismatch must fail");
    // 采样语义错配属于共享参数错误。
    assert_eq!(error.code(), Errc::InvalidArgument);
    // 诊断必须明确指向绑定与 pipeline contract。
    assert!(error.what().contains("sampling binding does not match"));
    // 采样 pipeline 明确缺失绑定时也必须在 Device 前失败。
    let missing = coverage_plan_with_sampling_kind(token, None);
    // 条件角色不完整不能由任一 Adapter 的历史状态补齐。
    assert!(missing.validate().is_err());
}

// FramePlan 必须在 Device 前拒绝离屏目标与采样纹理相同的反馈环。
#[test]
fn sampled_binding_rejects_feedback_loop_before_device() {
    // 创建稳定的测试 Surface token 以复用完整计划 fixture。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 构造绑定纹理为三十的完整 sampled plan。
    let mut invalid =
        coverage_plan_with_sampling_kind(token, Some(PipelineKind::GlyphCoverageQuad));
    // 把计划作用域切换为只允许显式离屏目标的 Device 事务。
    invalid.scope = FramePlanScope::Offscreen;
    // 让目标与当前 DrawPacket 的采样纹理使用同一个身份。
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

// sampled 真实资源预检失败必须发生在 Device activate 前且不留下日志。
#[test]
fn sampled_resource_preflight_rejects_before_device() {
    // 创建稳定的第一代 Surface token。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 构造完整的 coverage sampled 计划。
    let plan = coverage_plan_with_sampling_kind(token, Some(PipelineKind::GlyphCoverageQuad));
    // 注入 sampled 资源预检失败。
    let mut rejected = recording_context(token);
    // 只影响 shared sampled 资源预检，不改变其它 Device 行为。
    rejected.device.fail_sampled_preflight = true;
    // 同时安排 Surface acquire 环境失败以验证错误优先级。
    rejected.surface.fail_acquire = true;
    // 失败必须发生在任何 Device 原语之前。
    let error = plan
        // 通过普通 Surface 入口验证 acquire 前的资源边界。
        .execute_on_context(&mut rejected)
        // sampled 资源预检必须在 activate 前失败。
        .expect_err("sampled resource preflight must fail before device activation");
    // 失败分类必须是共享参数错误。
    assert_eq!(error.code(), Errc::InvalidArgument);
    // preflight 失败不得留下 activate、pass 或 submit 日志。
    assert!(rejected.device.log.is_empty());
    // preflight 失败不得触发 present。
    assert_eq!(rejected.surface.present_count, 0);
    // Surface sampled 资源预检失败必须早于 acquire。
    assert_eq!(rejected.surface.acquire_count, 0);
    // 关闭失败注入后，同一合法计划必须通过预检并完成执行。
    let mut accepted = recording_context(token);
    // 合法 sampled binding 由测试 Device 预检放行。
    assert!(plan.execute_on_context(&mut accepted).is_ok());
    // 合法资源计划必须只 acquire 一次。
    assert_eq!(accepted.surface.acquire_count, 1);
    // 安排只有 Surface acquire 失败的环境故障。
    let mut acquire_failed = recording_context(token);
    // 保持资源预检成功，只注入 acquire 故障。
    acquire_failed.surface.fail_acquire = true;
    // 合法资源遇到 acquire 故障必须返回 Surface 生命周期错误。
    let acquire_error = plan
        // 执行入口必须保留 acquire 的环境错误分类。
        .execute_on_context(&mut acquire_failed)
        // acquire 故障必须被观察到。
        .expect_err("acquire failure must remain a surface error");
    // 环境故障分类必须是 GraphicsSurfaceLost。
    assert_eq!(acquire_error.code(), Errc::GraphicsSurfaceLost);
    // acquire 故障必须记录一次 acquire 尝试。
    assert_eq!(acquire_failed.surface.acquire_count, 1);
    // acquire 失败不得进入任何 Device 原生命令。
    assert!(acquire_failed.device.log.is_empty());
    // acquire 失败不得进入最终 present。
    assert_eq!(acquire_failed.surface.present_count, 0);
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
    // capability 缺口不得取得 Surface image。
    assert_eq!(context.surface.acquire_count, 0);
    // 验证没有产生最终 present。
    assert_eq!(context.surface.present_count, 0);
}
