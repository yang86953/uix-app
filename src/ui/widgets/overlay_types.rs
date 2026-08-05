//! 跨组件族共享的浮层交互模型。

/// 浮层提示相对目标节点的放置方向。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TooltipPlacement {
    /// 将提示放在目标上方。
    Top,
    /// 将提示放在目标下方。
    Bottom,
    /// 将提示放在目标左侧。
    Left,
    /// 将提示放在目标右侧。
    Right,
}

/// 可复用浮层触发方式。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TriggerMode {
    /// 指针悬停时触发。
    Hover,
    /// 主动点击时触发。
    Click,
    /// 键盘焦点进入时触发。
    Focus,
    /// 请求上下文菜单时触发。
    ContextMenu,
}
