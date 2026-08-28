//! 通用 GPU Renderer 到共享基础图元 uniform ABI 的映射。

// 引入薄 RHI 拥有的基础图元值对象与物理 viewport。
use crate::platform::presentation::rhi::{
    RhiMeshRasterParams, RhiSampledRasterParams, RhiSectorRasterParams, RhiViewport,
};

// 引入父模块的唯一 renderer 类型。
use super::RhiRenderer;

// 为所有执行入口提供同一组基础图元 uniform lowering。
impl RhiRenderer {
    // 把 solid mesh 的 viewport 与颜色映射为共享 RHI 常量值对象。
    pub(super) fn mesh_uniform(viewport: RhiViewport, rgba: [f32; 4]) -> RhiMeshRasterParams {
        // 由共享值对象唯一排列 viewport、padding 与 straight-alpha 颜色。
        RhiMeshRasterParams::new(viewport, rgba)
    }

    // 把 sampled、Picture 与 coverage 共用的 viewport 映射为共享常量值对象。
    pub(super) fn sampled_uniform(viewport: RhiViewport) -> RhiSampledRasterParams {
        // 由共享值对象唯一排列 viewport 和确定性 padding。
        RhiSampledRasterParams::new(viewport)
    }

    // 把平台窗口外观折叠为最终 sampled 合成使用的圆角与缺口阴影常量。
    pub(super) fn surface_sampled_uniform(
        viewport: RhiViewport,
        corner_radius: f32,
        surface_shadow_fill: [f32; 2],
    ) -> RhiSampledRasterParams {
        RhiSampledRasterParams::with_surface_corner_fill(
            viewport,
            corner_radius,
            surface_shadow_fill[0],
            surface_shadow_fill[1],
        )
    }

    // 把轴对齐扇形事实映射为共享 RHI 常量值对象。
    pub(super) fn sector_uniform(
        // 接收当前 render target 的物理 viewport。
        viewport: RhiViewport,
        // 接收扇形的物理外接矩形。
        rect: [f32; 4],
        // 接收扇形的 straight-alpha 颜色。
        rgba: [f32; 4],
        // 接收起始角与正向扫过角。
        angles: [f32; 2],
    ) -> RhiSectorRasterParams {
        // 由共享值对象唯一排列 viewport、rect、color、angles 与 padding。
        RhiSectorRasterParams::new(viewport, rect, rgba, angles)
    }
}
