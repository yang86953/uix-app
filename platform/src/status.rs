// ============================================================================
// core/status.rs — 状态与尺寸枚举
//
// 框架级通用枚举：状态级别（替代分散的 AlertType/MessageType/...）、
// 控件尺寸（替代分散的 ButtonSize/InputSize/ListSize）、滚动方向。
// ============================================================================

/// 通用状态级别。
///
/// 统一替代各层分散定义的 `AlertType`、`MessageType`、
/// `NotificationType`、`NotificationLevel`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusLevel {
    Success,
    Info,
    Warning,
    Error,
}

/// 通用控件尺寸。
///
/// 统一替代各 widget 分散定义的 `ButtonSize`、`InputSize`、`ListSize`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlSize {
    Small,
    Medium,
    Large,
}

/// 滚动方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDirection {
    /// 仅纵向滚动。
    Vertical,
    /// 仅横向滚动。
    Horizontal,
    /// 双向滚动。
    Both,
}

impl ScrollDirection {
    pub fn can_scroll_x(&self) -> bool {
        matches!(self, Self::Horizontal | Self::Both)
    }

    pub fn can_scroll_y(&self) -> bool {
        matches!(self, Self::Vertical | Self::Both)
    }
}
