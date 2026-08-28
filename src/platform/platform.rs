//! Platform System 根合同——统一访问平台中立的功能域子系统。

use crate::core::Error;
use crate::platform::display::IDisplay;
use crate::platform::system::info::ISystemInfo;
use crate::platform::windowing::event::{EventBus, IEventLoop};
use crate::platform::windowing::window::IWindowManager;
use crate::platform::windowing::{IClipboard, ICursor, ITextInput};

/// 平台根接口——持有并暴露所有 OS 中立的功能域合同。
pub trait PlatformSystem {
    /// 在 owner thread 边界取出一个原生回调失败。
    ///
    /// 回调只能入队类型化失败；应用在取出后决定恢复、返回或报告。
    fn take_pending_failure(&mut self) -> Option<Error> {
        None
    }

    fn window_manager(&mut self) -> &mut dyn IWindowManager;
    fn event_loop(&mut self) -> &mut dyn IEventLoop;
    fn event_bus(&mut self) -> &mut EventBus;
    fn clipboard(&mut self) -> &mut dyn IClipboard;
    fn cursor(&mut self) -> &mut dyn ICursor;
    fn display(&self) -> &dyn IDisplay;
    fn text_input(&mut self) -> &mut dyn ITextInput;
    fn system_info(&self) -> &dyn ISystemInfo;
}
