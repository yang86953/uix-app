use std::cell::RefCell;
use std::rc::{Rc, Weak as RcWeak};
use std::sync::{Mutex, Weak as SyncWeak};

use crate::core::ComponentId;
use crate::ui::app_state::AppStateInner;
use crate::ui::component_snapshot::{
    AccessibilitySnapshot, AriaAttribute, ComponentConfigSnapshot, SnapshotFields,
};
use crate::ui::event::SemanticEvent;
use crate::ui::widget::{EventResult, WidgetTree};

#[derive(Clone)]
pub struct ComponentHandle {
    id: ComponentId,
    tree: Option<RcWeak<RefCell<WidgetTree>>>,
    app_state: Option<SyncWeak<Mutex<AppStateInner>>>,
}

impl ComponentHandle {
    pub fn new(id: ComponentId, tree: &Rc<RefCell<WidgetTree>>) -> Self {
        Self {
            id,
            tree: Some(Rc::downgrade(tree)),
            app_state: None,
        }
    }

    pub(crate) fn from_app_state(
        id: ComponentId,
        app_state: SyncWeak<Mutex<AppStateInner>>,
    ) -> Self {
        Self {
            id,
            tree: None,
            app_state: Some(app_state),
        }
    }

    pub fn id(&self) -> ComponentId {
        self.id
    }

    pub fn is_alive(&self) -> bool {
        if let Some(tree) = self.tree.as_ref().and_then(RcWeak::upgrade) {
            if tree
                .try_borrow()
                .is_ok_and(|tree| tree.get(self.id).is_some())
            {
                return true;
            }
        }
        self.app_state
            .as_ref()
            .and_then(SyncWeak::upgrade)
            .is_some_and(|state| {
                state
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .snapshot(self.id)
                    .is_some()
            })
    }

    pub fn snapshot(&self) -> Option<ComponentConfigSnapshot> {
        if let Some(tree) = self.tree.as_ref().and_then(RcWeak::upgrade) {
            if let Ok(tree) = tree.try_borrow() {
                if let Some(node) = tree.get(self.id) {
                    return Some(node.component_snapshot(self.id));
                }
            }
        }

        self.app_state
            .as_ref()
            .and_then(SyncWeak::upgrade)
            .and_then(|state| {
                state
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .snapshot(self.id)
            })
    }

    pub fn snapshot_fields(&self) -> Option<SnapshotFields> {
        self.snapshot().map(|snapshot| snapshot.fields)
    }

    pub fn accessibility(&self) -> Option<AccessibilitySnapshot> {
        self.snapshot().map(|snapshot| snapshot.accessibility())
    }

    pub fn aria_role(&self) -> Option<&'static str> {
        self.accessibility()?.aria_role()
    }

    pub fn aria_attributes(&self) -> Option<Vec<AriaAttribute>> {
        self.accessibility()
            .map(|accessibility| accessibility.aria_attributes())
    }

    pub fn text(&self) -> Option<String> {
        match self.snapshot_fields()? {
            SnapshotFields::Button { text, .. } | SnapshotFields::Label { text, .. } => Some(text),
            _ => None,
        }
    }

    pub fn label(&self) -> Option<String> {
        self.text()
    }

    pub fn placeholder(&self) -> Option<String> {
        match self.snapshot_fields()? {
            SnapshotFields::Input { placeholder, .. }
            | SnapshotFields::InputNumber { placeholder, .. }
            | SnapshotFields::Select { placeholder, .. }
            | SnapshotFields::AutoComplete { placeholder, .. }
            | SnapshotFields::TreeSelect { placeholder, .. }
            | SnapshotFields::Cascader { placeholder, .. }
            | SnapshotFields::DatePicker { placeholder, .. }
            | SnapshotFields::DateRangePicker { placeholder, .. }
            | SnapshotFields::TimePicker { placeholder, .. }
            | SnapshotFields::Mentions { placeholder, .. } => Some(placeholder),
            _ => None,
        }
    }

    pub fn disabled(&self) -> Option<bool> {
        match self.snapshot_fields()? {
            SnapshotFields::Button { disabled, .. }
            | SnapshotFields::Input { disabled, .. }
            | SnapshotFields::Checkbox { disabled, .. }
            | SnapshotFields::Radio { disabled, .. }
            | SnapshotFields::Switch { disabled, .. }
            | SnapshotFields::Rate { disabled, .. }
            | SnapshotFields::InputNumber { disabled, .. }
            | SnapshotFields::Select { disabled, .. }
            | SnapshotFields::Segmented { disabled, .. }
            | SnapshotFields::Typography { disabled, .. } => Some(disabled),
            _ => None,
        }
    }

    pub fn checked(&self) -> Option<bool> {
        match self.snapshot_fields()? {
            SnapshotFields::Checkbox { checked, .. } | SnapshotFields::Switch { checked, .. } => {
                Some(checked)
            }
            _ => None,
        }
    }

    pub fn numeric_value(&self) -> Option<f64> {
        match self.snapshot_fields()? {
            SnapshotFields::Slider { value, .. } => Some(value),
            SnapshotFields::ProgressBar {
                progress: value, ..
            } => Some(value as f64),
            SnapshotFields::Rate { value, .. } => Some(value as f64),
            SnapshotFields::InputNumber { value, .. } => Some(value),
            _ => None,
        }
    }

    pub fn invalidate(&self) {
        if let Some(tree) = self.tree.as_ref().and_then(RcWeak::upgrade) {
            if let Ok(mut tree) = tree.try_borrow_mut() {
                tree.invalidate_paint(self.id);
            };
            return;
        }

        if let Some(state) = self.app_state.as_ref().and_then(SyncWeak::upgrade) {
            let _ = state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .invalidate(self.id);
        }
    }

    pub fn emit(&self, mut event: SemanticEvent) -> EventResult {
        if let Some(tree) = self.tree.as_ref().and_then(RcWeak::upgrade) {
            let Ok(tree_ref) = tree.try_borrow() else {
                return EventResult::NotHandled;
            };
            if tree_ref.get(self.id).is_none() {
                return EventResult::NotHandled;
            }
            drop(tree_ref);

            event.target = self.id;
            event.current_target = self.id;
            return match tree.try_borrow_mut() {
                Ok(mut tree) => tree.dispatch_semantic(event),
                Err(_) => EventResult::NotHandled,
            };
        }

        if let Some(state) = self.app_state.as_ref().and_then(SyncWeak::upgrade) {
            event.target = self.id;
            event.current_target = self.id;
            let waker = state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .emit_semantic_event(self.id, event);
            if let Some(waker) = waker {
                waker.wake();
                return EventResult::Handled;
            }
        }

        EventResult::NotHandled
    }
}
