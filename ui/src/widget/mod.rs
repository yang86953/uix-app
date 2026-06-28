use uix_platform::{Point, Rect, Size};

// Platform types are used directly by WidgetEvent - no conversion needed.
pub use uix_platform::{KeyCode, KeyMod, MouseButton};

/// Event result enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult {
    Handled,
    NotHandled,
    Bubbled,
}

/// Widget event types.
#[derive(Debug, Clone)]
pub enum WidgetEvent {
    MouseDown { pos: Point, button: MouseButton, mods: KeyMod },
    MouseUp { pos: Point, button: MouseButton, mods: KeyMod },
    MouseMove { pos: Point },
    MouseWheel { pos: Point, delta: Point },
    KeyDown { key: KeyCode, mods: KeyMod },
    KeyUp { key: KeyCode, mods: KeyMod },
    KeyPress { text: String },
    FocusIn,
    FocusOut,
    HoverEnter,
    HoverLeave,
    Resize { width: f32, height: f32 },
    /// 窗口状态变化
    WindowMaximize,
    WindowMinimize,
    WindowRestore,
    WindowFocus,
    WindowBlur,
    /// 定时器触发
    Timer { id: u32 },
    /// 文件拖放
    FileDrop { files: Vec<String>, position: Point },
}

/// Widget tree node ID.
pub type WidgetId = usize;

// ── AsAny — 安全下转型支持（内联到 Widget trait） ────────────────
//
// as_any / as_any_mut 已从独立的 AsAny trait 移到 Widget trait 中，
// 消除 trait 继承。define_widget! 宏自动生成实现，手动 impl Widget
// 需要显式实现这两个方法。

// Widget / WidgetLayout / WidgetRender / WidgetEventHandler / WidgetLifecycle / IntoWidgetNode
// trait 定义已迁移至 api/traits.rs
pub use crate::api::traits::{IntoWidgetNode, Widget, WidgetEventHandler, WidgetLayout, WidgetLifecycle, WidgetRender};

// ── WidgetNode — 可组合 widget 节点 ──────────────────────────────

/// 可组合 widget 节点，支持声明式树构建。
///
/// `key` 字段与 React `key` prop 类似：在树重建时，`WidgetTree::build_node`
/// 按 `(key, parent_id)` 匹配旧节点，复用其 `widget_id`。
/// 这样 LayerTree 的 Picture 缓存（按 widget_id 索引）可以跨重建保持命中。
pub struct WidgetNode {
    pub widget: Box<dyn Widget>,
    pub children: Vec<WidgetNode>,
    pub z_index: i32,
    /// 稳定标识符，树重建时用于匹配旧节点。同一层级下必须唯一。
    pub key: Option<Box<str>>,
}

impl WidgetNode {
    pub fn new(widget: Box<dyn Widget>, children: Vec<WidgetNode>) -> Self {
        Self {
            widget,
            children,
            z_index: 0,
            key: None,
        }
    }
    /// 设置稳定 key，树重建时保持该节点的 widget_id 不变。
    pub fn key(mut self, k: &str) -> Self {
        self.key = Some(k.into());
        self
    }
    pub fn leaf(widget: Box<dyn Widget>) -> Self {
        Self {
            widget,
            children: vec![],
            z_index: 0,
            key: None,
        }
    }
    pub fn z_index(mut self, z: i32) -> Self {
        self.z_index = z;
        self
    }
}

// IntoWidgetNode trait + impls 已迁移至 api/traits.rs

/// Core widget tree metadata — 由 BoxedWidget 管理，widget 不直接接触。
pub trait WidgetCore {
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
    fn dirty(&self) -> bool;
    fn set_dirty(&mut self, v: bool);
    fn opacity(&self) -> f32;
    fn set_opacity(&mut self, v: f32);
    fn z_index(&self) -> i32;
    fn set_z_index(&mut self, v: i32);
}

// ── BoxedWidget — trait object 包装器 ────────────────────────────

/// 类型擦除的 widget，管理树元数据。
pub struct BoxedWidget {
    inner: Box<dyn Widget>,
    id: WidgetId,
    parent: Option<WidgetId>,
    children: Vec<WidgetId>,
    frame: Rect,
    visible: bool,
    is_dirty: bool,
    widget_opacity: f32,
    z: i32,
}

impl BoxedWidget {
    pub fn new(inner: Box<dyn Widget>) -> Self {
        Self {
            inner,
            id: 0,
            parent: None,
            children: Vec::new(),
            frame: Rect::zero(),
            visible: true,
            is_dirty: true,
            widget_opacity: 1.0,
            z: 0,
        }
    }
    pub fn inner(&self) -> &dyn Widget {
        &*self.inner
    }
    pub fn inner_mut(&mut self) -> &mut dyn Widget {
        &mut *self.inner
    }
    pub fn preferred_size(&self, engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        self.inner.preferred_size(engine)
    }
}

impl WidgetCore for BoxedWidget {
    fn id(&self) -> WidgetId {
        self.id
    }
    fn set_id(&mut self, id: WidgetId) {
        self.id = id;
    }
    fn parent(&self) -> Option<WidgetId> {
        self.parent
    }
    fn set_parent(&mut self, id: Option<WidgetId>) {
        self.parent = id;
    }
    fn children(&self) -> &[WidgetId] {
        &self.children
    }
    fn children_mut(&mut self) -> &mut Vec<WidgetId> {
        &mut self.children
    }
    fn frame(&self) -> Rect {
        self.frame
    }
    fn set_frame(&mut self, rect: Rect) {
        self.frame = rect;
    }
    fn visible(&self) -> bool {
        self.visible
    }
    fn set_visible(&mut self, v: bool) {
        self.visible = v;
    }
    fn dirty(&self) -> bool {
        self.is_dirty
    }
    fn set_dirty(&mut self, v: bool) {
        self.is_dirty = v;
    }
    fn opacity(&self) -> f32 {
        self.widget_opacity
    }
    fn set_opacity(&mut self, v: f32) {
        self.widget_opacity = v;
    }
    fn z_index(&self) -> i32 {
        self.z
    }
    fn set_z_index(&mut self, v: i32) {
        self.z = v;
    }
}

// ══════════════════════════════════════════════════════════════════
// 子模块
// ══════════════════════════════════════════════════════════════════

mod tree_core;
mod tree_dirty;
mod tree_events;
pub use tree_core::WidgetTree;
