    // 引入当前 canvas、能力表和 pending scroll 枚举。
    use super::super::canvas::NativeGpuCanvas2D;
    use super::super::pending::PendingNativeOp;
    use crate::core::Rect;
    use crate::draw::geometry::path::PathBuilder;
    // 引入 Additive 分段与圆角矩形测试需要的公开几何类型。
    use crate::draw::Canvas2D;
    use crate::draw::Color;
    use crate::draw::geometry::types::{BlendMode, Radius};
    // 引入 graphics backend 私有的 renderer 能力投影。
    use crate::draw::backend::gpu::NativeRasterCaps;

    // 正常整数 scroll 不应在 Canvas2D 入口被降级为 deferred error。
    #[test]
    fn records_native_scroll_boundary() {
        // 使用 GPU-only canvas，避免测试意外创建 CPU soft surface。
        let mut canvas = NativeGpuCanvas2D::new_gpu_only(64, 48, NativeRasterCaps::default());
        // 记录 source = viewport + delta、destination = viewport 的 scroll。
        canvas.scroll_region(Rect::new(8.0, 6.0, 32.0, 24.0), 3.0, -2.0);
        // 验证队列保留了目标相关操作，而不是即时拒绝。
        assert!(matches!(
            canvas.pending_native.as_slice(),
            [PendingNativeOp::ScrollCopy(scroll)]
                if scroll.viewport == Rect::new(8.0, 6.0, 32.0, 24.0)
                    && scroll.dx == 3
                    && scroll.dy == -2
        ));
        // 正常记录不应产生延迟错误。
        assert!(canvas.take_deferred_error().is_none());
    }

    // 非有限几何必须保留 typed failure，不能把 NaN 转成可执行 copy。
    #[test]
    fn rejects_invalid_native_scroll_geometry() {
        // 使用 GPU-only canvas 只验证入口审计，不触发任何 adapter 调用。
        let mut canvas = NativeGpuCanvas2D::new_gpu_only(16, 16, NativeRasterCaps::default());
        // 无穷 viewport 被延迟记录为明确的 NotImplemented 错误。
        canvas.scroll_region(Rect::new(f32::INFINITY, 0.0, 4.0, 4.0), 1.0, 0.0);
        // 验证没有生成可能污染 retained target 的 pending operation。
        assert!(canvas.pending_native.is_empty());
        // 验证错误仍保留在最终 present 边界消费。
        let Some(error) = canvas.take_deferred_error() else {
            // 非法 scroll 必须留下可消费的错误。
            panic!("invalid scroll must record an error");
        };
        assert_eq!(error.code(), crate::core::Errc::NotImplemented);
    }

    // hybrid canvas 在具备 retained RHI 时应把椭圆保留为 GPU native mesh。
    #[test]
    fn hybrid_ellipse_uses_shared_mesh_lowering() {
        // 只打开固定 probe 依赖的 retained surface 事实，保持测试边界最小。
        let caps = NativeRasterCaps {
            // retained surface 表示共享 mesh pipeline 已在构造前通过 probe。
            retained_color_target: true,
            ..NativeRasterCaps::default()
        };
        // 使用 hybrid canvas 验证非 GPU-only 入口也能复用相同 lowering。
        let mut canvas = NativeGpuCanvas2D::new(64, 48, caps);
        // 记录一个有限的椭圆填充操作。
        canvas.fill_ellipse(Rect::new(8.0, 6.0, 24.0, 18.0), Color::red());
        // 椭圆应进入 solid mesh，且不分配 soft staging。
        assert!(matches!(
            canvas.pending_native.as_slice(),
            [PendingNativeOp::SolidMesh(_)]
        ));
        // 没有发生 CPU fallback allocation。
        assert!(canvas.soft_fallback.is_none());
    }

    // 生产 RHI capability 应让 Additive 圆角矩形绕过 CPU staging。
    #[test]
    fn additive_rounded_rect_uses_native_shape_when_capability_is_explicit() {
        // 启用当前纵切需要的 retained 与 Additive RHI 事实能力。
        let caps = NativeRasterCaps {
            // retained surface 表示固定 shape pipeline 已通过 probe。
            retained_color_target: true,
            // 声明 retained RHI 可以执行 Additive pipeline。
            rhi_additive_blend: true,
            // 其余能力保持关闭，避免测试依赖无关图元。
            ..NativeRasterCaps::default()
        };
        // 使用 hybrid canvas，确保若能力分流错误就会真实分配 soft staging。
        let mut canvas = NativeGpuCanvas2D::new(32, 24, caps);
        // 切换到 destination-dependent Additive 语义。
        canvas.set_blend_mode(BlendMode::Additive);
        // 记录一个轴对齐圆角矩形。
        canvas.fill_rect(
            // 使用有限正矩形覆盖 shape SDF 入队。
            Rect::new(4.0, 3.0, 12.0, 8.0),
            // 使用不透明红色验证正常颜色载荷。
            Color::red(),
            // 非零圆角确保同一路径覆盖 rounded shape。
            Some(Radius::uniform(2.0)),
        );
        // pending queue 必须显式保留 Additive 事实。
        assert!(matches!(
            canvas.pending_native.as_slice(),
            [PendingNativeOp::SolidRect(rect)] if rect.additive
        ));
        // 直达 RHI shape 时不得创建 CPU surface。
        assert!(canvas.soft_fallback.is_none());
        // native 内容不能同时伪装成 soft segment。
        assert!(!canvas.soft_has_content);
    }

    // 横竖线与斜线必须共用解析覆盖率原语，避免方向相关的硬边锯齿。
    #[test]
    fn axis_aligned_and_diagonal_lines_share_analytic_pipeline() {
        // retained surface 表示共享 LineSegment pipeline 已通过启动探针。
        let caps = NativeRasterCaps {
            retained_color_target: true,
            ..NativeRasterCaps::default()
        };
        let mut canvas = NativeGpuCanvas2D::new_gpu_only(64, 48, caps);
        // 分别记录水平线、垂直线与对角线。
        canvas.draw_line(2.0, 4.0, 30.0, 4.0, Color::red(), 1.0);
        canvas.draw_line(8.0, 6.0, 8.0, 34.0, Color::green(), 1.5);
        canvas.draw_line(12.0, 8.0, 36.0, 30.0, Color::blue(), 2.0);
        // 三种方向都必须保留为同一种解析线段操作。
        assert_eq!(canvas.pending_native.len(), 3);
        assert!(
            canvas
                .pending_native
                .iter()
                .all(|op| matches!(op, PendingNativeOp::Line(_)))
        );
        assert!(canvas.take_deferred_error().is_none());
    }

    // 未声明 RHI Additive 的 adapter 必须保留既有等价 soft fallback。
    #[test]
    fn additive_rect_without_rhi_capability_stays_in_soft_segment() {
        // 仅打开 retained surface，刻意不声明 Additive RHI 能力。
        let caps = NativeRasterCaps {
            // 证明分流只受可选 blend 能力控制，而不是缺少 retained surface。
            retained_color_target: true,
            // 其余能力包括 rhi_additive_blend 保持默认 false。
            ..NativeRasterCaps::default()
        };
        // 使用允许 soft fallback 的 hybrid canvas。
        let mut canvas = NativeGpuCanvas2D::new(20, 12, caps);
        // 请求 Additive 矩形。
        canvas.set_blend_mode(BlendMode::Additive);
        // 绘制有限直角矩形。
        canvas.fill_rect(Rect::new(2.0, 2.0, 6.0, 4.0), Color::green(), None);
        // 不能把没有 RHI 事实支撑的操作放入 native queue。
        assert!(canvas.pending_native.is_empty());
        // 等价 CPU staging 必须存在。
        assert!(canvas.soft_fallback.is_some());
        // 快照应只包含一个 Additive soft segment。
        let segments = canvas.packed_soft_segments();
        // 单次绘制不能产生额外段。
        assert_eq!(segments.len(), 1);
        // 段的目标 blend 必须保持 Additive。
        assert!(segments[0].additive);
    }

    // hybrid path clip 应建立 soft mask，并让随后绘制受 mask 约束。
    #[test]
    fn hybrid_path_clip_uses_shared_soft_mask() {
        // 使用普通 hybrid canvas，验证路径裁剪不再被直接拒绝。
        let mut canvas = NativeGpuCanvas2D::new(32, 32, NativeRasterCaps::default());
        // 构造一个左上三角形路径。
        let mut builder = PathBuilder::new();
        builder
            .move_to(1.0, 1.0)
            .line_to(20.0, 1.0)
            .line_to(1.0, 20.0)
            .close();
        let path = builder.build();
        // 入栈后应分配共享 soft renderer，而不是产生 deferred error。
        canvas.push_clip_path(&path);
        assert!(canvas.soft_fallback.is_some());
        assert!(canvas.take_deferred_error().is_none());
        // 绘制整块矩形，结果只应出现在三角形内部。
        canvas.fill_rect(Rect::new(0.0, 0.0, 32.0, 32.0), Color::red(), None);
        let pixels = canvas.pixels();
        assert!(pixels[2 * 32 + 2] != 0);
        assert_eq!(pixels[24 * 32 + 24], 0);
        // 弹出路径裁剪后，后续绘制应恢复矩形 clip 的完整范围。
        canvas.pop_clip();
        canvas.fill_rect(Rect::new(24.0, 24.0, 4.0, 4.0), Color::blue(), None);
        assert!(canvas.pixels()[25 * 32 + 25] != 0);
    }

    // 连续 soft 操作必须按 SrcOver/Additive 切换封口并保持 painter order。
    #[test]
    fn soft_blend_changes_seal_ordered_segments() {
        // 默认能力强制三个矩形都进入共享 soft renderer。
        let mut canvas = NativeGpuCanvas2D::new(16, 8, NativeRasterCaps::default());
        // 首段使用默认 SrcOver 等价语义。
        canvas.fill_rect(Rect::new(1.0, 1.0, 2.0, 2.0), Color::red(), None);
        // 第二段切换到 destination-dependent Additive。
        canvas.set_blend_mode(BlendMode::Additive);
        // 非重叠像素让每段都保留独立、可检查的紧密 tile。
        canvas.fill_rect(Rect::new(5.0, 1.0, 2.0, 2.0), Color::green(), None);
        // 第三段切回显式 SrcOver。
        canvas.set_blend_mode(BlendMode::SrcOver);
        // 触发 Additive 段封口并建立最后一个当前段。
        canvas.fill_rect(Rect::new(9.0, 1.0, 2.0, 2.0), Color::blue(), None);
        // 只读快照必须同时包含两个已封口段和当前段。
        let segments = canvas.packed_soft_segments();
        // blend 切换产生且只产生三个连续段。
        assert_eq!(segments.len(), 3);
        // 第一段使用普通 premultiplied SrcOver pipeline。
        assert!(!segments[0].additive);
        // 中间段保留 Additive pipeline 事实。
        assert!(segments[1].additive);
        // 最后一段恢复 SrcOver pipeline。
        assert!(!segments[2].additive);
        // 每个 2x2 紧密 tile 都只携带四个可见像素。
        assert!(segments.iter().all(|segment| segment.pixels.len() == 4));
        // 三段目标横坐标必须保持原始绘制顺序。
        assert_eq!(
            segments
                .iter()
                .map(|segment| segment.tile.dst_x)
                .collect::<Vec<_>>(),
            vec![1, 5, 9]
        );
    }

    // 成功提交必须消费全部 soft 分段，后续操作不能重复上传旧像素。
    #[test]
    fn soft_segment_commit_clears_staging_without_changing_public_blend() {
        // 默认能力让测试只观察 soft staging 生命周期。
        let mut canvas = NativeGpuCanvas2D::new(12, 8, NativeRasterCaps::default());
        // 建立一个普通 soft 段。
        canvas.fill_rect(Rect::new(1.0, 1.0, 2.0, 2.0), Color::red(), None);
        // 切换并建立一个 Additive 当前段。
        canvas.set_blend_mode(BlendMode::Additive);
        // 写入第二段以形成可消费的两段队列。
        canvas.fill_rect(Rect::new(5.0, 1.0, 2.0, 2.0), Color::green(), None);
        // 提交前必须能观察到两个有序段。
        assert_eq!(canvas.packed_soft_segments().len(), 2);
        // 模拟 RHI 全部提交成功后的统一消费边界。
        canvas.commit_presented_frame();
        // 旧段不能在下一次提交快照中再次出现。
        assert!(canvas.packed_soft_segments().is_empty());
        // 全局和当前 soft 内容标记都必须归零。
        assert!(!canvas.soft_has_content && !canvas.soft_current_has_content);
        // 已封口队列必须释放共享像素载荷。
        assert!(canvas.pending_soft_segments.is_empty());
        // 内部分段类别等待下一次真实绘制重新建立。
        assert!(canvas.soft_segment_blend.is_none());
        // Canvas 的公开 blend 状态仍由调用方控制，不在内部提交时篡改。
        assert_eq!(canvas.current_blend_mode(), BlendMode::Additive);
        // 下一次绘制应建立一个全新的 Additive 段。
        canvas.fill_rect(Rect::new(8.0, 1.0, 2.0, 2.0), Color::blue(), None);
        // 新快照只包含提交后的新段。
        let segments = canvas.packed_soft_segments();
        // 不得重新带出此前的两个段。
        assert_eq!(segments.len(), 1);
        // 新段继承仍然有效的公开 Additive 状态。
        assert!(segments[0].additive);
        // 新段目标位置必须来自提交后的绘制。
        assert_eq!(segments[0].tile.dst_x, 8);
    }
