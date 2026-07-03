use uix_platform::Point;

/// Result of a drag target operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragEventResult {
    Accepted,
    Rejected,
    Ignored,
}

/// Manages drag-and-drop for a widget.
#[derive(Default)]
pub struct DragManager {
    dragging: bool,
    drag_start_pos: Point,
    drag_offset: Point,
}

impl DragManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_drag(&mut self, pos: Point) {
        self.dragging = true;
        self.drag_start_pos = pos;
        self.drag_offset = Point::zero();
    }

    pub fn update_drag(&mut self, pos: Point) {
        if self.dragging {
            self.drag_offset =
                Point::new(pos.x - self.drag_start_pos.x, pos.y - self.drag_start_pos.y);
        }
    }

    pub fn end_drag(&mut self) {
        self.dragging = false;
        self.drag_offset = Point::zero();
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging
    }
    pub fn drag_offset(&self) -> Point {
        self.drag_offset
    }
}
