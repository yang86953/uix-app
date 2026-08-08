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
    plan.push_move(TextureMove {
        source: TextureHandle::from_raw(11),
        destination: TextureHandle::from_raw(11),
        source_x: 0,
        source_y: 0,
        destination_x: 1,
        destination_y: 0,
        width: 2,
        height: 2,
    });
    // 创建记录型 device。
    let mut device = RecordingDevice {
        log: VecDeque::new(),
        capabilities: GraphicsCapabilities::full_gpu_baseline(),
        fail_submit: false,
    };
    // 创建记录型 surface。
    let mut surface = RecordingSurface {
        token,
        target: RenderTargetHandle::from_raw(2),
        present_count: 0,
        fail_present: false,
    };
    // 执行计划并要求最终提交成功。
    assert!(plan.execute(&mut device, &mut surface).is_ok());
    // 验证移动发生在 pass 结束之后、唯一 submit 之前。
    assert_eq!(
        device.log.into_iter().collect::<Vec<_>>(),
        vec![
            "begin_pass",
            "viewport",
            "scissor",
            "update_buffer",
            "draw",
            "end_pass",
            "move",
            "submit"
        ]
    );
    // 验证移动计划没有引入第二次 present。
    assert_eq!(surface.present_count, 1);
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
        color: RhiColor([0.0, 0.0, 0.0, 0.0]),
        scissor: RhiScissor {
            x: 2,
            y: 3,
            width: 10,
            height: 11,
        },
    });
    // 追加一个最小非空 draw packet。
    pass.push(FramePlanCommand::Draw(DrawPacket::triangles(
        PipelineHandle::from_raw(1),
        3,
    )));
    // 创建并追加唯一 surface pass。
    let mut plan = FramePlan::new(token, crate::core::PresentDamage::Full);
    // 保持局部清理位于 draw 之前。
    plan.push_pass(pass);
    // 创建记录型 device。
    let mut device = RecordingDevice {
        log: VecDeque::new(),
        capabilities: GraphicsCapabilities::full_gpu_baseline(),
        fail_submit: false,
    };
    // 创建记录型 surface。
    let mut surface = RecordingSurface {
        token,
        target: RenderTargetHandle::from_raw(2),
        present_count: 0,
        fail_present: false,
    };
    // 执行计划并要求最终提交成功。
    let result = plan.execute(&mut device, &mut surface);
    // 保留底层错误内容，便于区分计划验证和 mock adapter 失败。
    assert!(result.is_ok(), "clear rect plan failed: {result:?}");
    // 验证局部清理没有被提升到 pass 外或静默跳过。
    assert_eq!(
        device.log.into_iter().collect::<Vec<_>>(),
        vec![
            "begin_pass",
            "viewport",
            "clear_rect",
            "draw",
            "end_pass",
            "submit"
        ]
    );
    // 验证清理计划仍只触发一次最终 present。
    assert_eq!(surface.present_count, 1);
}

// 验证 submit 失败时不会进入最终 present。
#[test]
fn failed_submit_does_not_present() {
    // 创建第一代 surface。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 创建强制 submit 失败的 device。
    let mut device = RecordingDevice {
        log: VecDeque::new(),
        capabilities: GraphicsCapabilities::full_gpu_baseline(),
        fail_submit: true,
    };
    // 创建记录型 surface。
    let mut surface = RecordingSurface {
        token,
        target: RenderTargetHandle::from_raw(2),
        present_count: 0,
        fail_present: false,
    };
    // 执行计划并要求得到失败。
    let result = test_plan(token).execute(&mut device, &mut surface);
    // 验证错误保持 device lost 分类。
    let error = match result {
        // 失败分类必须在 submit 边界稳定可见。
        Err(error) => error,
        // 提交必须失败是测试的前提，不可静默通过。
        Ok(_) => panic!("submit must fail"),
    };
    assert_eq!(error.code(), Errc::GraphicsDeviceLost);
    // 验证失败 submit 没有触发 present。
    assert_eq!(surface.present_count, 0);
}

