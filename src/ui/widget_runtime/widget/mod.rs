pub(crate) use crate::core::Point;
pub(crate) use crate::core::WidgetId;
use crate::core::{Constraints, Rect, Size};
use crate::draw::geometry::spatial::{Ray3D, SpatialContext};
use crate::draw::scene::PicturePolicy;
pub(crate) use crate::platform::windowing::{KeyCode, KeyMod, MouseButton};
use crate::ui::accessibility::accessibility_override::AccessibilityOverride;
pub(crate) use crate::ui::event::SystemEvent;
use crate::ui::event::system_event_handler::SystemEventHandlerRegistration;
use crate::ui::event::{HandlerRegistration, HandlerSignature};
use crate::ui::render_handler::RenderHandlerRegistration;
use crate::ui::widget_runtime::focus_handle::FocusHandle;
use crate::ui::widget_runtime::provider_context::{
    ProviderContext, current_provider_context, with_provider_context,
};
use crate::ui::widget_runtime::view_transform::ViewTransform;
use crate::ui::widget_snapshot::{AccessibilitySnapshot, SnapshotFields, WidgetConfigSnapshot};

// EventResult 归 event Module（事件处理结果契约）；此处重导出保持树内路径不变。
pub(crate) use crate::ui::event::EventResult;

// 重新导出 api 中的 trait 定义
pub(crate) use crate::ui::widget_runtime::traits::{
    EventHandler, Widget, WidgetCapabilities, WidgetLayout, WidgetLifecycle, WidgetRender,
    WidgetTextInput,
};

pub(crate) trait WidgetCore {
    // 测试与 test-harness 目标保留组件 id 观测契约，生产默认路径不直接读取它。
    #[cfg_attr(any(test, feature = "test-harness"), allow(dead_code))]
    #[cfg(any(test, feature = "test-harness"))]
    fn id(&self) -> WidgetId;
    fn set_id(&mut self, id: WidgetId);
    fn parent(&self) -> Option<WidgetId>;
    fn set_parent(&mut self, id: Option<WidgetId>);
    fn children(&self) -> &[WidgetId];
    fn children_mut(&mut self) -> &mut Vec<WidgetId>;
    fn frame(&self) -> Rect;
    fn set_frame(&mut self, rect: Rect);
    fn visible(&self) -> bool;
    fn set_visible(&mut self, v: bool);
    fn z_index(&self) -> i32;
    fn set_z_index(&mut self, v: i32);
    /// Tab 键导航顺序索引。0 = 不可通过 Tab 导航获取焦点，> 0 = 可聚焦。
    fn tab_index(&self) -> i32;
    fn set_tab_index(&mut self, v: i32);
}

mod boxed;
mod node;

pub(crate) use self::boxed::BoxedWidget;
pub use self::node::WidgetNode;
// 声明透明度归一规则同时服务声明节点与运行时节点两个写入点。
pub(crate) use self::node::normalize_declared_opacity;

pub(crate) mod tree_core;
pub(crate) mod tree_dirty;
pub(crate) mod tree_events;
mod tree_semantics;
mod tree_transform;
mod tree_transition;
pub use tree_core::WidgetTree;
