use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::ui::component_handle::ComponentHandle;
use crate::ui::component_snapshot::ComponentConfigSnapshot;
use crate::ui::widget::WidgetId;

#[derive(Clone, Default)]
pub struct AppState {
    pub(crate) inner: Arc<Mutex<AppStateInner>>,
}

#[derive(Default)]
pub(crate) struct AppStateInner {
    components: HashMap<WidgetId, ComponentConfigSnapshot>,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn register(&self, id: WidgetId, snapshot: ComponentConfigSnapshot) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .register(id, snapshot);
    }

    pub(crate) fn unregister(&self, id: WidgetId) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .unregister(id);
    }

    pub fn get_handle(&self, id: WidgetId) -> Option<ComponentHandle> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(id)
            .then(|| ComponentHandle::from_app_state(id, Arc::downgrade(&self.inner)))
    }

    pub fn get_snapshot(&self, id: WidgetId) -> Option<ComponentConfigSnapshot> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot(id)
    }

    pub fn contains(&self, id: WidgetId) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(id)
    }

    pub fn len(&self) -> usize {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty()
    }
}

impl AppStateInner {
    fn register(&mut self, id: WidgetId, snapshot: ComponentConfigSnapshot) {
        self.components.insert(id, snapshot);
    }

    fn unregister(&mut self, id: WidgetId) {
        self.components.remove(&id);
    }

    pub(crate) fn snapshot(&self, id: WidgetId) -> Option<ComponentConfigSnapshot> {
        self.components.get(&id).cloned()
    }

    fn contains(&self, id: WidgetId) -> bool {
        self.components.contains_key(&id)
    }

    fn len(&self) -> usize {
        self.components.len()
    }

    fn is_empty(&self) -> bool {
        self.components.is_empty()
    }
}
