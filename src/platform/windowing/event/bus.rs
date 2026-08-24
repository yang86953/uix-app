// ============================================================================
// platform/windowing/event/bus.rs — 平台输入事件分发总线
//
// 设计意图：platform System 私有边界（windowing Module）的输入事件分发
// 通道，取代旧 callback 链式传递模式（collect closure），改用注册式订阅。
//
// 定位与边界（SMC 治理，2026-08-03）：
// - 本总线只做「输入事件分发」，不是通用 SMC 的「事实广播 EventBus」；
//   `-> bool` 返回语义（请求退出事件循环）已移除——事件总线不携带业务
//   应答（通用 SMC I-15 / I-24），退出请求经直接契约流转；
// - 当前无业务订阅者：UIX 事件路径经 map_event → tree.dispatch_event
//   直接分发；本通道作为 platform 私有预留分发点保留，发布为 no-op；
// - 优先级 / 通配符 / 一次性订阅是输入事件分发的合理需求，与
//   `src/bus/`（精确类型事实广播）用途不同，二者不互相替代；
// - 每条同步订阅进入路由清单（docs/架构/路由清单.md）。
//
// 使用方式：
//   let sub_id = bus.subscribe(UiEventType::PointerMove, |ev| {
//       // 处理事件
//   });
//   bus.unsubscribe(sub_id); // 取消订阅（幂等）
// ============================================================================

use super::types::{UiEvent, UiEventType};
use std::cell::RefCell;

/// 事件处理函数签名：接收事件引用，无返回值（事件分发不承诺业务应答）。
pub(crate) type EventHandler = Box<dyn FnMut(&UiEvent)>;

/// 默认优先级（中间值）。
pub(crate) const PRIORITY_DEFAULT: i32 = 0;
/// 最高优先级（最先执行）。
// 保留优先级边界常量，供后续平台订阅者排序策略使用。
#[allow(dead_code)]
pub(crate) const PRIORITY_HIGHEST: i32 = i32::MAX;
/// 最低优先级（最后执行）。
// 保留优先级边界常量，供后续平台订阅者排序策略使用。
#[allow(dead_code)]
pub(crate) const PRIORITY_LOWEST: i32 = i32::MIN;

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
///
/// # 生命周期契约
///
/// - 注销幂等：重复 `unsubscribe` 与对已注销 id 注销均为 no-op；
/// - 发布期间注册表被排他借用（同步单线程场景，处理器应短小有界）；
///   处理器内注册/注销本总线未声明支持，业务代码不得依赖；
/// - 处理器失败（panic）会沿发布调用传播：本通道面向平台输入事件，
///   处理器由 platform 私有边界控制，panic 属进程级缺陷，不隔离。
pub(crate) struct EventBus {
    subscribers: RefCell<Vec<SubscriberEntry>>,
    next_id: usize,
}

impl EventBus {
    /// 创建新事件总线。
    pub(crate) fn new() -> Self {
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
        let mut subscribers = self.subscribers.borrow_mut();
        // 注册是低频路径：在此维持优先级降序，避免每次发布重复稳定排序。
        // partition_point 越过所有同优先级旧条目，保留注册先后顺序。
        let insertion = subscribers.partition_point(|entry| entry.priority >= priority);
        subscribers.insert(
            insertion,
            SubscriberEntry {
                id,
                event_type,
                handler,
                priority,
                once,
            },
        );
        id
    }

    // ── 标准订阅 ───────────────────────────────────────────────

    /// 订阅特定类型的事件。
    ///
    /// 返回 subscription ID（用于取消订阅）。
    pub(crate) fn subscribe<F>(&mut self, event_type: UiEventType, handler: F) -> usize
    where
        F: FnMut(&UiEvent) + 'static,
    {
        self.add_entry(Some(event_type), PRIORITY_DEFAULT, false, Box::new(handler))
    }

