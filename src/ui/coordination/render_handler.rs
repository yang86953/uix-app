//! Tree-owned, type-erased component render registrations.
use std::{any::{Any, TypeId}, collections::HashMap, sync::Arc};
use crate::core::WidgetId;
use crate::ui::view::ViewNode;

pub struct RenderHandlerRegistration {
    kind: TypeId,
    value: Box<dyn Any>,
}
impl RenderHandlerRegistration {
    pub fn new<T: Any>(value: T) -> Self { Self { kind: TypeId::of::<T>(), value: Box::new(value) } }
}
#[derive(Default)]
pub struct RenderHandlerTable {
    handlers: HashMap<WidgetId, HashMap<TypeId, Box<dyn Any>>>,
}
impl RenderHandlerTable {
    pub(crate) fn replace_widget(&mut self, owner: WidgetId, handlers: Vec<RenderHandlerRegistration>) {
        if handlers.is_empty() { self.handlers.remove(&owner); return; }
        self.handlers.insert(owner, handlers.into_iter().map(|handler| (handler.kind, handler.value)).collect());
    }
    pub fn get<T: Any>(&self, owner: WidgetId) -> Option<&T> {
        self.handlers.get(&owner)?.get(&TypeId::of::<T>())?.downcast_ref()
    }
    pub fn get_mut<T: Any>(&mut self, owner: WidgetId) -> Option<&mut T> {
        self.handlers.get_mut(&owner)?.get_mut(&TypeId::of::<T>())?.downcast_mut()
    }
    pub(crate) fn clear_widget(&mut self, owner: WidgetId) { self.handlers.remove(&owner); }
    pub(crate) fn clear(&mut self) { self.handlers.clear(); }
}

// ── 空态渲染回调（System 私有边界）───────────────────────────────
//
// `EmptyRenderer` 由 WidgetConfig（widget）持有、widgets 消费、
// 用户闭包产出 ViewNode（view）：跨 Module 契约归本边界（SMC-04）。

use crate::ui::view::View;
use crate::ui::widget_runtime::config::{WidgetConfig, use_config};
use crate::ui::widget_runtime::traits::Widget;

/// 标识正在请求空态 View 的数据组件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmptyContext {
    widget_name: &'static str,
}

impl EmptyContext {
    fn of<T: Widget>() -> Self {
        let full_name = std::any::type_name::<T>();
        Self {
            widget_name: full_name.rsplit("::").next().map_or(full_name, |name| name),
        }
    }

    /// 返回请求空态视图的组件短类型名。
    pub fn widget_name(self) -> &'static str {
        self.widget_name
    }
}

/// 由 `WidgetConfig` 持有的可克隆空态 View factory。
#[derive(Clone)]
pub struct EmptyRenderer {
    renderer: Arc<dyn Fn(EmptyContext) -> ViewNode + Send + Sync>,
}

impl EmptyRenderer {
    /// 使用将空态上下文转换为视图的工厂创建渲染器。
    pub fn new<F, V>(renderer: F) -> Self
    where
        F: Fn(EmptyContext) -> V + Send + Sync + 'static,
        V: View,
    {
        Self {
            renderer: Arc::new(move |context| renderer(context).build()),
        }
    }

    /// 为指定组件类型构建一棵空态视图节点树。
    pub fn render<T: Widget>(&self) -> ViewNode {
        (self.renderer)(EmptyContext::of::<T>())
    }

    pub(crate) fn is_same_renderer(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.renderer, &other.renderer)
    }
}

/// 为指定组件类型构建当前配置的空态 View。
pub fn render_empty_for<T: Widget>() -> Option<ViewNode> {
    use_config()
        .empty_renderer
        .map(|renderer| renderer.render::<T>())
}

impl WidgetConfig {
    /// 为数据组件设置空态 View factory（方法定义随跨 Module 契约归本边界）。
    pub fn render_empty<F, V>(mut self, renderer: F) -> Self
    where
        F: Fn(EmptyContext) -> V + Send + Sync + 'static,
        V: View,
    {
        self.empty_renderer = Some(EmptyRenderer::new(renderer));
        self
    }
}
