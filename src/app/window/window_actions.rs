//! UI 窗口动作到当前原生窗口的窄边界适配。

// 引入窗口动作结果与可区分的平台错误码。
use crate::core::{Errc, Result};
use crate::platform::windowing::window::PlatformWindow;
// 引入当前原生指针事件携带的不可解释激活身份。
use crate::platform::windowing::event::PointerActivationId;
use crate::ui::WidgetTree;
use crate::ui::event::WindowAction;

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

// 执行 UI 在当前同步边界产生的全部窗口命令。
pub(crate) fn apply_pending_window_actions(
    // UI 树仍只拥有平台中立的窗口动作队列。
    tree: &mut WidgetTree,
    // 原生窗口是动作的唯一命令接收者。
    window: &mut dyn PlatformWindow,
    // 仅当前原生事件可提供一次性指针激活上下文。
    pointer_activation: Option<PointerActivationId>,
    // 返回真正的平台操作失败，正常过期授权由后端忽略。
) -> Result<()> {
    for action in tree.take_window_actions() {
        // 原生交互移动会接管 pointer，UI 不应等待可能永不返回的 PointerUp。
        let cancels_pointer_gesture = matches!(
            // 移动和缩放都会把当前指针手势移交给原生窗口管理器。
            action,
            // 两类交互接管都必须清理 UI pressed 与潜在 drag。
            WindowAction::BeginMoveDrag | WindowAction::BeginResizeDrag(_)
        );
        // 将同一事件上下文只作为动作执行参数转交，不存入 UI。
        apply_window_action(window, action, pointer_activation)?;
        // 平台已提交或安全忽略接管请求后，统一清除 pressed/drag 且保留键盘焦点。
        if cancels_pointer_gesture {
            // 使用平台中立的 WidgetTree 生命周期入口完成手势取消。
            tree.cancel_pointer_gesture_for_native_handoff();
            // 结束原生指针接管清理。
        }
    }
    Ok(())
}

// 将一个平台中立窗口动作映射到定向原生命令。
pub(crate) fn apply_window_action(
    // 窗口命令始终定向到事件所属的原生窗口。
    window: &mut dyn PlatformWindow,
    // UI 动作保持无平台数据的既有枚举契约。
    action: WindowAction,
    // 激活身份只在原生移动或缩放分支中具有意义。
    pointer_activation: Option<PointerActivationId>,
    // 保持既有窗口操作结果契约。
) -> Result<()> {
    match action {
        // 原生拖动必须关联产生该动作的同一次 PointerDown。
        WindowAction::BeginMoveDrag => window.begin_move_drag(pointer_activation),
        // 原生缩放必须保留 UI 热区声明的方向与同一次 PointerDown 身份。
        WindowAction::BeginResizeDrag(edge) => {
            // 两个参数只在当前同步调用边界内生效。
            window.begin_resize_drag(edge, pointer_activation)
        }
        WindowAction::Minimize => window.properties_mut().minimize(),
        WindowAction::MaximizeRestore | WindowAction::ToggleMaximizeFromTitleBar => {
            if window.properties().is_maximized() {
                window.properties_mut().restore()
            } else {
                window.properties_mut().maximize()
            }
        }
        WindowAction::ShowSystemMenuFromTitleBar => window.show_system_menu(),
        WindowAction::RequestClose => window.request_close(),
    }
}

// 统一报告自动居中结果，并把平台预期不支持与真实执行失败分开。
pub(crate) fn report_center_on_screen_result(
    // 接收统一诊断 System，真实失败进入框架报告。
    diagnostics: &crate::diagnostics::Diagnostics,
    // 日志上下文由具体窗口创建入口提供。
    context: &'static str,
    // 平台结果保持 typed error，不在适配层改写能力事实。
    result: Result<()>,
    // 返回本次是否实际记录了警告，供局部契约测试观测。
) -> bool {
    // 成功不需要任何诊断。
    let Err(error) = result else {
        // 明确表示没有记录警告。
        return false;
        // 结束成功分支。
    };
    // Wayland 等平台的预期能力缺失交给 compositor 默认放置。
    if error.code() == Errc::NotImplemented {
        // 预期能力缺失不应污染用户日志。
        return false;
        // 结束预期能力缺失分支。
    }
    // 其余平台错误保留可观测诊断，并携带静态上下文进入框架报告。
    tracing::warn!("{context}: {}", error.short_what());
    diagnostics.report_with_origin(
        error,
        crate::diagnostics::ReportOrigin::framework("window", context),
    );
    // 告知测试本次确实走过警告路径。
    true
    // 结束自动居中结果报告。
}
