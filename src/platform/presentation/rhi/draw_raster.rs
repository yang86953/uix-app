//! DrawPacket 独占的 viewport 与 scissor 栅格状态契约。

// 引入共享物理几何值对象。
use super::{RhiExtent, RhiScissor, RhiViewport};

// 原子保存一次 Draw 必须显式交付的动态栅格状态。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DrawRasterState {
    // 保存从物理目标左上角开始的 viewport 尺寸。
    viewport: RhiViewport,
    // 保存明确启用的左上原点 scissor；None 表示完整 viewport。
    scissor: Option<RhiScissor>,
}

// 为 Drawing、FramePlan 与 Adapter 提供完整构造和只读投影。
impl DrawRasterState {
    // 一次冻结当前 Draw 的 viewport 与显式裁剪选择。
    pub(crate) const fn new(viewport: RhiViewport, scissor: Option<RhiScissor>) -> Self {
        // 两项动态状态共同进入 packet，禁止依赖原生历史状态补齐。
        Self { viewport, scissor }
    }

    // 返回当前 Draw 的物理 viewport。
    pub(crate) const fn viewport(self) -> RhiViewport {
        // 复制共享几何值，不暴露成员改写能力。
        self.viewport
    }

    // 返回当前 Draw 明确选择的左上原点裁剪。
    pub(crate) const fn scissor(self) -> Option<RhiScissor> {
        // None 也是完整且可审计的无裁剪事实。
        self.scissor
    }

    // 判断两项状态是否属于现有 Adapter 的共同原生值域。
    pub(crate) fn is_valid(self) -> bool {
        // viewport 必须是正的有限整像素尺寸。
        self.viewport.is_valid()
            // 显式 scissor 必须具有非负起点、正尺寸和无溢出远端。
            && self
                .scissor
                // 无裁剪不需要额外矩形值域。
                .is_none_or(|scissor| scissor.native_rect().is_some())
    }

    // 判断完整动态状态是否落在当前 render target 物理范围内。
    pub(crate) fn fits_within(self, extent: RhiExtent) -> bool {
        // viewport 与显式 scissor 必须分别满足同一目标边界。
        self.viewport.fits_within(extent)
            && self
                .scissor
                // None 代表完整 viewport，不制造另一套默认矩形。
                .is_none_or(|scissor| scissor.fits_within(extent))
    }
}
