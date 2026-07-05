//! 布局引擎契约。

use crate::native::Rect;

/// 统一布局引擎 trait — Flex 和 Grid 的公共抽象。
pub trait LayoutEngine {
    /// 在内容区域内计算子节点位置。
    fn layout(
        &self,
        content_rect: Rect,
        children: &[crate::ui::layout::engine::LayoutChild],
    ) -> crate::ui::layout::engine::LayoutOutput;
}
