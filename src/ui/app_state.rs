use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;

use crate::core::{ComponentId, Rect};
use crate::draw::pipeline::{invalidate_paint_handle, InvalidationQueueHandle};
use crate::native::traits::event::EventLoopWaker;
use crate::ui::component_handle::ComponentHandle;
use crate::ui::component_snapshot::ComponentConfigSnapshot;
use crate::ui::event::SemanticEvent;

#[derive(Clone, Default)]
pub struct AppState {
    pub(crate) inner: Arc<Mutex<AppStateInner>>,
}

pub(crate) struct AppStateInner {
    owner_thread: ThreadId,
    components: HashMap<ComponentId, AppStateEntry>,
    semantic_events: VecDeque<(ComponentId, SemanticEvent)>,
    event_loop_waker: EventLoopWaker,
}

struct AppStateEntry {
    snapshot: ComponentConfigSnapshot,
    invalidation: InvalidationQueueHandle,
    rect: Option<Rect>,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn register(
        &self,
        id: ComponentId,
        snapshot: ComponentConfigSnapshot,
        invalidation: InvalidationQueueHandle,
        rect: Option<Rect>,
    ) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .register(id, snapshot, invalidation, rect);
    }

    pub(crate) fn unregister(&self, id: ComponentId) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .unregister(id);
    }

    pub fn get_handle(&self, id: ComponentId) -> Option<ComponentHandle> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(id)
            .then(|| ComponentHandle::from_app_state(id, Arc::downgrade(&self.inner)))
    }

    pub(crate) fn set_event_loop_waker(&self, waker: EventLoopWaker) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .set_event_loop_waker(waker);
    }

    pub(crate) fn drain_semantic_events(&self) -> Vec<(ComponentId, SemanticEvent)> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain_semantic_events()
    }

    pub(crate) fn has_semantic_events(&self) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .has_semantic_events()
    }

    pub fn get_snapshot(&self, id: ComponentId) -> Option<ComponentConfigSnapshot> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot(id)
    }

    pub fn contains(&self, id: ComponentId) -> bool {
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

impl Default for AppStateInner {
    fn default() -> Self {
        Self {
            owner_thread: std::thread::current().id(),
            components: HashMap::new(),
            semantic_events: VecDeque::new(),
            event_loop_waker: EventLoopWaker::default(),
        }
    }
}

impl AppStateInner {
    fn assert_owner_thread(&self) {
        assert!(
            std::thread::current().id() == self.owner_thread,
            "AppState and lookup ComponentHandle must be used on the UI thread; use AppHandle::post_to_ui from background threads"
        );
    }

    fn register(
        &mut self,
        id: ComponentId,
        snapshot: ComponentConfigSnapshot,
        invalidation: InvalidationQueueHandle,
        rect: Option<Rect>,
    ) {
        self.assert_owner_thread();
        self.components.insert(
            id,
            AppStateEntry {
                snapshot,
                invalidation,
                rect,
            },
        );
    }

    fn unregister(&mut self, id: ComponentId) {
        self.assert_owner_thread();
        self.components.remove(&id);
    }

    fn set_event_loop_waker(&mut self, waker: EventLoopWaker) {
        self.assert_owner_thread();
        self.event_loop_waker = waker;
    }

    pub(crate) fn snapshot(&self, id: ComponentId) -> Option<ComponentConfigSnapshot> {
        self.assert_owner_thread();
        self.components.get(&id).map(|entry| entry.snapshot.clone())
    }

    pub(crate) fn invalidate(&self, id: ComponentId) -> bool {
        self.assert_owner_thread();
        let Some(entry) = self.components.get(&id) else {
            return false;
        };
        invalidate_paint_handle(&entry.invalidation, id, entry.rect);
        true
    }

    pub(crate) fn emit_semantic_event(
        &mut self,
        id: ComponentId,
        event: SemanticEvent,
    ) -> Option<EventLoopWaker> {
        self.assert_owner_thread();
        if !self.contains(id) {
            return None;
        }
        self.semantic_events.push_back((id, event));
        Some(self.event_loop_waker.clone())
    }

    fn drain_semantic_events(&mut self) -> Vec<(ComponentId, SemanticEvent)> {
        self.assert_owner_thread();
        self.semantic_events.drain(..).collect()
    }

    fn has_semantic_events(&self) -> bool {
        self.assert_owner_thread();
        !self.semantic_events.is_empty()
    }

    fn contains(&self, id: ComponentId) -> bool {
        self.assert_owner_thread();
        self.components.contains_key(&id)
    }

    fn len(&self) -> usize {
        self.assert_owner_thread();
        self.components.len()
    }

    fn is_empty(&self) -> bool {
        self.assert_owner_thread();
        self.components.is_empty()
    }
}
