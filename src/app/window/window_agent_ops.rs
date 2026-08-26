//! PlatformWindow 到 AgentWindowOps 的适配器。
//!
//! 窗口级 Agent 动作（缩放 / 移动 / 最大化等）由窗口驱动层在每帧 UI turn
//! 内通过本适配器调用平台窗口；平台错误映射为 `AgentWindowOpsError`，
//! 由命令执行器转换为 `window_operation_failed` 命令失败。

use crate::app::queues::window_agent_state::{AgentWindowOps, AgentWindowOpsError};
use crate::platform::windowing::window::PlatformWindow;

/// 借用平台窗口实现窗口级 Agent 动作。
pub(crate) struct PlatformWindowAgentOps<'a> {
    window: &'a mut dyn PlatformWindow,
}

impl<'a> PlatformWindowAgentOps<'a> {
    pub(crate) fn new(window: &'a mut dyn PlatformWindow) -> Self {
        Self { window }
    }
}

impl AgentWindowOps for PlatformWindowAgentOps<'_> {
    fn resize(&mut self, width: i32, height: i32) -> Result<(), AgentWindowOpsError> {
        self.window
            .properties_mut()
            .set_size(width, height)
            .map_err(map_error("resize"))
    }

    fn move_to(&mut self, x: i32, y: i32) -> Result<(), AgentWindowOpsError> {
        self.window
            .properties_mut()
            .set_position(x, y)
            .map_err(map_error("move"))
    }

    fn maximize(&mut self) -> Result<(), AgentWindowOpsError> {
        self.window
            .properties_mut()
            .maximize()
            .map_err(map_error("maximize"))
    }

    fn minimize(&mut self) -> Result<(), AgentWindowOpsError> {
        self.window
            .properties_mut()
            .minimize()
            .map_err(map_error("minimize"))
    }

    fn restore(&mut self) -> Result<(), AgentWindowOpsError> {
        self.window
            .properties_mut()
            .restore()
            .map_err(map_error("restore"))
    }

    fn request_close(&mut self) -> Result<(), AgentWindowOpsError> {
        self.window
            .request_close()
            .map_err(map_error("request_close"))
    }
}

fn map_error(operation: &'static str) -> impl FnOnce(crate::core::Error) -> AgentWindowOpsError {
    move |error| AgentWindowOpsError {
        operation,
        message: error.short_what().to_owned(),
    }
}
