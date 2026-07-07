use crate::core::{Point, Rect};
use crate::ui::ComponentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    Modal,
    Drawer,
    Popover,
    Tooltip,
    ContextMenu,
    Message,
    Notification,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OverlayId(u64);

impl OverlayId {
    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct OverlayEntry {
    id: OverlayId,
    owner: ComponentId,
    kind: OverlayKind,
    bounds: Option<Rect>,
    z_index: i32,
    modal: bool,
    dismiss_on_outside: bool,
    focus_trap: bool,
    managed: bool,
}

impl OverlayEntry {
    pub fn new(owner: ComponentId, kind: OverlayKind) -> Self {
        Self {
            id: OverlayId(0),
            owner,
            kind,
            bounds: None,
            z_index: 0,
            modal: matches!(kind, OverlayKind::Modal | OverlayKind::Drawer),
            dismiss_on_outside: matches!(kind, OverlayKind::Modal | OverlayKind::Drawer),
            focus_trap: matches!(kind, OverlayKind::Modal | OverlayKind::Drawer),
            managed: false,
        }
    }

    pub fn bounds(mut self, bounds: Rect) -> Self {
        self.bounds = Some(bounds);
        self
    }

    pub fn z_index(mut self, z_index: i32) -> Self {
        self.z_index = z_index;
        self
    }

    pub fn modal(mut self, modal: bool) -> Self {
        self.modal = modal;
        self
    }

    pub fn dismiss_on_outside(mut self, dismiss: bool) -> Self {
        self.dismiss_on_outside = dismiss;
        self
    }

    pub fn focus_trap(mut self, focus_trap: bool) -> Self {
        self.focus_trap = focus_trap;
        self
    }

    pub fn managed(mut self, managed: bool) -> Self {
        self.managed = managed;
        self
    }

    pub fn id(&self) -> OverlayId {
        self.id
    }

    pub fn owner(&self) -> ComponentId {
        self.owner
    }

    pub fn kind(&self) -> OverlayKind {
        self.kind
    }

    pub fn bounds_rect(&self) -> Option<Rect> {
        self.bounds
    }

    pub fn z_index_value(&self) -> i32 {
        self.z_index
    }

    pub fn is_modal(&self) -> bool {
        self.modal
    }

    pub fn dismisses_on_outside(&self) -> bool {
        self.dismiss_on_outside
    }

    pub fn traps_focus(&self) -> bool {
        self.focus_trap
    }

    pub fn is_managed(&self) -> bool {
        self.managed
    }
}

#[derive(Debug, Default)]
pub struct OverlayStack {
    next_id: u64,
    entries: Vec<OverlayEntry>,
}

impl OverlayStack {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, owner: ComponentId, kind: OverlayKind) -> OverlayId {
        self.push_entry(OverlayEntry::new(owner, kind))
    }

    pub fn push_entry(&mut self, mut entry: OverlayEntry) -> OverlayId {
        self.next_id += 1;
        let id = OverlayId(self.next_id);
        entry.id = id;
        self.entries.push(entry);
        self.entries.sort_by_key(|entry| entry.z_index);
        id
    }

    pub fn remove(&mut self, id: OverlayId) -> Option<OverlayEntry> {
        let index = self.entries.iter().position(|entry| entry.id == id)?;
        Some(self.entries.remove(index))
    }

    pub fn remove_for_owner(&mut self, owner: ComponentId) -> Vec<OverlayEntry> {
        let mut removed = Vec::new();
        self.entries.retain(|entry| {
            if entry.owner == owner {
                removed.push(entry.clone());
                false
            } else {
                true
            }
        });
        removed
    }

    pub fn retain_entries(&mut self, mut keep: impl FnMut(&OverlayEntry) -> bool) {
        self.entries.retain(|entry| keep(entry));
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &OverlayEntry> {
        self.entries.iter()
    }

    pub fn top(&self) -> Option<&OverlayEntry> {
        self.entries.last()
    }

    pub fn hit_test(&self, x: f32, y: f32) -> Option<&OverlayEntry> {
        self.entries.iter().rev().find(|entry| {
            entry
                .bounds
                .is_some_and(|bounds| bounds.contains(Point::new(x, y)))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ComponentId;

    #[test]
    fn same_z_index_keeps_latest_entry_on_top() {
        let mut stack = OverlayStack::new();
        let first = ComponentId::new(1);
        let second = ComponentId::new(2);

        stack.push_entry(OverlayEntry::new(first, OverlayKind::Popover).z_index(10));
        stack.push_entry(OverlayEntry::new(second, OverlayKind::Tooltip).z_index(10));

        assert_eq!(stack.top().map(|entry| entry.owner()), Some(second));
    }

    #[test]
    fn remove_for_owner_returns_removed_entries() {
        let mut stack = OverlayStack::new();
        let owner = ComponentId::new(1);
        let other = ComponentId::new(2);

        stack.push_entry(OverlayEntry::new(owner, OverlayKind::Modal).z_index(10));
        stack.push_entry(OverlayEntry::new(other, OverlayKind::Tooltip).z_index(20));

        let removed = stack.remove_for_owner(owner);

        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].owner(), owner);
        assert_eq!(stack.len(), 1);
        assert_eq!(stack.top().map(|entry| entry.owner()), Some(other));
    }

    #[test]
    fn hit_test_returns_topmost_entry_at_point() {
        let mut stack = OverlayStack::new();
        let lower = ComponentId::new(1);
        let upper = ComponentId::new(2);

        stack.push_entry(
            OverlayEntry::new(lower, OverlayKind::Popover)
                .bounds(Rect::new(0.0, 0.0, 100.0, 100.0))
                .z_index(10),
        );
        stack.push_entry(
            OverlayEntry::new(upper, OverlayKind::Tooltip)
                .bounds(Rect::new(20.0, 20.0, 100.0, 100.0))
                .z_index(20),
        );

        assert_eq!(
            stack.hit_test(30.0, 30.0).map(|entry| entry.owner()),
            Some(upper)
        );
    }

    #[test]
    fn hit_test_ignores_entries_without_bounds() {
        let mut stack = OverlayStack::new();
        let unbounded = ComponentId::new(1);
        let bounded = ComponentId::new(2);

        stack.push_entry(OverlayEntry::new(unbounded, OverlayKind::Popover).z_index(20));
        stack.push_entry(
            OverlayEntry::new(bounded, OverlayKind::Tooltip)
                .bounds(Rect::new(0.0, 0.0, 50.0, 50.0))
                .z_index(10),
        );

        assert_eq!(
            stack.hit_test(10.0, 10.0).map(|entry| entry.owner()),
            Some(bounded)
        );
        assert!(stack.hit_test(80.0, 80.0).is_none());
    }
}
