use crate::ui::{EventResult, SystemEvent, SystemEventKind};

/// 事件管理器默认优先级（中间值）。
pub const EM_PRIORITY_DEFAULT: i32 = 0;
/// 事件管理器最高优先级（最先处理）。
pub const EM_PRIORITY_HIGHEST: i32 = i32::MAX;
/// 事件管理器最低优先级（最后处理）。
pub const EM_PRIORITY_LOWEST: i32 = i32::MIN;

struct HandlerEntry {
    id: usize,
    /// None = 处理所有事件类型
    event_kind: Option<SystemEventKind>,
    handler: Box<dyn FnMut(&SystemEvent) -> EventResult>,
    priority: i32,
}

/// 每个 widget 的事件处理链管理器。
///
/// 支持按事件类型过滤和优先级排序，在 widget 的 `on_event` 之后自动调用。
///
/// # 使用方式
///
/// ```ignore
/// use crate::ui::managers::EventManager;
///
/// let mut mgr = EventManager::new();
///
/// // 处理所有事件（等价于旧的 add_handler）
/// mgr.add_handler(|ev| { ...; EventResult::Handled });
///
/// // 只处理特定类型
/// mgr.on_kind(SystemEventKind::PointerDown, |ev| { ...; EventResult::Handled });
///
/// // 高优先级处理
/// mgr.on_kind_with_priority(SystemEventKind::Wheel, 100, |ev| { ... });
/// ```
#[derive(Default)]
pub struct EventManager {
    handlers: Vec<HandlerEntry>,
    next_id: usize,
}

impl EventManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加一个处理所有事件类型的 handler。
    /// 返回 handler ID（用于 `remove`）。
    pub fn add_handler<F>(&mut self, handler: F) -> usize
    where
        F: FnMut(&SystemEvent) -> EventResult + 'static,
    {
        self.add_handler_inner(None, EM_PRIORITY_DEFAULT, Box::new(handler))
    }

    /// 按优先级添加 handler（处理所有事件类型）。
    pub fn add_handler_with_priority<F>(&mut self, priority: i32, handler: F) -> usize
    where
        F: FnMut(&SystemEvent) -> EventResult + 'static,
    {
        self.add_handler_inner(None, priority, Box::new(handler))
    }

    /// 只处理特定事件类型。
    pub fn on_kind<F>(&mut self, kind: SystemEventKind, handler: F) -> usize
    where
        F: FnMut(&SystemEvent) -> EventResult + 'static,
    {
        self.add_handler_inner(Some(kind), EM_PRIORITY_DEFAULT, Box::new(handler))
    }

    /// 按优先级处理特定事件类型。
    pub fn on_kind_with_priority<F>(
        &mut self,
        kind: SystemEventKind,
        priority: i32,
        handler: F,
    ) -> usize
    where
        F: FnMut(&SystemEvent) -> EventResult + 'static,
    {
        self.add_handler_inner(Some(kind), priority, Box::new(handler))
    }

    fn add_handler_inner(
        &mut self,
        event_kind: Option<SystemEventKind>,
        priority: i32,
        handler: Box<dyn FnMut(&SystemEvent) -> EventResult>,
    ) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push(HandlerEntry {
            id,
            event_kind,
            handler,
            priority,
        });
        id
    }

    /// 移除指定 ID 的 handler。
    pub fn remove(&mut self, id: usize) {
        self.handlers.retain(|e| e.id != id);
    }

    /// 清空所有 handler。
    pub fn clear(&mut self) {
        self.handlers.clear();
    }

    /// 当前 handler 数量。
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    /// 是否没有注册任何 handler。
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }

    /// 分发事件到所有匹配的 handler。
    ///
    /// 按优先级降序（高→低）执行，相同优先级按注册顺序。
    /// 任意 handler 返回 `Handled` 则终止并返回 `Handled`。
    pub fn dispatch(&mut self, event: &SystemEvent) -> EventResult {
        // 按优先级降序排序
        self.handlers.sort_by(|a, b| b.priority.cmp(&a.priority));

        for entry in &mut self.handlers {
            let matches = match entry.event_kind {
                Some(kind) => kind == event.kind(),
                None => true,
            };
            if matches && (entry.handler)(event) == EventResult::Handled {
                return EventResult::Handled;
            }
        }
        EventResult::NotHandled
    }
}