    /// 订阅所有事件（通配符订阅）。
    ///
    /// 返回 subscription ID（用于取消订阅）。
    pub(crate) fn subscribe_all<F>(&mut self, handler: F) -> usize
    where
        F: FnMut(&UiEvent) + 'static,
    {
        self.add_entry(None, PRIORITY_DEFAULT, false, Box::new(handler))
    }

    // ── 优先级订阅 ─────────────────────────────────────────────

    /// 按优先级订阅特定类型的事件。
    ///
    /// `priority` 越高越先执行。相同优先级按注册顺序。
    pub(crate) fn subscribe_with_priority<F>(
        &mut self,
        event_type: UiEventType,
        priority: i32,
        handler: F,
    ) -> usize
    where
        F: FnMut(&UiEvent) + 'static,
    {
        self.add_entry(Some(event_type), priority, false, Box::new(handler))
    }

    /// 按优先级订阅所有事件。
    pub(crate) fn subscribe_all_with_priority<F>(&mut self, priority: i32, handler: F) -> usize
    where
        F: FnMut(&UiEvent) + 'static,
    {
        self.add_entry(None, priority, false, Box::new(handler))
    }

    // ── 一次性订阅 ─────────────────────────────────────────────

    /// 一次性订阅特定类型的事件：触发一次后自动取消订阅。
    pub(crate) fn subscribe_once<F>(&mut self, event_type: UiEventType, handler: F) -> usize
    where
        F: FnMut(&UiEvent) + 'static,
    {
        self.add_entry(Some(event_type), PRIORITY_DEFAULT, true, Box::new(handler))
    }

    /// 一次性订阅所有事件：触发一次后自动取消订阅。
    pub(crate) fn subscribe_all_once<F>(&mut self, handler: F) -> usize
    where
        F: FnMut(&UiEvent) + 'static,
    {
        self.add_entry(None, PRIORITY_DEFAULT, true, Box::new(handler))
    }

    /// 一次性 + 优先级订阅特定类型的事件。
    pub(crate) fn subscribe_once_with_priority<F>(
        &mut self,
        event_type: UiEventType,
        priority: i32,
        handler: F,
    ) -> usize
    where
        F: FnMut(&UiEvent) + 'static,
    {
        self.add_entry(Some(event_type), priority, true, Box::new(handler))
    }

    // ── 取消订阅 ───────────────────────────────────────────────

    /// 取消订阅（幂等）：对已注销或不存在的 id 注销为 no-op。
    pub(crate) fn unsubscribe(&mut self, id: usize) {
        self.subscribers.borrow_mut().retain(|e| e.id != id);
    }

    // ── 发布事件 ───────────────────────────────────────────────

    /// 发布事件到所有匹配的订阅者。
    ///
    /// 按优先级降序（高→低）依次调用匹配的 handler，
    /// 相同优先级按注册顺序。发布后自动清理已触发的一次性订阅。
    ///
    /// 事件分发不承诺业务应答：退出事件循环等请求经直接契约流转，
    /// 不通过本总线的返回值表达。
    pub(crate) fn publish(&self, event: &UiEvent) {
        let mut subs = self.subscribers.borrow_mut();

        for entry in subs.iter_mut() {
            let matches = match entry.event_type {
                Some(et) => et == event.type_,
                None => true, // 通配符订阅匹配所有
            };
            if matches {
                (entry.handler)(event);
                // 一次性订阅：触发后标记为待删除（id=0，因为有效 id 从 1 开始）
                if entry.once {
                    entry.id = 0;
                }
            }
        }

        // 清理已触发的一次性订阅（id=0 的条目）
        subs.retain(|e| e.id != 0);
    }

    /// 清空所有订阅。
    pub(crate) fn clear(&mut self) {
        self.subscribers.borrow_mut().clear();
    }

    /// 当前订阅者数量。
    pub(crate) fn subscriber_count(&self) -> usize {
        self.subscribers.borrow().len()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/platform/windowing/event/bus__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
