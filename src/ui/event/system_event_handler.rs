use super::{EventResult, SystemEvent};

/// 系统事件处理函数类型：接收事件并返回处理结果。
pub(crate) type SystemEventHandler = Box<dyn FnMut(&SystemEvent) -> EventResult + 'static>;

/// 事件过滤器：按事件类别决定处理函数是否响应某个事件。
#[derive(Clone, Copy)]
pub(crate) enum SystemEventFilter {
    /// 响应所有事件。
    Any,
    /// 仅响应指针（按下/双击/抬起/移动/进入/离开）事件。
    Pointer,
    /// 仅响应键盘按下/抬起事件。
    Key,
    /// 仅响应焦点进入/离开事件。
    Focus,
    /// 仅响应滚轮事件。
    Scroll,
}

impl SystemEventFilter {
    /// 判断过滤器是否匹配给定事件。
    fn matches(self, event: &SystemEvent) -> bool {
        match self {
            Self::Any => true,
            Self::Pointer => matches!(
                event,
                SystemEvent::PointerDown { .. }
                    | SystemEvent::PointerDoubleClick { .. }
                    | SystemEvent::PointerUp { .. }
                    | SystemEvent::PointerMove { .. }
                    | SystemEvent::PointerEnter
                    | SystemEvent::PointerLeave
            ),
            Self::Key => matches!(
                event,
                SystemEvent::KeyDown { .. } | SystemEvent::KeyUp { .. }
            ),
            Self::Focus => matches!(event, SystemEvent::FocusIn | SystemEvent::FocusOut),
            Self::Scroll => matches!(event, SystemEvent::Wheel { .. }),
        }
    }
}

/// 系统事件处理注册项：绑定过滤器与处理函数，并携带阶段/连续性选项。
pub(crate) struct SystemEventHandlerRegistration {
    /// 事件过滤器。
    filter: SystemEventFilter,
    /// 实际处理函数。
    handler: SystemEventHandler,
    /// 是否在捕获阶段（而非冒泡阶段）处理。
    capture_phase: bool,
    /// 是否要求持续接收指针移动事件。
    continuous_pointer_move: bool,
}

impl SystemEventHandlerRegistration {
    /// 创建响应所有事件的注册项。
    pub(crate) fn new(handler: SystemEventHandler) -> Self {
        Self {
            filter: SystemEventFilter::Any,
            handler,
            capture_phase: false,
            continuous_pointer_move: false,
        }
    }

    /// 创建按类别过滤的注册项；指针类过滤器自动开启持续移动接收。
    pub(crate) fn filtered(filter: SystemEventFilter, handler: SystemEventHandler) -> Self {
        Self {
            filter,
            handler,
            capture_phase: false,
            // 指针过滤器需要跟踪进入/离开过程中的移动事件。
            continuous_pointer_move: matches!(filter, SystemEventFilter::Pointer),
        }
    }

    /// 切换为捕获阶段处理。
    pub(crate) fn capture_phase(mut self) -> Self {
        self.capture_phase = true;
        self
    }

    /// 过滤器匹配时调用处理函数并返回其结果。
    pub(crate) fn handle(&mut self, event: &SystemEvent) -> Option<EventResult> {
        self.filter.matches(event).then(|| (self.handler)(event))
    }

    /// 是否需要在捕获阶段收到事件。
    pub(crate) fn wants_capture_phase(&self) -> bool {
        self.capture_phase
    }

    /// 是否需要在指针离开后仍持续接收移动事件。
    pub(crate) fn wants_continuous_pointer_move(&self) -> bool {
        self.continuous_pointer_move
    }
}
