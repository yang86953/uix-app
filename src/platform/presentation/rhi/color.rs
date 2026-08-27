//! 跨图形 API 共用的颜色编码与混合值域契约。

// 引入同一 RHI 边界拥有的 surface、texture 与 capability 值。
use super::{SurfaceToken, TextureFormat};
// 引入清理输出必须复用的封闭写掩码与抖动语义。
use super::pipeline::{PipelineColorWriteMask, PipelineDitherState};

// 定义颜色纹理与 surface 中 RGB 数值的唯一编码。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum RhiRgbEncoding {
    // RGB 分量保持 UI 颜色输入的 sRGB 编码值，不由原生 API 隐式解码或编码。
    SrgbEncoded,
}

// 定义固定功能混合器执行颜色运算的唯一值域。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum RhiBlendDomain {
    // 混合直接作用于 sRGB 编码数值，保持现有 UIX 跨后端视觉公式。
    EncodedRgb,
}

// 保存所有生产 Adapter 必须共同实现的颜色处理事实。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct RhiColorContract {
    // 保存颜色纹理与 surface 的 RGB 编码。
    pub(crate) rgb_encoding: RhiRgbEncoding,
    // 保存固定功能混合器的颜色运算值域。
    pub(crate) blend_domain: RhiBlendDomain,
}

// 保存整目标与局部颜色清理必须共同实现的输出状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct RhiColorClearContract {
    // 保存清理必须写入的颜色通道集合。
    pub(crate) write_mask: PipelineColorWriteMask,
    // 保存清理量化八位目标时的抖动语义。
    pub(crate) dither: PipelineDitherState,
}

// 定义 UIX 当前唯一允许的颜色路径。
pub(crate) const UIX_COLOR_CONTRACT: RhiColorContract = RhiColorContract {
    // CPU 颜色、shader 值、颜色纹理和 surface 保持同一 sRGB 编码数值。
    rgb_encoding: RhiRgbEncoding::SrgbEncoded,
    // OpenGL 与 D3D11 都必须在编码值域执行相同混合公式。
    blend_domain: RhiBlendDomain::EncodedRgb,
};

// 定义 UIX 当前唯一允许的颜色清理输出状态。
pub(crate) const UIX_COLOR_CLEAR_CONTRACT: RhiColorClearContract = RhiColorClearContract {
    // 清理必须像 D3D11 ClearView 一样覆盖全部 RGBA 通道。
    write_mask: PipelineColorWriteMask::All,
    // 清理不得让 OpenGL ES 默认 dither 改写八位颜色最低位。
    dither: PipelineDitherState::Disabled,
};

// 为常量 capability 检查提供不依赖派生 PartialEq 的契约判定。
impl RhiColorContract {
    // 判断当前值是否精确符合 UIX 唯一允许的颜色路径。
    pub(crate) const fn is_uix_contract(self) -> bool {
        // 两个维度必须同时匹配，未来扩展任一枚举都会自动进入拒绝路径。
        matches!(self.rgb_encoding, RhiRgbEncoding::SrgbEncoded)
            // 混合域同样必须保持编码 RGB。
            && matches!(self.blend_domain, RhiBlendDomain::EncodedRgb)
    }
}

// 为 Adapter 验收提供不依赖原生 API 的清理契约判定。
impl RhiColorClearContract {
    // 判断当前值是否精确符合 UIX 唯一允许的颜色清理状态。
    pub(crate) const fn is_uix_contract(self) -> bool {
        // 清理必须完整覆盖 RGBA 通道。
        matches!(self.write_mask, PipelineColorWriteMask::All)
            // 所有 Adapter 都必须关闭颜色抖动。
            && matches!(self.dither, PipelineDitherState::Disabled)
    }
}

// 让 texture format 显式投影其颜色解释，避免 Unorm 被误当作完整颜色语义。
impl TextureFormat {
    // 返回颜色 texture 的统一契约；覆盖率 texture 不携带 RGB 颜色。
    pub(crate) const fn color_contract(self) -> Option<RhiColorContract> {
        // 只把双向颜色格式映射到 UIX 的唯一颜色路径。
        match self {
            // BGRA 只改变内存通道顺序，不改变颜色编码或混合域。
            Self::Bgra8Unorm => Some(UIX_COLOR_CONTRACT),
            // RGBA 与 BGRA 共享完全相同的颜色解释。
            Self::Rgba8Unorm => Some(UIX_COLOR_CONTRACT),
            // R8 只保存覆盖率，不能被误标为 RGB 颜色。
            Self::R8Unorm => None,
        }
    }
}

// 让每代 surface token 显式暴露其不可漂移的颜色契约。
impl SurfaceToken {
    // 返回所有 Surface Adapter 在 acquire 时共同承诺的颜色路径。
    pub(crate) const fn color_contract(self) -> RhiColorContract {
        // 当前 RHI 不允许某个平台单独改变 surface 的编码或混合域。
        UIX_COLOR_CONTRACT
    }
}
