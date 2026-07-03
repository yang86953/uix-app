pub use uix_platform::{KeyCode, KeyMod, MouseButton};
use uix_platform::{Point, Rect, Size};

/// WidgetEvent 的种类区分（无载荷），用于事件管理器按类型过滤。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WidgetEventKind {
    MouseDown,
    MouseUp,
    MouseMove,
    MouseWheel,
    KeyDown,
    KeyUp,
    KeyPress,
    FocusIn,
    FocusOut,
    HoverEnter,
    HoverLeave,
    Resize,
    WindowMaximize,
    WindowMinimize,
    WindowRestore,
    WindowFocus,
    WindowBlur,
    Timer,
    FileDrop,
    /// 组合事件：拖拽
    DragStart,
    DragMove,
    DragEnd,
}
use uix_graphics::spatial::{Ray3D, SpatialContext};
use uix_graphics::GraphicsEngine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult {
    Handled,
    NotHandled,
    Bubbled,
}

#[derive(Debug, Clone)]
pub enum WidgetEvent {
    MouseDown {
        pos: Point,
        button: MouseButton,
        mods: KeyMod,
    },
    MouseUp {
        pos: Point,
        button: MouseButton,
        mods: KeyMod,
    },
    MouseMove {
        pos: Point,
        mods: KeyMod,
    },
    MouseWheel {
        pos: Point,
        delta: Point,
    },
    KeyDown {
        key: KeyCode,
        mods: KeyMod,
    },
    KeyUp {
        key: KeyCode,
        mods: KeyMod,
    },
    KeyPress {
        text: String,
    },
    FocusIn,
    FocusOut,
    HoverEnter,
    HoverLeave,
    Resize {
        width: f32,
        height: f32,
    },
    WindowMaximize,
    WindowMinimize,
    WindowRestore,
    WindowFocus,
    WindowBlur,
    Timer {
        id: u32,
    },
    FileDrop {
        files: Vec<String>,
        position: Point,
    },
    /// 组合事件：拖拽开始（MouseDown + MouseMove 超出阈值后触发）
    DragStart {
        pos: Point,
        button: MouseButton,
        mods: KeyMod,
    },
    /// 组合事件：拖拽移动
    DragMove {
        pos: Point,
        delta: Point,
        mods: KeyMod,
    },
    /// 组合事件：拖拽结束
    DragEnd {
        pos: Point,
        button: MouseButton,
        mods: KeyMod,
    },
}

impl WidgetEvent {
    /// 返回事件的种类（忽略载荷），用于事件管理器按类型过滤。
    pub fn kind(&self) -> WidgetEventKind {
        match self {
            WidgetEvent::MouseDown { .. } => WidgetEventKind::MouseDown,
            WidgetEvent::MouseUp { .. } => WidgetEventKind::MouseUp,
            WidgetEvent::MouseMove { .. } => WidgetEventKind::MouseMove,
            WidgetEvent::MouseWheel { .. } => WidgetEventKind::MouseWheel,
            WidgetEvent::KeyDown { .. } => WidgetEventKind::KeyDown,
            WidgetEvent::KeyUp { .. } => WidgetEventKind::KeyUp,
            WidgetEvent::KeyPress { .. } => WidgetEventKind::KeyPress,
            WidgetEvent::FocusIn => WidgetEventKind::FocusIn,
            WidgetEvent::FocusOut => WidgetEventKind::FocusOut,
            WidgetEvent::HoverEnter => WidgetEventKind::HoverEnter,
            WidgetEvent::HoverLeave => WidgetEventKind::HoverLeave,
            WidgetEvent::Resize { .. } => WidgetEventKind::Resize,
            WidgetEvent::WindowMaximize => WidgetEventKind::WindowMaximize,
            WidgetEvent::WindowMinimize => WidgetEventKind::WindowMinimize,
            WidgetEvent::WindowRestore => WidgetEventKind::WindowRestore,
            WidgetEvent::WindowFocus => WidgetEventKind::WindowFocus,
            WidgetEvent::WindowBlur => WidgetEventKind::WindowBlur,
            WidgetEvent::Timer { .. } => WidgetEventKind::Timer,
            WidgetEvent::FileDrop { .. } => WidgetEventKind::FileDrop,
            WidgetEvent::DragStart { .. } => WidgetEventKind::DragStart,
            WidgetEvent::DragMove { .. } => WidgetEventKind::DragMove,
            WidgetEvent::DragEnd { .. } => WidgetEventKind::DragEnd,
        }
    }
}

