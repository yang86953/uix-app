//! UI 窗口动作到当前原生窗口的窄边界适配。

use crate::core::Result;
use crate::native::traits::window::PlatformWindow;
use crate::ui::event::WindowAction;
use crate::ui::WidgetTree;

/// 在窗口显示前关闭系统标题栏，并重新声明期望的客户区尺寸。
pub(crate) fn configure_custom_title_bar(
    window: &mut dyn PlatformWindow,
    width: i32,
    height: i32,
) -> Result<()> {
    let properties = window.properties_mut();
    properties.set_system_title_bar_visible(false)?;
    properties.set_size(width, height)
}

pub(crate) fn apply_pending_window_actions(
    tree: &mut WidgetTree,
    window: &mut dyn PlatformWindow,
) -> Result<()> {
    for action in tree.take_window_actions() {
        apply_window_action(window, action)?;
    }
    Ok(())
}

pub(crate) fn apply_window_action(
    window: &mut dyn PlatformWindow,
    action: WindowAction,
) -> Result<()> {
    match action {
        WindowAction::BeginMoveDrag => window.begin_move_drag(),
        WindowAction::Minimize => window.properties_mut().minimize(),
        WindowAction::MaximizeRestore | WindowAction::ToggleMaximizeFromTitleBar => {
            if window.properties().is_maximized() {
                window.properties_mut().restore()
            } else {
                window.properties_mut().maximize()
            }
        }
        WindowAction::RequestClose => window.request_close(),
    }
}
