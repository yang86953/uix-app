    // 引入被测规划函数。
    use super::plan_surface_composite;
    // 引入 damage、quad 和 RHI 测试值。
    use crate::core::PresentDamage;
    // 引入 sampled quad。
    use crate::draw::backend::rhi_renderer::RhiSampledQuad;
    // 引入 RHI 句柄、load action 与 extent。
    use crate::platform::presentation::rhi::{LoadAction, RhiExtent, TextureHandle};

    // 构造覆盖整个测试 surface 的 sampled quad。
    fn full_quad() -> RhiSampledQuad {
        // 返回稳定且有效的 sampled ABI。
        RhiSampledQuad {
            // 从左边界开始。
            x: 0.0,
            // 从上边界开始。
            y: 0.0,
            // 覆盖 100 像素宽度。
            w: 100.0,
            // 覆盖 80 像素高度。
            h: 80.0,
            // 使用轴对齐四角。
            corners: [[0.0, 0.0], [100.0, 0.0], [100.0, 80.0], [0.0, 80.0]],
            // 不改变 retained texture 颜色。
            rgba: [1.0; 4],
            // 使用 SrcOver。
            additive: false,
            // 提供非零测试纹理句柄。
            texture: TextureHandle::from_raw(7),
            // 采样完整横向 UV 起点。
            u0: 0.0,
            // 采样完整纵向 UV 起点。
            v0: 0.0,
            // 采样完整横向 UV 终点。
            u1: 1.0,
            // 采样完整纵向 UV 终点。
            v1: 1.0,
            // 基础规划测试不启用窗口圆角。
            surface_corner_radius: 0.0,
            // 初始 quad 不携带裁剪。
            scissor: None,
        }
    }

    // 验证 partial damage 只产生对应 scissor 且不执行全幅 clear。
    #[test]
    fn partial_damage_draws_and_presents_the_same_rectangles() {
        // 准备两个物理 damage rect。
        let damage = PresentDamage::Partial(vec![(2, 3, 10, 20), (40, 30, 15, 12)]);
        // 规划 100×80 surface 的最终合成。
        let plan = plan_surface_composite(damage.clone(), RhiExtent::new(100, 80), full_quad());
        // 最终 present damage 必须原样保留。
        assert_eq!(plan.damage, damage);
        // partial 路径必须保留 damage 外像素。
        assert!(matches!(plan.load, LoadAction::Load));
        // 每个 damage rect 对应一个 sampled draw。
        assert_eq!(plan.quads.len(), 2);
        // 第一个 scissor 与第一个 present rect 完全一致。
        assert_eq!(
            plan.quads[0]
                .scissor
                .map(|rect| (rect.x, rect.y, rect.width, rect.height)),
            Some((2, 3, 10, 20))
        );
        // 第二个 scissor 与第二个 present rect 完全一致。
        assert_eq!(
            plan.quads[1]
                .scissor
                .map(|rect| (rect.x, rect.y, rect.width, rect.height)),
            Some((40, 30, 15, 12))
        );
    }

    // 验证越界 partial 同时降级为完整绘制和完整提交。
    #[test]
    fn invalid_partial_damage_falls_back_to_full_draw_and_present() {
        // 构造越过右边界的 damage。
        let damage = PresentDamage::Partial(vec![(90, 0, 20, 10)]);
        // 规划最终合成。
        let plan = plan_surface_composite(damage, RhiExtent::new(100, 80), full_quad());
        // native present 必须改为 Full。
        assert_eq!(plan.damage, PresentDamage::Full);
        // 完整路径允许执行透明全幅 clear。
        assert!(matches!(plan.load, LoadAction::Clear(_)));
        // 完整路径只需一个 sampled quad。
        assert_eq!(plan.quads.len(), 1);
        // 完整 quad 不得保留局部 scissor。
        assert!(plan.quads[0].scissor.is_none());
    }