pub type WidgetId = usize;

// 重新导出 api 中的 trait 定义
pub use crate::api::traits::{
    IntoWidgetNode, WidgetCapabilities, WidgetComponent, WidgetEventHandler, WidgetLayout,
    WidgetLifecycle, WidgetRender,
};

pub struct WidgetNode {
    pub widget: Box<dyn WidgetComponent>,
    pub children: Vec<WidgetNode>,
    pub z_index: i32,
    pub key: Option<Box<str>>,
    pub tab_idx: i32,
}

impl WidgetNode {
    pub fn new(widget: Box<dyn WidgetComponent>, children: Vec<WidgetNode>) -> Self {
        Self {
            widget,
            children,
            z_index: 0,
            key: None,
            tab_idx: 0,
        }
    }
    pub fn key(mut self, k: &str) -> Self {
        self.key = Some(k.into());
        self
    }
    pub fn leaf(widget: Box<dyn WidgetComponent>) -> Self {
        Self {
            widget,
            children: vec![],
            z_index: 0,
            key: None,
            tab_idx: 0,
        }
    }
    pub fn z_index(mut self, z: i32) -> Self {
        self.z_index = z;
        self
    }
    /// 设置 Tab 键导航顺序索引（> 0 表示可通过 Tab 获取焦点）。
    pub fn tab_index(mut self, idx: i32) -> Self {
        self.tab_idx = idx;
        self
    }
}

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
    /// Tab 键导航顺序索引。0 = 不可通过 Tab 导航获取焦点，> 0 = 可聚焦。
    fn tab_index(&self) -> i32;
    fn set_tab_index(&mut self, v: i32);
}

pub struct BoxedWidget {
    component: Box<dyn WidgetComponent>,
    caps: WidgetCapabilities,
    id: WidgetId,
    parent: Option<WidgetId>,
    children: Vec<WidgetId>,
    frame: Rect,
    visible: bool,
    is_dirty: bool,
    widget_opacity: f32,
    z: i32,
    /// Tab 键导航顺序（0=不可通过 Tab 导航聚焦）。
    tab_idx: i32,
}

impl BoxedWidget {
    pub fn new(component: Box<dyn WidgetComponent>) -> Self {
        let caps = component.capabilities();
        Self {
            component,
            caps,
            id: 0,
            parent: None,
            children: Vec::new(),
            frame: Rect::zero(),
            visible: true,
            is_dirty: true,
            widget_opacity: 1.0,
            z: 0,
            tab_idx: 0,
        }
    }
    pub fn component(&self) -> &dyn WidgetComponent {
        &*self.component
    }
    pub fn component_mut(&mut self) -> &mut dyn WidgetComponent {
        &mut *self.component
    }
    pub fn capabilities(&self) -> WidgetCapabilities {
        self.caps
    }

    pub fn as_render(&self) -> Option<&dyn WidgetRender> {
        self.component.as_render()
    }
    pub fn as_render_mut(&mut self) -> Option<&mut dyn WidgetRender> {
        self.component.as_render_mut()
    }
    pub fn as_event(&self) -> Option<&dyn WidgetEventHandler> {
        self.component.as_event()
    }
    pub fn as_event_mut(&mut self) -> Option<&mut dyn WidgetEventHandler> {
        self.component.as_event_mut()
    }
    pub fn as_layout(&self) -> Option<&dyn WidgetLayout> {
        self.component.as_layout()
    }

    pub fn as_lifecycle(&self) -> Option<&dyn WidgetLifecycle> {
        self.component.as_lifecycle()
    }

