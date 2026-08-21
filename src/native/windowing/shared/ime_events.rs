use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::core::WindowId;
use crate::platform::windowing::event::UiEvent;

fn event_queue(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
) -> std::sync::MutexGuard<'_, VecDeque<UiEvent>> {
    events.lock().unwrap_or_else(|error| error.into_inner())
}

/// Tracks whether an IME composition session is in progress.
#[derive(Debug, Clone, Default)]
pub(crate) struct ImeCompositionState {
    pub(crate) active: bool,
}

/// Accumulates a double-buffered native IME update until its apply boundary.
#[derive(Debug, Default)]
#[allow(dead_code)]
pub(crate) struct PendingImeBatch {
    preedit: Option<String>,
    commit: Option<String>,
}

#[allow(dead_code)]
impl PendingImeBatch {
    pub(crate) fn set_preedit(&mut self, text: Option<String>) {
        self.preedit = Some(text.unwrap_or_default());
    }

    pub(crate) fn set_commit(&mut self, text: Option<String>) {
        self.commit = Some(text.unwrap_or_default());
    }

    pub(crate) fn apply_for_window(
        &mut self,
        events: &Arc<Mutex<VecDeque<UiEvent>>>,
        state: &mut ImeCompositionState,
        window_id: WindowId,
    ) {
        let batch = std::mem::take(self);
        match batch.commit.as_deref() {
            Some(text) if !text.is_empty() => {
                on_committed_text_for_window(events, state, text, window_id);
            }
            Some(_) => on_unmark_text_for_window(events, state, window_id),
            None if batch.preedit.is_none() => {
                on_unmark_text_for_window(events, state, window_id);
            }
            None => {}
        }

        if let Some(preedit) = batch.preedit {
            if preedit.is_empty() {
                on_unmark_text_for_window(events, state, window_id);
            } else {
                on_marked_text_for_window(events, state, &preedit, window_id);
            }
        }
    }

    // 在调用方已验证健康的事件队列 guard 中应用整批 IME 更新。
    pub(crate) fn apply_for_window_in_queue(
        // batch owner 在成功提交时被清空。
        &mut self,
        // 调用方持有唯一的健康事件队列访问。
        events: &mut VecDeque<UiEvent>,
        // composition 状态与事件在同一外部事务中更新。
        state: &mut ImeCompositionState,
        // 全部事件继续路由到原目标窗口。
        window_id: WindowId,
        // 窄端口不返回失败，因为 owner 健康性由调用方保证。
    ) {
        // 一次取走完整双缓冲 batch，保持 Done 边界语义。
        let batch = std::mem::take(self);
        // commit 事实先于本批次 preedit 应用。
        match batch.commit.as_deref() {
            // 非空 commit 结束 composition 并投递文本。
            Some(text) if !text.is_empty() => {
                // 复用同一健康队列，不执行二次锁。
                on_committed_text_targeted_in_queue(events, state, text, Some(window_id));
            }
            // 空 commit 只结束当前 composition。
            Some(_) => on_unmark_text_targeted_in_queue(events, state, Some(window_id)),
            // 没有任何 batch 事实时保持既有 unmark 语义。
            None if batch.preedit.is_none() => {
                // 复用同一健康队列，不执行二次锁。
                on_unmark_text_targeted_in_queue(events, state, Some(window_id));
            }
            // 只有 preedit 时留待下方处理。
            None => {}
        }

        // preedit 在 commit 事实之后应用。
        if let Some(preedit) = batch.preedit {
            // 空 preedit 表示 composition 结束。
            if preedit.is_empty() {
                // 复用同一健康队列，不执行二次锁。
                on_unmark_text_targeted_in_queue(events, state, Some(window_id));
            } else {
                // 非空 preedit 更新同一窗口的 marked text。
                on_marked_text_targeted_in_queue(events, state, &preedit, Some(window_id));
            }
        }
    }
}

// 仅 Windows TSF 路径（tsf_session.rs）使用；macOS/Wayland 使用 *_for_window 变体。
pub(crate) fn on_marked_text(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    state: &mut ImeCompositionState,
    text: &str,
) {
    on_marked_text_targeted(events, state, text, None);
}

pub(crate) fn on_marked_text_for_window(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    state: &mut ImeCompositionState,
    text: &str,
    window_id: WindowId,
) {
    on_marked_text_targeted(events, state, text, Some(window_id));
}

fn on_marked_text_targeted(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    state: &mut ImeCompositionState,
    text: &str,
    window_id: Option<WindowId>,
) {
    // 保持兼容入口在空文本时不触碰事件队列。
    if text.is_empty() {
        // 空 marked text 继续保持幂等。
        return;
    }
    // 兼容入口继续负责取得既有事件队列 owner。
    let mut queue = event_queue(events);
    // 统一委托不再加锁的队列 guard 窄端口。
    on_marked_text_targeted_in_queue(&mut queue, state, text, window_id);
}

