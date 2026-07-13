// ============================================================================
// native/shared/window_mode.rs — 原生窗口模式确认状态
//
// 平台协议回调用它记录 compositor / window server 已确认的窗口模式。
// 该状态与 PlatformWindowCore 的调用侧状态分开，避免异步确认被同步请求提前吞掉。
// ============================================================================

/// 原生 maximize 确认相对上一次 configure 的转换。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeMaximizeTransition {
    Unchanged,
    Maximized,
    Restored,
}

/// 单个原生窗口已确认的模式状态。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NativeWindowModeState {
    maximized: bool,
    fullscreen: bool,
}

impl NativeWindowModeState {
    /// 应用一次原生 configure，并返回 maximize 维度的边沿转换。
    pub(crate) fn apply_configure(
        &mut self,
        maximized: bool,
        fullscreen: bool,
    ) -> NativeMaximizeTransition {
        let transition = match (self.maximized, maximized) {
            (false, true) => NativeMaximizeTransition::Maximized,
            (true, false) => NativeMaximizeTransition::Restored,
            _ => NativeMaximizeTransition::Unchanged,
        };
        self.maximized = maximized;
        self.fullscreen = fullscreen;
        transition
    }

    pub(crate) fn snapshot(self) -> (bool, bool) {
        (self.maximized, self.fullscreen)
    }
}
