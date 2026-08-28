//! 平台中立输入协议。
//!
//! 本叶模块只定义 app、平台聚合与原生后端共享的输入合同；具体光标
//! 与 IME 实现及其原生资源生命周期继续由原生私有层持有。
//! 键盘即时状态经事件流（UiEvent）消费，不设轮询端口。

use crate::core::{Error, Point, Rect, WindowId};

use super::CursorType;

/// 平台光标控制合同。
pub(crate) trait ICursor {
    /// 切换当前光标形状。
    fn set_cursor(&mut self, cursor: CursorType) -> Result<(), Error>;
    /// 设置光标可见性。
    fn show_cursor(&mut self, visible: bool) -> Result<(), Error>;
    /// 读取当前光标位置。
    fn cursor_position(&self) -> Result<Point, Error>;
    /// 移动光标到指定位置。
    fn set_cursor_position(&mut self, x: i32, y: i32) -> Result<(), Error>;
    /// 启用或解除光标约束。
    fn confine_cursor(&mut self, confine: bool) -> Result<(), Error>;
    /// 捕获鼠标输入。
    fn capture_mouse(&mut self) -> Result<(), Error>;
    /// 释放鼠标输入捕获。
    fn release_mouse(&mut self) -> Result<(), Error>;
}

/// 平台文本输入与 IME 会话合同。
pub(crate) trait ITextInput {
    /// 原子切换后续 IME 会话调用所属的窗口。
    ///
    /// 文本输入即使由进程级服务提供也仍按窗口归属；实现必须同时切换不透明
    /// 原生目标，以及用于标记后续事件的 `WindowId`。
    fn set_target_window(
        &mut self,
        window_id: WindowId,
        native_window: *mut std::ffi::c_void,
    ) -> Result<(), Error>;
    /// 启动当前目标窗口的文本输入会话。
    fn start(&mut self) -> Result<(), Error>;
    /// 停止当前文本输入会话。
    fn stop(&mut self) -> Result<(), Error>;
    /// 更新输入法候选窗口对应的光标矩形。
    fn set_cursor_rect(&mut self, rect: Rect) -> Result<(), Error>;
}