// 在调用方已持有的队列 guard 中提交 marked-text 事件序列。
fn on_marked_text_targeted_in_queue(
    // 调用方持有唯一的健康事件队列访问。
    queue: &mut VecDeque<UiEvent>,
    // composition 状态与队列在同一事务中更新。
    state: &mut ImeCompositionState,
    // 本次 preedit 文本。
    text: &str,
    // 可选目标窗口保持兼容入口语义。
    window_id: Option<WindowId>,
    // 窄端口不加锁也不改变 owner。
) {
    // 空 marked text 不产生事件。
    if text.is_empty() {
        // 保持既有空输入幂等语义。
        return;
    }
    // 首次 preedit 先投递 composition start。
    if !state.active {
        // 事件继续经过统一窗口 target helper。
        queue.push_back(target(UiEvent::ime_composition_start(), window_id));
        // start 事件入队后提交 active 状态。
        state.active = true;
    }
    // 每次非空 preedit 都投递 composition update。
    queue.push_back(target(UiEvent::ime_composition_update(text), window_id));
}

// 仅 Windows TSF 路径（tsf_session.rs）使用；macOS/Wayland 使用 *_for_window 变体。
pub(crate) fn on_committed_text(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    state: &mut ImeCompositionState,
    text: &str,
) {
    on_committed_text_targeted(events, state, text, None);
}

pub(crate) fn on_committed_text_for_window(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    state: &mut ImeCompositionState,
    text: &str,
    window_id: WindowId,
) {
    on_committed_text_targeted(events, state, text, Some(window_id));
}

fn on_committed_text_targeted(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    state: &mut ImeCompositionState,
    text: &str,
    window_id: Option<WindowId>,
) {
    // 保持兼容入口在空文本时不触碰事件队列。
    if text.is_empty() {
        // 空 commit 继续保持幂等。
        return;
    }
    // 兼容入口继续负责取得既有事件队列 owner。
    let mut queue = event_queue(events);
    // 统一委托不再加锁的队列 guard 窄端口。
    on_committed_text_targeted_in_queue(&mut queue, state, text, window_id);
}

// 在调用方已持有的队列 guard 中提交 committed-text 事件序列。
fn on_committed_text_targeted_in_queue(
    // 调用方持有唯一的健康事件队列访问。
    queue: &mut VecDeque<UiEvent>,
    // composition 状态与队列在同一事务中更新。
    state: &mut ImeCompositionState,
    // 本次 committed 文本。
    text: &str,
    // 可选目标窗口保持兼容入口语义。
    window_id: Option<WindowId>,
    // 窄端口不加锁也不改变 owner。
) {
    // 空 commit 不产生文本事件。
    if text.is_empty() {
        // 保持既有空输入幂等语义。
        return;
    }
    // 活跃 composition 先以 committed 文本结束。
    if state.active {
        // composition end 保持先于 text-input。
        queue.push_back(target(UiEvent::ime_composition_end(text), window_id));
        // end 事件入队后提交 inactive 状态。
        state.active = false;
    }
    // committed 文本始终产生统一 text-input 事件。
    queue.push_back(target(UiEvent::text_input(text), window_id));
}

// 仅 Windows TSF 路径（tsf_session.rs）使用；macOS/Wayland 使用 *_for_window 变体。
pub(crate) fn on_unmark_text(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    state: &mut ImeCompositionState,
) {
    on_unmark_text_targeted(events, state, None);
}

pub(crate) fn on_unmark_text_for_window(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    state: &mut ImeCompositionState,
    window_id: WindowId,
) {
    on_unmark_text_targeted(events, state, Some(window_id));
}

// 在调用方已验证健康的事件队列 guard 中结束目标窗口 composition。
pub(crate) fn on_unmark_text_for_window_in_queue(
    // 调用方持有唯一的健康事件队列访问。
    events: &mut VecDeque<UiEvent>,
    // composition 状态与队列在同一事务中更新。
    state: &mut ImeCompositionState,
    // 结束事件继续路由到原目标窗口。
    window_id: WindowId,
    // 窄端口不加锁也不改变 owner。
) {
    // 委托统一的 queue-guard 实现。
    on_unmark_text_targeted_in_queue(events, state, Some(window_id));
}

fn on_unmark_text_targeted(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    state: &mut ImeCompositionState,
    window_id: Option<WindowId>,
) {
    // 保持兼容入口在 composition 未激活时不触碰事件队列。
    if !state.active {
        // 无活动 composition 继续保持幂等。
        return;
    }
    // 兼容入口继续负责取得既有事件队列 owner。
    let mut queue = event_queue(events);
    // 统一委托不再加锁的队列 guard 窄端口。
    on_unmark_text_targeted_in_queue(&mut queue, state, window_id);
}

// 在调用方已持有的队列 guard 中提交 composition-end。
fn on_unmark_text_targeted_in_queue(
    // 调用方持有唯一的健康事件队列访问。
    queue: &mut VecDeque<UiEvent>,
    // composition 状态与队列在同一事务中更新。
    state: &mut ImeCompositionState,
    // 可选目标窗口保持兼容入口语义。
    window_id: Option<WindowId>,
    // 窄端口不加锁也不改变 owner。
) {
    // 没有活跃 composition 时保持幂等。
    if !state.active {
        // 不产生虚假的 end 事件。
        return;
    }
    // active composition 只投递一次空文本 end。
    queue.push_back(target(UiEvent::ime_composition_end(""), window_id));
    // end 事件入队后提交 inactive 状态。
    state.active = false;
}

fn target(event: UiEvent, window_id: Option<WindowId>) -> UiEvent {
    match window_id {
        Some(window_id) => event.for_window(window_id),
        None => event,
    }
}
