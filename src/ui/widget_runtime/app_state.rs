use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;

use crate::core::{Rect, WidgetId};
use crate::draw::renderer::{InvalidationQueueHandle, invalidate_paint_handle};
use crate::platform::windowing::event::EventLoopWaker;
use crate::ui::event::SemanticEvent;
use crate::ui::widget_runtime::widget_handle::WidgetHandle;
use crate::ui::widget_snapshot::WidgetConfigSnapshot;

#[derive(Clone, Default)]
/// 可克隆的应用级组件快照与受控句柄注册表。
///
/// 组件登记和查询必须在所属 UI 线程执行；后台操作应通过 `AppHandle` 投递。
pub struct AppState {
    pub(crate) inner: Arc<Mutex<AppStateInner>>,
}

pub(crate) struct AppStateInner {
    owner_thread: ThreadId,
    widgets: HashMap<WidgetId, AppStateEntry>,
    semantic_events: VecDeque<(WidgetId, SemanticEvent)>,
    focus_requests: VecDeque<(WidgetId, FocusRequest)>,
    event_loop_waker: EventLoopWaker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusRequest {
    Focus,
    FocusAndReveal,
    Blur,
}

struct AppStateEntry {
    snapshot: WidgetConfigSnapshot,
    invalidation: InvalidationQueueHandle,
    rect: Option<Rect>,
}

impl AppState {
    /// 创建绑定到当前线程且尚未登记组件的状态注册表。
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn register(
        &self,
        id: WidgetId,
        snapshot: WidgetConfigSnapshot,
        invalidation: InvalidationQueueHandle,
        rect: Option<Rect>,
    ) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .register(id, snapshot, invalidation, rect);
    }

    pub(crate) fn unregister(&self, id: WidgetId) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .unregister(id);
    }

    /// 返回当前仍登记的组件句柄。
    pub fn get_handle(&self, id: WidgetId) -> Option<WidgetHandle> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(id)
            .then(|| WidgetHandle::from_app_state(id, Arc::downgrade(&self.inner)))
    }

    pub(crate) fn set_event_loop_waker(&self, waker: EventLoopWaker) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .set_event_loop_waker(waker);
    }

    pub(crate) fn drain_semantic_events_for_scope_into(
        &self,
        tree_scope: u64,
        matched: &mut Vec<(WidgetId, SemanticEvent)>,
    ) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain_semantic_events_for_scope_into(tree_scope, matched);
    }

    pub(crate) fn has_semantic_events_for_scope(&self, tree_scope: u64) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .has_semantic_events_for_scope(tree_scope)
    }

    pub(crate) fn drain_focus_requests_for_scope_into(
        &self,
        tree_scope: u64,
        matched: &mut Vec<(WidgetId, FocusRequest)>,
    ) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain_focus_requests_for_scope_into(tree_scope, matched);
    }

    pub(crate) fn has_focus_requests_for_scope(&self, tree_scope: u64) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .has_focus_requests_for_scope(tree_scope)
    }

    /// 返回指定组件当前配置快照的 owned 副本。
    pub fn get_snapshot(&self, id: WidgetId) -> Option<WidgetConfigSnapshot> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot(id)
    }

    /// 判断指定组件身份当前是否仍登记。
    pub fn contains(&self, id: WidgetId) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(id)
    }

    /// 返回当前登记的组件数量。
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// 判断当前是否没有登记任何组件。
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
            widgets: HashMap::new(),
            semantic_events: VecDeque::new(),
            focus_requests: VecDeque::new(),
            event_loop_waker: EventLoopWaker::default(),
        }
    }
}

impl AppStateInner {
    fn assert_owner_thread(&self) {
        assert!(
            std::thread::current().id() == self.owner_thread,
            "AppState and lookup WidgetHandle must be used on the UI thread; use AppHandle::post_to_ui from background threads"
        );
    }

