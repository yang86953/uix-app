// ============================================================================
// platform/event_bus.rs — 事件订阅/发布总线
//
// 设计意图：平台层负责事件收集与分发，其他层通过订阅机制接收事件。
// 取代原来的 callback 链式传递模式（collect closure），
// 改用注册式订阅。
//
// 使用方式：
//   let sub_id = bus.subscribe(UiEventType::MouseMove, |ev| {
//       // 处理事件，返回 true 继续，false 退出事件循环
//       true
//   });
//   bus.unsubscribe(sub_id); // 取消订阅
// ============================================================================

use super::types::{UiEvent, UiEventType};
use std::cell::RefCell;

/// 事件处理函数签名：接收事件引用，返回 false 表示请求退出事件循环。
pub type EventHandler = Box<dyn FnMut(&UiEvent) -> bool>;

/// 默认优先级（中间值）。
pub const PRIORITY_DEFAULT: i32 = 0;
/// 最高优先级（最先执行）。
pub const PRIORITY_HIGHEST: i32 = i32::MAX;
/// 最低优先级（最后执行）。
pub const PRIORITY_LOWEST: i32 = i32::MIN;

struct SubscriberEntry {
    id: usize,
    /// None = 订阅所有事件，Some(type) = 仅订阅特定类型
    event_type: Option<UiEventType>,
    handler: EventHandler,
    /// 优先级：数值越高越先执行。相同优先级按注册顺序。
    priority: i32,
    /// 一次性标记：触发一次后自动取消订阅。
    once: bool,
}

/// 事件总线——线程局部（per-Platform），支持订阅/取消订阅/发布。
///
/// 内部使用 RefCell 实现内部可变性：
/// - subscribe/unsubscribe 需要 &mut
/// - publish 需要 &（RefCell 提供运行时借用检查）
///
/// # 优先级
///
/// 高优先级订阅者先收到事件。相同优先级的订阅者按注册顺序执行。
/// 使用 `subscribe_with_priority` 指定优先级，默认 `PRIORITY_DEFAULT` (0)。
///
/// # 一次性订阅
///
/// `subscribe_once` / `subscribe_all_once` 注册的 handler 触发一次后自动取消。
/// 适合「等待窗口出现」或「等待特定按键」等一次性场景。
pub struct EventBus {
    subscribers: RefCell<Vec<SubscriberEntry>>,
    next_id: usize,
}

impl EventBus {
    /// 创建新事件总线。
    pub fn new() -> Self {
        Self {
            subscribers: RefCell::new(Vec::new()),
            next_id: 1,
        }
    }

    // ── 底层订阅方法 ───────────────────────────────────────────

    fn add_entry(
        &mut self,
        event_type: Option<UiEventType>,
        priority: i32,
        once: bool,
        handler: EventHandler,
    ) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.subscribers.borrow_mut().push(SubscriberEntry {
            id,
            event_type,
            handler,
            priority,
            once,
        });
        id
    }

    // ── 标准订阅 ───────────────────────────────────────────────

    /// 订阅特定类型的事件。
    ///
    /// 返回 subscription ID（用于取消订阅）。
    pub fn subscribe<F>(&mut self, event_type: UiEventType, handler: F) -> usize
    where
        F: FnMut(&UiEvent) -> bool + 'static,
    {
        self.add_entry(Some(event_type), PRIORITY_DEFAULT, false, Box::new(handler))
    }

    /// 订阅所有事件（通配符订阅）。
    ///
    /// 返回 subscription ID（用于取消订阅）。
    pub fn subscribe_all<F>(&mut self, handler: F) -> usize
    where
        F: FnMut(&UiEvent) -> bool + 'static,
    {
        self.add_entry(None, PRIORITY_DEFAULT, false, Box::new(handler))
    }

    // ── 优先级订阅 ─────────────────────────────────────────────

    /// 按优先级订阅特定类型的事件。
    ///
    /// `priority` 越高越先执行。相同优先级按注册顺序。
    pub fn subscribe_with_priority<F>(
        &mut self,
        event_type: UiEventType,
        priority: i32,
        handler: F,
    ) -> usize
    where
        F: FnMut(&UiEvent) -> bool + 'static,
    {
        self.add_entry(Some(event_type), priority, false, Box::new(handler))
    }

    /// 按优先级订阅所有事件。
    pub fn subscribe_all_with_priority<F>(&mut self, priority: i32, handler: F) -> usize
    where
        F: FnMut(&UiEvent) -> bool + 'static,
    {
        self.add_entry(None, priority, false, Box::new(handler))
    }

    // ── 一次性订阅 ─────────────────────────────────────────────

    /// 一次性订阅特定类型的事件：触发一次后自动取消订阅。
    pub fn subscribe_once<F>(&mut self, event_type: UiEventType, handler: F) -> usize
    where
        F: FnMut(&UiEvent) -> bool + 'static,
    {
        self.add_entry(Some(event_type), PRIORITY_DEFAULT, true, Box::new(handler))
    }

    /// 一次性订阅所有事件：触发一次后自动取消订阅。
    pub fn subscribe_all_once<F>(&mut self, handler: F) -> usize
    where
        F: FnMut(&UiEvent) -> bool + 'static,
    {
        self.add_entry(None, PRIORITY_DEFAULT, true, Box::new(handler))
    }

    /// 一次性 + 优先级订阅特定类型的事件。
    pub fn subscribe_once_with_priority<F>(
        &mut self,
        event_type: UiEventType,
        priority: i32,
        handler: F,
    ) -> usize
    where
        F: FnMut(&UiEvent) -> bool + 'static,
    {
        self.add_entry(Some(event_type), priority, true, Box::new(handler))
    }

    // ── 取消订阅 ───────────────────────────────────────────────

    /// 取消订阅。
    pub fn unsubscribe(&mut self, id: usize) {
        self.subscribers.borrow_mut().retain(|e| e.id != id);
    }

    // ── 发布事件 ───────────────────────────────────────────────

    /// 发布事件到所有匹配的订阅者。
    ///
    /// 按优先级降序（高→低）依次调用匹配的 handler，
    /// 相同优先级按注册顺序。
    /// 任意 handler 返回 false 则立即终止并返回 false。
    /// 发布后自动清理已触发的一次性订阅。
    pub fn publish(&self, event: &UiEvent) -> bool {
        let mut subs = self.subscribers.borrow_mut();

        // 按优先级降序排序（高优先级先执行）
        // 相同优先级的排序是稳定的，保持注册顺序
        subs.sort_by_key(|b| std::cmp::Reverse(b.priority));

        for entry in subs.iter_mut() {
            let matches = match entry.event_type {
                Some(et) => et == event.type_,
                None => true, // 通配符订阅匹配所有
            };
            if matches {
                if !(entry.handler)(event) {
                    return false;
                }
                // 一次性订阅：触发后标记为待删除（id=0，因为有效 id 从 1 开始）
                if entry.once {
                    entry.id = 0;
                }
            }
        }

        // 清理已触发的一次性订阅（id=0 的条目）
        subs.retain(|e| e.id != 0);
        true
    }

    /// 清空所有订阅。
    pub fn clear(&mut self) {
        self.subscribers.borrow_mut().clear();
    }

    /// 当前订阅者数量。
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.borrow().len()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
