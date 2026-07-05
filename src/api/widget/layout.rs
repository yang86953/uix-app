//! # 布局引擎契约
//!
//! 统一布局引擎 trait 与布局类型 re-export。

use crate::platform::Rect;

/// 统一布局引擎 trait — Flex 和 Grid 的公共抽象。
pub trait LayoutEngine {
    /// 在内容区域内计算子节点位置。
    fn layout(
        &self,
        content_rect: Rect,
        children: &[crate::widget::layout::engine::LayoutChild],
    ) -> crate::widget::layout::engine::LayoutOutput;
}

// ── 布局类型 re-export ──
pub use crate::widget::layout::engine::{
    child_from_tree, BoxModel, FlexLayout, GridLayout, LayoutChild, LayoutOutput,
};
pub use crate::widget::layout::{AlignItems, FlexDirection, GridTrack, JustifyContent};
