    // 引入被测能力投影和底层事实类型。
    use super::{GraphicsDeviceCapabilities, NativeRasterCaps};

    // 验证 Renderer profile 只从 Device 原语推导 retained 能力。
    #[test]
    // 执行 retained 与 Additive 事实投影断言。
    fn renderer_profile_derives_retained_target_and_additive_facts() {
        // 构造 D3D11 与 OpenGL ES 生产实现共同满足的底层 Device 快照。
        let device_capabilities = GraphicsDeviceCapabilities::full_gpu_baseline();
        // 从唯一原语来源派生 Renderer 使用的窄能力投影。
        let renderer_capabilities = NativeRasterCaps::from_device_capabilities(device_capabilities);
        // Renderer 必须从离屏、采样和复制原语推导跨帧颜色目标。
        assert!(renderer_capabilities.retained_color_target);
        // 投影必须保留真实的 RHI Additive 能力。
        assert!(renderer_capabilities.rhi_additive_blend);
        // 通用基线不应替具体 Adapter 虚构可选纹理移动能力。
        assert!(!renderer_capabilities.rhi_texture_region_move);
        // retained 事实必须继续满足生产 GPU-only 绘制基线。
        assert!(renderer_capabilities.has_gpu_only_baseline());
        // 模拟生产 Adapter 明确实现共享 TextureMove 原语。
        let mut move_capable_device = device_capabilities;
        // 只开启与滚动复用相关的可选底层事实。
        move_capable_device.texture_region_move = true;
        // 投影必须把 Adapter 事实交给 Drawing 能力组合层。
        assert!(
            // 从更新后的同一 Device 快照重新派生能力。
            NativeRasterCaps::from_device_capabilities(move_capable_device).rhi_texture_region_move
        );
        // 构造缺少 render-to-texture 的不完整 Device 快照。
        let mut incomplete_device = device_capabilities;
        // 关闭离屏写入原语以验证派生能力会同步失效。
        incomplete_device.render_to_texture = false;
        // 缺少任一必要原语都不得宣称 retained color target。
        assert!(
            !NativeRasterCaps::from_device_capabilities(incomplete_device).retained_color_target
        );
        // 结束能力投影测试。
    }
    // 结束 renderer 能力投影测试模块。
