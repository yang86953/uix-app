//! BackTop 回到顶部 — 滚动超过阈值时显示返回顶部按钮。

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::ui::SnapshotFields;
use crate::ui::{EventResult, SystemEvent, WidgetTree};

component! {
    /// BackTop — 回到顶部按钮。
    pub struct BackTop {
        /// 滚动超过此高度才显示
        visibility_height: f32,
        /// 是否可见
        visible: bool,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        // 点击回到顶部
        if let SystemEvent::PointerDown { .. } = event {
            EventResult::Handled
        } else {
            EventResult::NotHandled
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if !self.visible { return; }

        let primary = ctx.tokens().color_primary();
        let bg_elevated = ctx.tokens().color_bg_elevated();

        // 圆形按钮
        let cx = frame.x + frame.w * 0.5;
        let cy = frame.y + frame.h * 0.5;
        let r = frame.w.min(frame.h) * 0.4;

        ctx.fill_circle(cx, cy, r, primary);
        ctx.fill_circle(cx, cy, r - 2.0, bg_elevated);
        // ↑ 箭头
        ctx.text_center("↑", frame, primary, 14.0);
    }
}

impl Default for BackTop {
    fn default() -> Self {
        Self::new()
    }
}

impl BackTop {
    pub fn new() -> Self {
        Self {
            visibility_height: 400.0,
            visible: false,
        }
    }

    /// 由外部 ScrollView 或 on_update 调用，传入当前 scroll_y
    pub fn update_visibility(&mut self, scroll_y: f32) {
        self.visible = scroll_y > self.visibility_height;
    }

    pub fn visibility_height(mut self, v: f32) -> Self {
        self.visibility_height = v;
        self
    }
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(40.0, 40.0)
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::BackTop {
            visibility_height: self.visibility_height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::traits::WidgetLayout;

    #[test]
    fn measure_clamps_back_top_size() {
        let measured = BackTop::new().measure(Constraints::loose(Size::new(24.0, 32.0)));

        assert_eq!(measured, Size::new(24.0, 32.0));
    }
}
