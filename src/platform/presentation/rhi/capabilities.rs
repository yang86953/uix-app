//! Device 与 Surface 正交角色各自拥有的薄 RHI 能力契约。

// 引入只属于最终呈现边界的 buffer 保留证明。
use crate::core::PresentCoherency;

// 引入 Device 颜色路径的共享事实。
use super::{RhiColorContract, UIX_COLOR_CONTRACT};

// 定义只描述资源、pipeline、pass 与 submit 原语的 Device 能力。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GraphicsDeviceCapabilities {
    // 保存颜色纹理与混合器必须共同实现的颜色路径。
    pub(crate) color_contract: RhiColorContract,
    // 记录是否支持动态 buffer 创建和更新。
    pub(crate) dynamic_buffers: bool,
    // 记录是否支持纹理上传。
    pub(crate) texture_upload: bool,
    // 记录是否支持纹理复制。
    pub(crate) texture_copy: bool,
    // 记录是否支持带重叠安全语义的纹理区域移动。
    pub(crate) texture_region_move: bool,
    // 记录是否支持 render pass 内的局部颜色清理。
    pub(crate) clear_rect: bool,
    // 记录是否支持采样纹理绑定。
    pub(crate) sampled_textures: bool,
    // 记录是否支持 render-to-texture。
    pub(crate) render_to_texture: bool,
    // 记录是否支持 scissor。
    pub(crate) scissor: bool,
    // 记录是否支持 premultiplied-alpha blend。
    pub(crate) premultiplied_alpha_blend: bool,
    // 记录是否支持 sampled Additive blend；这是可选绘制能力而非 GPU 基线。
    pub(crate) additive_blend: bool,
}

// 为 Device 能力快照提供 GPU 基线和缺口检查。
impl GraphicsDeviceCapabilities {
    // 创建文档要求的完整 GPU Device 基线能力。
    pub(crate) const fn full_gpu_baseline() -> Self {
        // 返回不依赖具体 API 或 Surface 的底层能力集合。
        Self {
            // 所有 Adapter 必须从基线继承同一颜色编码与混合值域。
            color_contract: UIX_COLOR_CONTRACT,
            // 动态 buffer 是类型化 FramePlan 上传的基础。
            dynamic_buffers: true,
            // 颜色、覆盖率和 atlas 都需要纹理上传。
            texture_upload: true,
            // retained 与 backdrop 路径需要纹理复制。
            texture_copy: true,
            // 重叠移动必须由具体 Device Adapter 显式开启。
            texture_region_move: false,
            // 局部颜色清理必须由具体 Device Adapter 显式开启。
            clear_rect: false,
            // 通用 pipeline 基线要求采样纹理。
            sampled_textures: true,
            // 离屏合成要求 render-to-texture。
            render_to_texture: true,
            // 所有 pass 使用统一左上原点 scissor。
            scissor: true,
            // UI 合成基线固定要求预乘 alpha 混合。
            premultiplied_alpha_blend: true,
            // 当前两套生产 Device 都提供 Additive pipeline。
            additive_blend: true,
        }
    }

    // 判断通用 GPU Renderer 的最小 Device 原语是否全部存在。
    pub(crate) const fn has_gpu_baseline(self) -> bool {
        // 基线缺一项就不能进入正常 GPU 渲染流程。
        self.color_contract.is_uix_contract()
            // 动态 buffer 必须存在。
            && self.dynamic_buffers
            // 纹理上传必须存在。
            && self.texture_upload
            // 纹理复制必须存在。
            && self.texture_copy
            // 采样纹理必须存在。
            && self.sampled_textures
            // 离屏目标必须存在。
            && self.render_to_texture
            // scissor 必须存在。
            && self.scissor
            // 预乘 alpha 混合必须存在。
            && self.premultiplied_alpha_blend
    }

    // 返回第一个缺失的 Device 基线能力，供 bootstrap 产生稳定诊断。
    pub(crate) const fn first_missing_gpu_baseline(self) -> Option<&'static str> {
        // 颜色路径不一致时禁止把平台私有转换带入通用 Renderer。
        if !self.color_contract.is_uix_contract() {
            // 返回稳定的颜色契约能力名称。
            return Some("color_contract");
        }
        // 检查动态 buffer 能力。
        if !self.dynamic_buffers {
            // 返回稳定的 capability 名称。
            return Some("dynamic_buffers");
        }
        // 检查纹理上传能力。
        if !self.texture_upload {
            // 返回稳定的 capability 名称。
            return Some("texture_upload");
        }
        // 检查纹理复制能力。
        if !self.texture_copy {
            // 返回稳定的 capability 名称。
            return Some("texture_copy");
        }
        // 检查采样纹理能力。
        if !self.sampled_textures {
            // 返回稳定的 capability 名称。
            return Some("sampled_textures");
        }
        // 检查 render-to-texture 能力。
        if !self.render_to_texture {
            // 返回稳定的 capability 名称。
            return Some("render_to_texture");
        }
        // 检查 scissor 能力。
        if !self.scissor {
            // 返回稳定的 capability 名称。
            return Some("scissor");
        }
        // 检查 premultiplied-alpha blend 能力。
        if !self.premultiplied_alpha_blend {
            // 返回稳定的 capability 名称。
            return Some("premultiplied_alpha_blend");
        }
        // 所有 Device 基线能力都存在。
        None
    }
}

// 定义只描述 Surface 可选原语的能力快照。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct GraphicsSurfaceCapabilities {
    // 保存该 Surface 实际承诺的跨 present 像素保留语义。
    pub(crate) present_coherency: PresentCoherency,
    // 记录 Surface 是否支持同步像素回读。
    pub(crate) readback: bool,
}

// 为 Surface 能力提供显式生产构造器。
impl GraphicsSurfaceCapabilities {
    // 构造一个同时声明呈现一致性与同步回读的 Surface 能力快照。
    pub(crate) const fn with_readback(
        // 接收实际 swapchain 或 retained presenter 提供的保留证明。
        present_coherency: PresentCoherency,
    ) -> Self {
        // 两项事实都直接来自实现对应原语的 Surface Adapter。
        Self {
            // 保留最终呈现边界的类型化一致性事实。
            present_coherency,
            // 标记同步回读原语可用。
            readback: true,
        }
    }
}