    fn register(
        &mut self,
        id: WidgetId,
        snapshot: WidgetConfigSnapshot,
        invalidation: InvalidationQueueHandle,
        rect: Option<Rect>,
    ) {
        self.assert_owner_thread();
        self.widgets.insert(
            id,
            AppStateEntry {
                snapshot,
                invalidation,
                rect,
            },
        );
    }

    fn unregister(&mut self, id: WidgetId) {
        self.assert_owner_thread();
        self.widgets.remove(&id);
        self.semantic_events.retain(|(target, _)| *target != id);
        self.focus_requests.retain(|(target, _)| *target != id);
    }

    fn set_event_loop_waker(&mut self, waker: EventLoopWaker) {
        self.assert_owner_thread();
        self.event_loop_waker = waker;
    }

    pub(crate) fn snapshot(&self, id: WidgetId) -> Option<WidgetConfigSnapshot> {
        self.map_snapshot(id, Clone::clone)
    }

    pub(crate) fn map_snapshot<R>(
        &self,
        id: WidgetId,
        map: impl FnOnce(&WidgetConfigSnapshot) -> R,
    ) -> Option<R> {
        self.widgets.get(&id).map(|entry| map(&entry.snapshot))
    }

    pub(crate) fn invalidate(&self, id: WidgetId) -> Option<EventLoopWaker> {
        let entry = self.widgets.get(&id)?;
        invalidate_paint_handle(&entry.invalidation, id, entry.rect);
        Some(self.event_loop_waker.clone())
    }

    pub(crate) fn emit_semantic_event(
        &mut self,
        id: WidgetId,
        event: SemanticEvent,
    ) -> Option<EventLoopWaker> {
        if !self.contains(id) {
            return None;
        }
        self.semantic_events.push_back((id, event));
        Some(self.event_loop_waker.clone())
    }

    pub(crate) fn enqueue_focus_request(
        &mut self,
        id: WidgetId,
        request: FocusRequest,
    ) -> Option<EventLoopWaker> {
        if !self.widgets.contains_key(&id) {
            return None;
        }
        self.focus_requests.push_back((id, request));
        Some(self.event_loop_waker.clone())
    }

    fn drain_semantic_events_for_scope_into(
        &mut self,
        tree_scope: u64,
        matched: &mut Vec<(WidgetId, SemanticEvent)>,
    ) {
        self.assert_owner_thread();
        matched.clear();
        let pending = self.semantic_events.len();
        for _ in 0..pending {
            let Some((id, event)) = self.semantic_events.pop_front() else {
                break;
            };
            if id.tree_scope() == tree_scope {
                matched.push((id, event));
            } else {
                self.semantic_events.push_back((id, event));
            }
        }
    }

    fn has_semantic_events_for_scope(&self, tree_scope: u64) -> bool {
        self.assert_owner_thread();
        self.semantic_events
            .iter()
            .any(|(id, _)| id.tree_scope() == tree_scope)
    }

    fn drain_focus_requests_for_scope_into(
        &mut self,
        tree_scope: u64,
        matched: &mut Vec<(WidgetId, FocusRequest)>,
    ) {
        self.assert_owner_thread();
        matched.clear();
        let pending = self.focus_requests.len();
        for _ in 0..pending {
            let Some((id, request)) = self.focus_requests.pop_front() else {
                break;
            };
            if id.tree_scope() == tree_scope {
                matched.push((id, request));
            } else {
                self.focus_requests.push_back((id, request));
            }
        }
    }

    fn has_focus_requests_for_scope(&self, tree_scope: u64) -> bool {
        self.assert_owner_thread();
        self.focus_requests
            .iter()
            .any(|(id, _)| id.tree_scope() == tree_scope)
    }

    pub(crate) fn contains(&self, id: WidgetId) -> bool {
        self.widgets.contains_key(&id)
    }

    fn len(&self) -> usize {
        self.widgets.len()
    }

    fn is_empty(&self) -> bool {
        self.widgets.is_empty()
    }
}

// cfg(test) 完整辅助实现位于 tests-src，仅测试构建编译。
#[cfg(test)]
#[path = "../../../tests-src/ui/widget_runtime/app_state_tests.rs"]
mod app_state_tests;