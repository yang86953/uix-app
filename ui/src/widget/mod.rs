use uix_platform::{Point, Rect, Size};
pub use uix_platform::{KeyCode, KeyMod, MouseButton};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult { Handled, NotHandled, Bubbled }

#[derive(Debug, Clone)]
pub enum WidgetEvent {
    MouseDown { pos: Point, button: MouseButton, mods: KeyMod },
    MouseUp { pos: Point, button: MouseButton, mods: KeyMod },
    MouseMove { pos: Point },
    MouseWheel { pos: Point, delta: Point },
    KeyDown { key: KeyCode, mods: KeyMod },
    KeyUp { key: KeyCode, mods: KeyMod },
    KeyPress { text: String },
    FocusIn, FocusOut, HoverEnter, HoverLeave,
    Resize { width: f32, height: f32 },
    WindowMaximize, WindowMinimize, WindowRestore, WindowFocus, WindowBlur,
    Timer { id: u32 },
    FileDrop { files: Vec<String>, position: Point },
}

pub type WidgetId = usize;

// 重新导出 api 中的 trait 定义
pub use crate::api::traits::{
    WidgetCapabilities, WidgetComponent, WidgetLayout, WidgetRender,
    WidgetEventHandler, WidgetLifecycle, IntoWidgetNode,
};

pub struct WidgetNode {
    pub widget: Box<dyn WidgetComponent>,
    pub children: Vec<WidgetNode>,
    pub z_index: i32,
    pub key: Option<Box<str>>,
}

impl WidgetNode {
    pub fn new(widget: Box<dyn WidgetComponent>, children: Vec<WidgetNode>) -> Self {
        Self { widget, children, z_index: 0, key: None }
    }
    pub fn key(mut self, k: &str) -> Self { self.key = Some(k.into()); self }
    pub fn leaf(widget: Box<dyn WidgetComponent>) -> Self {
        Self { widget, children: vec![], z_index: 0, key: None }
    }
    pub fn z_index(mut self, z: i32) -> Self { self.z_index = z; self }
}

pub trait WidgetCore {
    fn id(&self) -> WidgetId; fn set_id(&mut self, id: WidgetId);
    fn parent(&self) -> Option<WidgetId>; fn set_parent(&mut self, id: Option<WidgetId>);
    fn children(&self) -> &[WidgetId]; fn children_mut(&mut self) -> &mut Vec<WidgetId>;
    fn frame(&self) -> Rect; fn set_frame(&mut self, rect: Rect);
    fn visible(&self) -> bool; fn set_visible(&mut self, v: bool);
    fn dirty(&self) -> bool; fn set_dirty(&mut self, v: bool);
    fn opacity(&self) -> f32; fn set_opacity(&mut self, v: f32);
    fn z_index(&self) -> i32; fn set_z_index(&mut self, v: i32);
}

pub struct BoxedWidget {
    component: Box<dyn WidgetComponent>,
    caps: WidgetCapabilities,
    id: WidgetId, parent: Option<WidgetId>, children: Vec<WidgetId>,
    frame: Rect, visible: bool, is_dirty: bool, widget_opacity: f32, z: i32,
}

impl BoxedWidget {
    pub fn new(component: Box<dyn WidgetComponent>) -> Self {
        let caps = component.capabilities();
        Self {
            component, caps,
            id: 0, parent: None, children: Vec::new(),
            frame: Rect::zero(), visible: true, is_dirty: true,
            widget_opacity: 1.0, z: 0,
        }
    }
    pub fn component(&self) -> &dyn WidgetComponent { &*self.component }
    pub fn component_mut(&mut self) -> &mut dyn WidgetComponent { &mut *self.component }
    pub fn capabilities(&self) -> WidgetCapabilities { self.caps }

    pub fn as_render(&self) -> Option<&dyn WidgetRender> { self.component.as_render() }
    pub fn as_render_mut(&mut self) -> Option<&mut dyn WidgetRender> { self.component.as_render_mut() }
    pub fn as_event(&self) -> Option<&dyn WidgetEventHandler> { self.component.as_event() }
    pub fn as_event_mut(&mut self) -> Option<&mut dyn WidgetEventHandler> { self.component.as_event_mut() }
    pub fn as_lifecycle(&self) -> Option<&dyn WidgetLifecycle> { self.component.as_lifecycle() }
    pub fn as_lifecycle_mut(&mut self) -> Option<&mut dyn WidgetLifecycle> { self.component.as_lifecycle_mut() }
    pub fn as_layout(&self) -> Option<&dyn WidgetLayout> { self.component.as_layout() }
}

impl WidgetCore for BoxedWidget {
    fn id(&self) -> WidgetId { self.id } fn set_id(&mut self, id: WidgetId) { self.id = id; }
    fn parent(&self) -> Option<WidgetId> { self.parent } fn set_parent(&mut self, id: Option<WidgetId>) { self.parent = id; }
    fn children(&self) -> &[WidgetId] { &self.children } fn children_mut(&mut self) -> &mut Vec<WidgetId> { &mut self.children }
    fn frame(&self) -> Rect { self.frame } fn set_frame(&mut self, rect: Rect) { self.frame = rect; }
    fn visible(&self) -> bool { self.visible } fn set_visible(&mut self, v: bool) { self.visible = v; }
    fn dirty(&self) -> bool { self.is_dirty } fn set_dirty(&mut self, v: bool) { self.is_dirty = v; }
    fn opacity(&self) -> f32 { self.widget_opacity } fn set_opacity(&mut self, v: f32) { self.widget_opacity = v; }
    fn z_index(&self) -> i32 { self.z } fn set_z_index(&mut self, v: i32) { self.z = v; }
}

mod tree_core;
mod tree_dirty;
mod tree_events;
pub use tree_core::WidgetTree;
