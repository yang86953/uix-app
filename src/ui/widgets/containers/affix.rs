//! Affix 固定定位组件 — 滚动超过指定偏移时将子组件固定在视口位置。
//!
//! 监听滚动位置变化，当 scroll_y > offset_top 时固定。

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::ui::SnapshotFields;
use crate::ui::{EventResult, SystemEvent, WidgetTree};

component! {
    /// Affix — 固定定位容器。
    ///
    /// 当页面滚动超过 offset_top 时，将子组件固定在视口顶部。
    /// 通过修改子组件 frame 的 y 坐标实现固定效果。
    pub struct Affix {
        /// 触发固定的滚动偏移阈值（px）
        offset_top: f32,
        /// 当前是否处于固定状态
        affixed: bool,
        /// 子组件原始 Y 位置（未固定时的自然位置）
        original_y: f32,
        /// 子组件高度（用于占位）
        child_height: f32,
        /// 当前滚动 Y
        scroll_y: f32,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    on_event => (&mut self, _event: &SystemEvent) -> EventResult {
        EventResult::NotHandled
    }

    render => (&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {}

    layout_children => (&self, _frame: Rect, _children: &[crate::ui::LayoutChild],
        _tree: &crate::ui::core::widget::WidgetTree) -> Vec<(crate::ui::ComponentId, Rect)>
    {
        Vec::new()
    }
}

impl Affix {
    pub fn new(offset_top: f32) -> Self {
        Self {
            offset_top,
            affixed: false,
            original_y: 0.0,
            child_height: 0.0,
            scroll_y: 0.0,
        }
    }

    /// 由外部每帧调用，传入当前滚动 Y，更新固定状态
    pub fn update_scroll(&mut self, scroll_y: f32) {
        self.scroll_y = scroll_y;
        self.affixed = scroll_y > self.offset_top;
    }

    /// 设置子组件原始位置和高度（由外部布局后注入）
    pub fn set_child_bounds(&mut self, y: f32, h: f32) {
        self.original_y = y;
        self.child_height = h;
    }

    pub fn is_affixed(&self) -> bool {
        self.affixed
    }
    pub fn offset_top(mut self, v: f32) -> Self {
        self.offset_top = v;
        self
    }

    /// 计算子组件应放置的位置（固定时返回视口顶部偏移，否则返回原始位置）
    pub fn child_y(&self) -> f32 {
        if self.affixed {
            self.offset_top
        } else {
            self.original_y - self.scroll_y
        }
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(0.0, if self.affixed { self.child_height } else { 0.0 })
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.offset_top = next.offset_top;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Affix {
            offset_top: self.offset_top,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::traits::WidgetLayout;

    #[test]
    fn measure_clamps_affixed_placeholder_height() {
        let mut affix = Affix::new(12.0);
        affix.set_child_bounds(0.0, 80.0);
        affix.update_scroll(20.0);

        let measured = affix.measure(Constraints::loose(Size::new(120.0, 32.0)));

        assert_eq!(measured, Size::new(0.0, 32.0));
    }
}
