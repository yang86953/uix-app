pub use crate::core::ComponentId;
pub use crate::core::Point;
use crate::core::{Constraints, Rect, Size};
use crate::draw::geometry::spatial::{Ray3D, SpatialContext};
use crate::draw::scene::PicturePolicy;
pub use crate::native::windowing::input::{KeyCode, KeyMod, MouseButton};
use crate::ui::accessibility::accessibility_override::AccessibilityOverride;
use crate::ui::component::focus_handle::FocusHandle;
use crate::ui::component::provider_context::{
    current_provider_context, with_provider_context, ProviderContext,
};
use crate::ui::component::view_transform::ViewTransform;
use crate::ui::component_snapshot::{
    AccessibilitySnapshot, ComponentConfigSnapshot, SnapshotFields,
};
use crate::ui::event::system_event_handler::SystemEventHandlerRegistration;
pub use crate::ui::event::SystemEvent;
use crate::ui::event::{HandlerRegistration, HandlerSignature};
use crate::ui::render_handler::RenderHandlerRegistration;

pub(crate) type WidgetId = ComponentId;

// EventResult 归 event Module（事件处理结果契约）；此处重导出保持树内路径不变。
pub use crate::ui::event::EventResult;

// 重新导出 api 中的 trait 定义
pub use crate::ui::component::traits::{
    EventHandler, WidgetCapabilities, WidgetComponent, WidgetLayout, WidgetLifecycle, WidgetRender,
    WidgetTextInput,
};

pub trait WidgetCore {
    #[cfg(any(test, feature = "test-harness"))]
    fn id(&self) -> ComponentId;
    fn set_id(&mut self, id: ComponentId);
    fn parent(&self) -> Option<ComponentId>;
    fn set_parent(&mut self, id: Option<ComponentId>);
    fn children(&self) -> &[ComponentId];
    fn children_mut(&mut self) -> &mut Vec<ComponentId>;
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

pub use self::boxed::BoxedWidget;
pub use self::node::WidgetNode;

pub(crate) mod tree_core;
pub(crate) mod tree_dirty;
pub(crate) mod tree_events;
mod tree_semantics;
mod tree_transform;
mod tree_transition;
pub use tree_core::WidgetTree;
