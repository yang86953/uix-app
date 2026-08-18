//! 通用 GPU renderer 从 platform 薄 RHI 事实派生的私有能力投影。

// 引入 platform presentation 拥有的底层事实快照。
use crate::native::present::rhi::GraphicsDeviceCapabilities;

// 为 renderer 能力投影启用可比较、可复制的值语义。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
// 保存 graphics backend 真正消费的最小能力集合。
pub(crate) struct NativeRasterCaps {
    // 记录 Drawing Renderer 能否用 Device 原语持有并合成跨帧主颜色目标。
    pub(crate) retained_color_target: bool,
    // 记录 retained RHI 是否具备重叠安全的纹理区域移动。
    pub(crate) rhi_texture_region_move: bool,
    // 记录 retained RHI 是否具备 premultiplied Additive pipeline。
    pub(crate) rhi_additive_blend: bool,
    // 结束 renderer 私有能力投影定义。
}

// 为 renderer 能力投影提供唯一构造与基线判断。
impl NativeRasterCaps {
    // 从同一次薄 RHI Device 快照推导 Renderer 真正消费的能力。
    pub(crate) const fn from_device_capabilities(
        // 接收只包含底层原语的 Device 能力。
        capabilities: GraphicsDeviceCapabilities,
    ) -> Self {
        // 只在 Drawing 边界组合底层事实，不要求 Adapter 知道 retained 策略。
        Self {
            // retained color target 需要可写离屏纹理、采样合成和确定复制三项原语。
            retained_color_target: capabilities.render_to_texture
                // 最终 surface composite 必须能采样主颜色纹理。
                && capabilities.sampled_textures
                // 区域修复和 backdrop 路径必须能复制颜色纹理。
                && capabilities.texture_copy,
            // 滚动复用能力必须直接来自当前 Device Adapter 的可选原语事实。
            rhi_texture_region_move: capabilities.texture_region_move,
            // Additive pipeline 事实直接来自同一个薄 RHI 快照。
            rhi_additive_blend: capabilities.additive_blend,
            // 结束能力投影值。
        }
        // 结束能力投影构造。
    }

    // 判断固定 probe 验证后的 GPU-only renderer 是否具备 retained surface 基线。
    pub(crate) const fn has_gpu_only_baseline(self) -> bool {
        // 逐图元 pipeline 已由固定 probe 验证，GPU-only 只需 Renderer retained 事实。
        self.retained_color_target
        // 结束 GPU-only 基线判断。
    }
    // 结束 renderer 能力投影实现。
}

// 仅在单元测试构建中验证 graphics 到 platform 的事实投影。
#[cfg(test)]
// 组织 renderer 私有能力投影测试。
mod tests {
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
}
