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

// 验证 Draw 栅格状态的值域、目标边界与只读投影。
#[cfg(test)]
mod tests {
    // 引入被测完整状态及共享几何值对象。
    use super::*;

    // 创建稳定的完整目标 viewport。
    const VIEWPORT: RhiViewport = RhiViewport {
        // 固定测试宽度。
        width: 20.0,
        // 固定测试高度。
        height: 10.0,
    };

    // 创建位于目标内的显式裁剪。
    const SCISSOR: RhiScissor = RhiScissor {
        // 从第二列开始。
        x: 1,
        // 从第三行开始。
        y: 2,
        // 覆盖四列。
        width: 4,
        // 覆盖五行。
        height: 5,
    };

    // 完整状态必须保持构造时冻结的两项事实。
    #[test]
    fn draw_raster_state_owns_viewport_and_scissor() {
        // 一次构造明确启用裁剪的状态。
        let state = DrawRasterState::new(VIEWPORT, Some(SCISSOR));
        // viewport 投影必须保持原值。
        assert_eq!(state.viewport(), VIEWPORT);
        // scissor 投影必须保持显式 Some。
        assert_eq!(state.scissor(), Some(SCISSOR));
        // 两项值都属于共享原生值域。
        assert!(state.is_valid());
        // 两项值都完整落在测试目标内。
        assert!(state.fits_within(RhiExtent::new(20, 10)));
    }

    // 无效或越界状态必须在任一 Adapter 前共享拒绝。
    #[test]
    fn draw_raster_state_rejects_invalid_or_outside_geometry() {
        // 小数 viewport 不能由两个 Adapter 选择不同量化规则。
        let fractional = DrawRasterState::new(
            // 构造非整像素宽度。
            RhiViewport {
                // 使用半像素宽度触发共同值域门禁。
                width: 19.5,
                // 高度保持合法。
                height: 10.0,
            },
            // 明确关闭裁剪。
            None,
        );
        // 小数 viewport 必须在目标边界检查前失败。
        assert!(!fractional.is_valid());
        // 合法值域但越过目标的 scissor 也必须失败。
        let outside = DrawRasterState::new(
            // 保持 viewport 合法。
            VIEWPORT,
            // 构造右边界越过目标的裁剪。
            Some(RhiScissor {
                // 从目标最右列开始。
                x: 19,
                // 从首行开始。
                y: 0,
                // 两列会越过目标。
                width: 2,
                // 保持高度合法。
                height: 1,
            }),
        );
        // 矩形自身仍属于共同原生值域。
        assert!(outside.is_valid());
        // 目标关系门禁必须拒绝越界组合。
        assert!(!outside.fits_within(RhiExtent::new(20, 10)));
    }
}
