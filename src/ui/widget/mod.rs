use crate::base::{Point, Rect, Size};

// Platform types are used directly by WidgetEvent - no conversion needed.
pub use crate::base::{KeyCode, KeyMod, MouseButton};

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

// ── AsAny — 安全下转型支持 ────────────────────────────────────────

/// 安全下转型:从 `&dyn Widget` 向下转型到具体类型。
pub trait AsAny {
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

impl<T: 'static> AsAny for T {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Widget trait — 核心行为抽象。纯行为，不含树元数据。
pub trait Widget: AsAny {
    fn build(&self) -> Vec<Box<dyn Widget>> {
        vec![]
    }

    fn on_init(&mut self) {}
    fn on_mount(&mut self) {}
    fn on_unmount(&mut self) {}
    fn on_event(&mut self, _event: &WidgetEvent) -> EventResult {
        EventResult::NotHandled
    }
    fn on_update(&mut self, _dt: f32) {}

    /// 是否需要持续更新/渲染帧（如动画）。
    fn needs_continuous_update(&self) -> bool {
        false
    }

    /// 返回需要重新渲染的区域（动画边界可能超出 frame）。
    fn dirty_rect(&self, frame: Rect) -> Rect {
        frame
    }

    /// 返回像素缓冲滚动偏移量（如 ScrollView）。
    fn scroll_delta(&self, _frame: Rect) -> Option<(f32, f32)> {
        None
    }

    /// 返回首选尺寸用于布局。
    fn preferred_size(&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        Size::zero()
    }

    /// 渲染本组件到 render context。在孩子之前调用。
    fn render(
        &self,
        frame: Rect,
        ctx: &mut crate::ui::render_context::RenderContext,
        tree: &WidgetTree,
    );

    /// 在孩子之后渲染覆盖层（滚动条、涟漪等）。
    fn post_render(
        &self,
        _frame: Rect,
        _ctx: &mut crate::ui::render_context::RenderContext,
        _tree: &WidgetTree,
    ) {
    }

    fn flex_grow(&self) -> f32 {
        0.0
    }
    fn flex_shrink(&self) -> f32 {
        0.0
    }

    /// 返回子 widget 的裁剪矩形（如 ScrollView 视口）。
    fn children_clip(&self, _frame: Rect) -> Option<Rect> {
        None
    }

    /// 是否是重绘边界（RepaintBoundary）。
    fn is_repaint_boundary(&self) -> bool {
        false
    }

    /// 计算子布局。返回 (child_id, rect) 对。
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

/// 转换为 WidgetNode 的 trait。
pub trait IntoWidgetNode {
    fn into_node(self) -> WidgetNode;
}

impl<T: Widget + 'static> IntoWidgetNode for T {
    fn into_node(self) -> WidgetNode {
        WidgetNode::leaf(Box::new(self))
    }
}

impl IntoWidgetNode for WidgetNode {
    fn into_node(self) -> WidgetNode {
        self
    }
}

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
    pub fn preferred_size(&self, engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
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
mod tree_render;
pub use tree_core::WidgetTree;
