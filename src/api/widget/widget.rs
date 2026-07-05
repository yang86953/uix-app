//! # Widget 组件契约
//!
//! 定义 widget 能力 trait 与核心数据类型 re-export。

use std::any::Any;
use crate::render::painting::PaintContext as RenderContext;
use crate::render::spatial::{Ray3D, SpatialContext};
use crate::render::traits::GraphicsEngine;
use crate::platform::{Rect, Size};

// ── 核心数据类型 re-export ──
pub use crate::widget::children::WidgetChildren;
pub use crate::widget::context::WidgetContext;
pub use crate::widget::widget::{
    BoxedWidget, EventResult, WidgetCore, WidgetEvent, WidgetId, WidgetNode, WidgetTree,
};

/// 能力位标记。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WidgetCapabilities(u8);

impl WidgetCapabilities {
    pub const LAYOUT: u8 = 0b0001;
    pub const RENDER: u8 = 0b0010;
    pub const EVENT: u8 = 0b0100;
    pub const LIFECYCLE: u8 = 0b1000;

    pub const fn new() -> Self {
        Self(0)
    }
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }
    pub fn insert(&mut self, cap: u8) {
        self.0 |= cap;
    }
    pub fn contains(&self, cap: u8) -> bool {
        self.0 & cap != 0
    }
    pub fn bits(&self) -> u8 {
        self.0
    }
}

/// 组件核心标识 — 所有 widget 必须实现。
pub trait WidgetComponent: 'static {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// 返回此 widget 实现了哪些能力。
    fn capabilities(&self) -> WidgetCapabilities;
    /// 返回当前 widget 的内部可见性。
    fn visible(&self) -> bool {
        true
    }
    fn build(&self) -> Vec<Box<dyn WidgetComponent>> {
        vec![]
    }
    /// 默认 Tab 键导航索引（> 0 表示组件默认可通过 Tab 聚焦）。
    /// 应用层可通过 `WidgetNode::tab_index()` 覆盖。
    fn tab_index(&self) -> i32 {
        0
    }

    // ── 可选能力上转型（宏自动生成） ──
    fn as_layout(&self) -> Option<&dyn WidgetLayout> {
        None
    }
    fn as_render(&self) -> Option<&dyn WidgetRender> {
        None
    }
    fn as_render_mut(&mut self) -> Option<&mut dyn WidgetRender> {
        None
    }
    fn as_event(&self) -> Option<&dyn WidgetEventHandler> {
        None
    }
    fn as_event_mut(&mut self) -> Option<&mut dyn WidgetEventHandler> {
        None
    }
    fn as_lifecycle(&self) -> Option<&dyn WidgetLifecycle> {
        None
    }
    fn as_lifecycle_mut(&mut self) -> Option<&mut dyn WidgetLifecycle> {
        None
    }
}

/// 布局行为：尺寸、弹性、子节点排列。
pub trait WidgetLayout: WidgetComponent {
    fn preferred_size(&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::zero()
    }
    fn flex_grow(&self) -> f32 {
        0.0
    }
    fn flex_shrink(&self) -> f32 {
        0.0
    }
    fn layout_children(
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
    ) -> Vec<(WidgetId, Rect)> {
        let _ = (frame, children, tree);
        Vec::new()
    }
}

/// 渲染行为：绘制、覆盖层、脏区域。
pub trait WidgetRender: WidgetComponent {
    fn render(&self, frame: Rect, ctx: &mut RenderContext, tree: &WidgetTree);
    fn draw_margin(&self) -> f32 {
        0.0
    }
    fn dirty_rect(&self, frame: Rect) -> Rect {
        let m = self.draw_margin();
        if m > 0.0 {
            Rect::new(
                frame.x - m,
                frame.y - m,
                frame.w + m * 2.0,
                frame.h + m * 2.0,
            )
        } else {
            frame
        }
    }
    fn is_repaint_boundary(&self) -> bool {
        false
    }
    fn children_clip(&self, _frame: Rect) -> Option<Rect> {
        None
    }
}

/// 事件行为：输入事件处理、持续更新、滚动偏移、命中测试。
pub trait WidgetEventHandler: WidgetComponent {
    fn on_event(&mut self, _event: &WidgetEvent) -> EventResult {
        EventResult::NotHandled
    }
    fn needs_continuous_update(&self) -> bool {
        false
    }
    fn scroll_delta(&self, _frame: Rect) -> Option<(f32, f32)> {
        None
    }
    /// 帧间滚动偏移（用于 scroll_region 像素移动优化）。
    fn scroll_delta_for_dirty(&self) -> Option<(f32, f32)> {
        None
    }
    /// viewport 容器当前 scroll 偏移（content 坐标系）；非 viewport 返回 `None`。
    fn viewport_scroll_offset(&self) -> Option<(f32, f32)> {
        None
    }
    fn hit_test_frame(&self, actual_frame: Rect) -> Rect {
        actual_frame
    }
    fn hit_test_3d(&self, ray: &Ray3D, _spatial: &SpatialContext, frame: Rect) -> bool {
        if let Some(hit_point) = ray.intersect_z0() {
            hit_point.x >= frame.x
                && hit_point.x <= frame.x + frame.w
                && hit_point.y >= frame.y
                && hit_point.y <= frame.y + frame.h
        } else {
            false
        }
    }
}

/// 生命周期行为。
pub trait WidgetLifecycle: WidgetComponent {
    fn on_init(&mut self) {}
    fn on_mount(&mut self) {}
    fn on_unmount(&mut self) {}
    fn on_update(&mut self, _dt: f64) {}
}

/// 转换为 WidgetNode 的 trait。
pub trait IntoWidgetNode {
    fn into_node(self) -> WidgetNode;
}

impl<T: WidgetComponent + 'static> IntoWidgetNode for T {
    fn into_node(self) -> WidgetNode {
        WidgetNode::leaf(Box::new(self))
    }
}

impl IntoWidgetNode for WidgetNode {
    fn into_node(self) -> WidgetNode {
        self
    }
}
