use crate::core::ComponentId;
use crate::core::{Point, Rect};

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

pub(crate) mod placement;
pub use placement::Placement;
