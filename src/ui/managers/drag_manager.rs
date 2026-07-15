use crate::core::Point;
use crate::native::traits::input::{KeyMod, MouseButton};
use crate::ui::ComponentId;

/// Result of a drag target operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragEventResult {
    Accepted,
    Rejected,
    Ignored,
}

/// Manages drag-and-drop for a component.
pub struct DragManager {
    dragging: bool,
    drag_start_pos: Point,
    drag_offset: Point,
    potential: bool,
    last_pos: Point,
    button: MouseButton,
    mods: KeyMod,
    target: Option<ComponentId>,
}

impl Default for DragManager {
    fn default() -> Self {
        Self {
            dragging: false,
            drag_start_pos: Point::zero(),
            drag_offset: Point::zero(),
            potential: false,
            last_pos: Point::zero(),
            button: MouseButton::None,
            mods: KeyMod::NONE,
            target: None,
        }
    }
}

impl DragManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_drag(&mut self, pos: Point) {
        self.dragging = true;
        self.potential = false;
        self.drag_start_pos = pos;
        self.last_pos = pos;
        self.drag_offset = Point::zero();
    }

    pub fn begin_gesture(
        &mut self,
        target: Option<ComponentId>,
        pos: Point,
        button: MouseButton,
        mods: KeyMod,
    ) {
        self.potential = true;
        self.dragging = false;
        self.drag_start_pos = pos;
        self.last_pos = pos;
        self.drag_offset = Point::zero();
        self.button = button;
        self.mods = mods;
        self.target = target;
    }

    pub fn activate_gesture(&mut self) {
        self.potential = false;
        self.dragging = true;
    }

    pub fn update_drag(&mut self, pos: Point) {
        if self.dragging || self.potential {
            self.drag_offset =
                Point::new(pos.x - self.drag_start_pos.x, pos.y - self.drag_start_pos.y);
            self.last_pos = pos;
        }
    }

    pub fn update_last_pos(&mut self, pos: Point) {
        self.last_pos = pos;
    }

    pub fn end_drag(&mut self) {
        self.dragging = false;
        self.potential = false;
        self.drag_offset = Point::zero();
        self.button = MouseButton::None;
        self.mods = KeyMod::NONE;
        self.target = None;
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    pub fn is_potential(&self) -> bool {
        self.potential
    }

    pub fn target(&self) -> Option<ComponentId> {
        self.target
    }

    pub fn start_pos(&self) -> Point {
        self.drag_start_pos
    }

    pub fn last_pos(&self) -> Point {
        self.last_pos
    }

    pub fn button(&self) -> MouseButton {
        self.button
    }

    pub fn is_gesture_button(&self, button: MouseButton) -> bool {
        (self.dragging || self.potential) && self.button == button
    }

    pub fn mods(&self) -> KeyMod {
        self.mods
    }

    pub fn drag_offset(&self) -> Point {
        self.drag_offset
    }

    pub fn unregister_component(&mut self, component_id: ComponentId) {
        if self.target == Some(component_id) {
            self.end_drag();
        }
    }

    pub fn clear_tree_drag(&mut self) {
        self.end_drag();
    }
}
