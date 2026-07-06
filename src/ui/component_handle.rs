use std::cell::RefCell;
use std::rc::{Rc, Weak as RcWeak};
use std::sync::{Mutex, Weak as SyncWeak};

use crate::ui::app_state::AppStateInner;
use crate::ui::component_snapshot::{ComponentConfigSnapshot, SnapshotFields};
use crate::ui::event::SemanticEvent;
use crate::ui::widget::{EventResult, WidgetId, WidgetTree};

#[derive(Clone)]
pub struct ComponentHandle {
    id: WidgetId,
    tree: Option<RcWeak<RefCell<WidgetTree>>>,
    app_state: Option<SyncWeak<Mutex<AppStateInner>>>,
}

impl ComponentHandle {
    pub fn new(id: WidgetId, tree: &Rc<RefCell<WidgetTree>>) -> Self {
        Self {
            id,
            tree: Some(Rc::downgrade(tree)),
            app_state: None,
        }
    }

    pub(crate) fn from_app_state(id: WidgetId, app_state: SyncWeak<Mutex<AppStateInner>>) -> Self {
        Self {
            id,
            tree: None,
            app_state: Some(app_state),
        }
    }

    pub fn id(&self) -> WidgetId {
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
                    return Some(ComponentConfigSnapshot::from_component(
                        self.id,
                        node.component(),
                    ));
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

    pub fn text(&self) -> Option<String> {
        match self.snapshot_fields()? {
            SnapshotFields::Button { text, .. } | SnapshotFields::Label { text, .. } => Some(text),
            _ => None,
        }
    }

    pub fn placeholder(&self) -> Option<String> {
        match self.snapshot_fields()? {
            SnapshotFields::Input { placeholder, .. } => Some(placeholder),
            _ => None,
        }
    }

    pub fn disabled(&self) -> Option<bool> {
        match self.snapshot_fields()? {
            SnapshotFields::Button { disabled, .. } | SnapshotFields::Input { disabled, .. } => {
                Some(disabled)
            }
            _ => None,
        }
    }

    pub fn invalidate(&self) {
        let Some(tree) = self.tree.as_ref().and_then(RcWeak::upgrade) else {
            return;
        };
        if let Ok(mut tree) = tree.try_borrow_mut() {
            tree.invalidate_paint(self.id);
        };
    }

    pub fn emit(&self, mut event: SemanticEvent) -> EventResult {
        let Some(tree) = self.tree.as_ref().and_then(RcWeak::upgrade) else {
            return EventResult::NotHandled;
        };
        let Ok(tree_ref) = tree.try_borrow() else {
            return EventResult::NotHandled;
        };
        if tree_ref.get(self.id).is_none() {
            return EventResult::NotHandled;
        }
        drop(tree_ref);

        event.target = self.id;
        event.current_target = self.id;
        let result = match tree.try_borrow_mut() {
            Ok(mut tree) => tree.dispatch_semantic(event),
            Err(_) => EventResult::NotHandled,
        };
        result
    }
}
