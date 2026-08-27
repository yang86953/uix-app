//! 通用 GPU renderer 从 platform 薄 RHI 事实派生的私有能力投影。

// 引入 platform presentation 拥有的底层事实快照。
use crate::platform::presentation::rhi::GraphicsDeviceCapabilities;

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