// 验证最终 present 的受控 surface lost 不会产生成功提交。
#[test]
fn failed_present_returns_surface_lost_without_commit() {
    // 创建第一代 surface。
    let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
    // 创建能够完成全部设备命令的记录型 device。
    let mut device = RecordingDevice {
        // 从空调用日志开始。
        log: VecDeque::new(),
        // 提供执行 FramePlan 所需的完整 GPU 基线。
        capabilities: GraphicsCapabilities::full_gpu_baseline(),
        // 保持 submit 成功，让测试到达最终 present。
        fail_submit: false,
    };
    // 创建只在最终 present 返回 surface lost 的记录型 surface。
    let mut surface = RecordingSurface {
        // 保持计划与 acquire 的代际一致。
        token,
        // 使用稳定的测试 surface target。
        target: RenderTargetHandle::from_raw(2),
        // 从尚未 present 的状态开始。
        present_count: 0,
        // 安排最终 present 返回受控 surface lost。
        fail_present: true,
    };
    // 执行完整计划并取得最终失败结果。
    let result = test_plan(token).execute(&mut device, &mut surface);
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
    assert_eq!(device.log.back(), Some(&"submit"));
    // 受控故障必须恰好发生在一次最终 present 边界。
    assert_eq!(surface.present_count, 1);
}

    // 验证 surface resize 会拒绝旧计划并允许新代际继续提交。
    #[test]
    fn resize_rejects_old_plan_and_accepts_new_generation() {
        // 创建旧一代 token。
        let old_token = SurfaceToken::new(1, RhiExtent::new(64, 64));
        // 创建尚未 resize 的第一代 surface。
        let mut surface = RecordingSurface {
            token: old_token,
            target: RenderTargetHandle::from_raw(2),
            present_count: 0,
            fail_present: false,
        };
        // 通过 GraphicsSurface 契约执行一次受控 resize。
        let new_token = surface
            .resize(RhiExtent::new(96, 80))
            .expect("recording surface resize must succeed");
        // resize 必须推进一代 surface generation。
        assert_eq!(new_token.generation, old_token.generation + 1);
        // resize 必须保存新的物理 extent。
        assert_eq!(new_token.extent, RhiExtent::new(96, 80));
        // 创建记录型 device。
        let mut device = RecordingDevice {
            log: VecDeque::new(),
            capabilities: GraphicsCapabilities::full_gpu_baseline(),
            fail_submit: false,
        };
        // 执行旧计划并要求得到 surface lost。
        let result = test_plan(old_token).execute(&mut device, &mut surface);
        // 验证错误分类稳定。
        let error = match result {
            // 旧计划必须失败，分类由统一边界负责。
            Err(error) => error,
            // 过期计划成功执行会破坏代际隔离语义。
            Ok(_) => panic!("stale plan must fail"),
        };
        assert_eq!(error.code(), Errc::GraphicsSurfaceLost);
        // 验证旧计划没有开始任何 device 工作。
        assert!(device.log.is_empty());
        // 验证旧计划没有进入最终 present。
        assert_eq!(surface.present_count, 0);
        // 使用 resize 返回的新 token 生成下一帧计划。
        let new_result = test_plan(new_token).execute(&mut device, &mut surface);
        // 新代际计划必须恢复正常提交。
        assert!(
            new_result.is_ok(),
            "new generation plan failed: {new_result:?}"
        );
        // 新代际计划只能触发一次最终 present。
        assert_eq!(surface.present_count, 1);
    }

    // 验证缺少 GPU 基线时在 acquire 前被拒绝。
    #[test]
    fn missing_gpu_baseline_is_rejected_before_acquire() {
        // 创建第一代 surface。
        let token = SurfaceToken::new(1, RhiExtent::new(64, 64));
        // 创建缺少纹理上传能力的 capability 快照。
        let mut capabilities = GraphicsCapabilities::full_gpu_baseline();
        // 显式关闭一项 GPU 基线能力。
        capabilities.texture_upload = false;
        // 创建报告缺口的记录型 device。
        let mut device = RecordingDevice {
            log: VecDeque::new(),
            capabilities,
            fail_submit: false,
        };
        // 创建记录型 surface。
        let mut surface = RecordingSurface {
            token,
            target: RenderTargetHandle::from_raw(2),
            present_count: 0,
            fail_present: false,
        };
        // 执行计划并要求得到 capability 错误。
        let result = test_plan(token).execute(&mut device, &mut surface);
        // 验证缺口保持 NotImplemented 分类。
        let error = match result {
            // 缺口必须被拒绝，不允许降级执行。
            Err(error) => error,
            // 缺失基线的计划成功是能力门禁的失效。
            Ok(_) => panic!("missing baseline must fail"),
        };
        assert_eq!(error.code(), Errc::NotImplemented);
        // 验证缺口发生在 acquire 之前。
        assert!(device.log.is_empty());
        // 验证没有产生最终 present。
        assert_eq!(surface.present_count, 0);
    }