    pub fn as_lifecycle_mut(&mut self) -> Option<&mut dyn WidgetLifecycle> {
        self.component.as_lifecycle_mut()
    }

    // ═══ 便捷分发方法 ═══

    pub fn preferred_size(&self, engine: Option<&dyn GraphicsEngine>) -> Size {
        self.component()
            .as_layout()
            .map(|l| l.preferred_size(engine))
            .unwrap_or_default()
    }
    pub fn flex_grow(&self) -> f32 {
        self.component()
            .as_layout()
            .map(|l| l.flex_grow())
            .unwrap_or(0.0)
    }
    pub fn flex_shrink(&self) -> f32 {
        self.component()
            .as_layout()
            .map(|l| l.flex_shrink())
            .unwrap_or(0.0)
    }
    pub fn layout_children(
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
    ) -> Vec<(WidgetId, Rect)> {
        self.component()
            .as_layout()
            .map(|l| l.layout_children(frame, children, tree))
            .unwrap_or_default()
    }
    pub fn children_clip(&self, frame: Rect) -> Option<Rect> {
        self.component()
            .as_render()
            .and_then(|r| r.children_clip(frame))
    }
    pub fn dirty_rect(&self, frame: Rect) -> Rect {
        self.component()
            .as_render()
            .map(|r| r.dirty_rect(frame))
            .unwrap_or(frame)
    }
    pub fn needs_continuous_update(&self) -> bool {
        self.component()
            .as_event()
            .map(|e| e.needs_continuous_update())
            .unwrap_or(false)
    }
    pub fn scroll_delta(&self, frame: Rect) -> Option<(f32, f32)> {
        self.component()
            .as_event()
            .and_then(|e| e.scroll_delta(frame))
    }
    pub fn scroll_delta_for_dirty(&self) -> Option<(f32, f32)> {
        self.component()
            .as_event()
            .and_then(|e| e.scroll_delta_for_dirty())
    }
    pub fn hit_test_frame(&self, actual_frame: Rect) -> Rect {
        self.component()
            .as_event()
            .map(|e| e.hit_test_frame(actual_frame))
            .unwrap_or(actual_frame)
    }
    pub fn hit_test_3d(&self, ray: &Ray3D, spatial: &SpatialContext, frame: Rect) -> bool {
        self.component()
            .as_event()
            .map(|e| e.hit_test_3d(ray, spatial, frame))
            .unwrap_or(false)
    }
    pub fn on_event(&mut self, event: &WidgetEvent) -> EventResult {
        self.component_mut()
            .as_event_mut()
            .map(|e| e.on_event(event))
            .unwrap_or(EventResult::NotHandled)
    }
    pub fn is_focusable(&self) -> bool {
        self.tab_idx > 0 && self.visible && self.component.as_event().is_some()
    }

    pub fn on_update(&mut self, dt: f64) {
        if let Some(l) = self.component_mut().as_lifecycle_mut() {
            l.on_update(dt);
        }
    }
    pub fn render(
        &self,
        frame: Rect,
        ctx: &mut crate::render_context::RenderContext,
        tree: &WidgetTree,
    ) {
        if let Some(r) = self.component().as_render() {
            r.render(frame, ctx, tree);
        }
    }
    pub fn post_render(
        &self,
        frame: Rect,
        ctx: &mut crate::render_context::RenderContext,
        tree: &WidgetTree,
    ) {
        if let Some(r) = self.component().as_render() {
            r.post_render(frame, ctx, tree);
        }
    }
    pub fn is_repaint_boundary(&self) -> bool {
        self.component()
            .as_render()
            .map(|r| r.is_repaint_boundary())
            .unwrap_or(false)
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
    fn tab_index(&self) -> i32 {
        self.tab_idx
    }
    fn set_tab_index(&mut self, v: i32) {
        self.tab_idx = v;
    }
}

mod tree_core;
mod tree_dirty;
mod tree_events;
pub use tree_core::WidgetTree;
