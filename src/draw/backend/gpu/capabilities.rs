//! 通用 GPU renderer 从 platform 薄 RHI 事实派生的私有能力投影。

// 引入 platform presentation 拥有的底层事实快照。
use crate::native::present::rhi::GraphicsCapabilities;

// 为 renderer 能力投影启用可比较、可复制的值语义。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
// 保存 graphics backend 真正消费的最小能力集合。
pub(crate) struct NativeRasterCaps {
    // 记录主颜色缓冲是否在提交间保留像素；它与 present coherency 正交。
    pub(crate) retained_framebuffer: bool,
    // 记录 retained RHI 是否具备 premultiplied Additive pipeline。
    pub(crate) rhi_additive_blend: bool,
    // 结束 renderer 私有能力投影定义。
}

// 为 renderer 能力投影提供唯一构造与基线判断。
impl NativeRasterCaps {
    // 从同一次薄 RHI 事实快照投影 renderer 真正消费的能力。
    pub(crate) const fn from_rhi_capabilities(capabilities: GraphicsCapabilities) -> Self {
        // 只复制绘制侧需要的事实，不建立第二份 adapter capability 来源。
        Self {
            // 主颜色目标的跨帧保留语义直接来自薄 RHI 快照。
            retained_framebuffer: capabilities.retained_framebuffer,
            // Additive pipeline 事实直接来自同一个薄 RHI 快照。
            rhi_additive_blend: capabilities.additive_blend,
            // 结束能力投影值。
        }
        // 结束能力投影构造。
    }

    // 判断固定 probe 验证后的 GPU-only renderer 是否具备 retained surface 基线。
    pub(crate) const fn has_gpu_only_baseline(self) -> bool {
        // 逐图元 pipeline 已由固定 probe 验证，GPU-only 只需 retained surface 事实。
        self.retained_framebuffer
        // 结束 GPU-only 基线判断。
    }
    // 结束 renderer 能力投影实现。
}

// 仅在单元测试构建中验证 graphics 到 platform 的事实投影。
#[cfg(test)]
// 组织 renderer 私有能力投影测试。
mod tests {
    // 引入被测能力投影和底层事实类型。
    use super::{GraphicsCapabilities, NativeRasterCaps};

    // 验证 renderer profile 只从 retained 薄 RHI 快照派生。
    #[test]
    // 执行 retained 与 Additive 事实投影断言。
    fn renderer_profile_projects_retained_and_additive_facts() {
        // 构造 D3D11 与 OpenGL ES 生产实现共同满足的 retained RHI 快照。
        let rhi_capabilities = GraphicsCapabilities::retained_gpu_baseline();
        // 从唯一事实来源派生 renderer 使用的窄能力投影。
        let renderer_capabilities = NativeRasterCaps::from_rhi_capabilities(rhi_capabilities);
        // 投影必须保留跨帧主颜色目标事实。
        assert!(renderer_capabilities.retained_framebuffer);
        // 投影必须保留真实的 RHI Additive 能力。
        assert!(renderer_capabilities.rhi_additive_blend);
        // retained 事实必须继续满足生产 GPU-only 绘制基线。
        assert!(renderer_capabilities.has_gpu_only_baseline());
        // 结束能力投影测试。
    }
    // 结束 renderer 能力投影测试模块。
}
