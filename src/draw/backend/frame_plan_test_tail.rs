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

    // 验证 surface 代际变化会在 acquire 前拒绝旧计划。
    #[test]
    fn stale_generation_is_rejected_before_acquire() {
        // 创建旧一代 token。
        let old_token = SurfaceToken::new(1, RhiExtent::new(64, 64));
        // 创建已经重建到新一代的 surface。
        let mut surface = RecordingSurface {
            token: SurfaceToken::new(2, RhiExtent::new(64, 64)),
            target: RenderTargetHandle::from_raw(2),
            present_count: 0,
            fail_present: false,
        };
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
