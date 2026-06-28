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

use crate::event::{UiEvent, UiEventType};
use std::cell::RefCell;

/// 事件处理函数签名：接收事件引用，返回 false 表示请求退出事件循环。
pub type EventHandler = Box<dyn FnMut(&UiEvent) -> bool>;

struct SubscriberEntry {
    id: usize,
    /// None = 订阅所有事件，Some(type) = 仅订阅特定类型
    event_type: Option<UiEventType>,
    handler: EventHandler,
}

/// 事件总线——线程局部（per-Platform），支持订阅/取消订阅/发布。
///
/// 内部使用 RefCell 实现内部可变性：
/// - subscribe/unsubscribe 需要 &mut
/// - publish 需要 &（RefCell 提供运行时借用检查）
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

    /// 订阅特定类型的事件。
    ///
    /// 返回 subscription ID（用于取消订阅）。
    pub fn subscribe<F>(&mut self, event_type: UiEventType, handler: F) -> usize
    where
        F: FnMut(&UiEvent) -> bool + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.subscribers.borrow_mut().push(SubscriberEntry {
            id,
            event_type: Some(event_type),
            handler: Box::new(handler),
        });
        id
    }

    /// 订阅所有事件（通配符订阅）。
    ///
    /// 返回 subscription ID（用于取消订阅）。
    pub fn subscribe_all<F>(&mut self, handler: F) -> usize
    where
        F: FnMut(&UiEvent) -> bool + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.subscribers.borrow_mut().push(SubscriberEntry {
            id,
            event_type: None,
            handler: Box::new(handler),
        });
        id
    }

    /// 取消订阅。
    pub fn unsubscribe(&mut self, id: usize) {
        self.subscribers.borrow_mut().retain(|e| e.id != id);
    }

    /// 发布事件到所有匹配的订阅者。
    ///
    /// 按注册顺序依次调用匹配的 handler。
    /// 任意 handler 返回 false 则立即终止并返回 false。
    pub fn publish(&self, event: &UiEvent) -> bool {
        let mut subs = self.subscribers.borrow_mut();
        for entry in subs.iter_mut() {
            let matches = match entry.event_type {
                Some(et) => et == event.type_,
                None => true, // 通配符订阅匹配所有
            };
            if matches {
                if !(entry.handler)(event) {
                    return false;
                }
            }
        }
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
