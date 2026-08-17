//! UI 窗口动作到当前原生窗口的窄边界适配。

// 引入窗口动作结果与可区分的平台错误码。
use crate::core::{Errc, Result};
use crate::native::windowing::window::PlatformWindow;
// 引入当前原生指针事件携带的不可解释激活身份。
use crate::native::windowing::event::PointerActivationId;
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
    // 日志上下文由具体窗口创建入口提供。
    context: &str,
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
    // 其余平台错误仍需保留可观测诊断。
    tracing::warn!("{context}: {}", error.short_what());
    // 告知测试本次确实走过警告路径。
    true
    // 结束自动居中结果报告。
}

// 单元测试直接验证自动居中结果分类，不依赖真实窗口后端。
#[cfg(test)]
// 测试模块只访问当前窄边界。
mod center_on_screen_result_tests {
    // 引入当前模块的统一报告入口。
    use super::report_center_on_screen_result;
    // 引入构造 typed failure 所需的错误类型与错误码。
    use crate::core::{Errc, Error};

    // 验证只有真实执行失败会进入警告路径。
    #[test]
    // 单一测试覆盖成功、预期缺失与真实失败三种完整分类。
    fn suppresses_only_expected_not_implemented_failure() {
        // 成功结果不得记录警告。
        assert!(!report_center_on_screen_result("success", Ok(())));
        // 构造 Wayland 当前使用的预期能力缺失。
        let unsupported = Error::new(Errc::NotImplemented, "center is compositor-owned");
        // 预期能力缺失不得记录警告。
        assert!(!report_center_on_screen_result(
            "unsupported",
            Err(unsupported)
        ));
        // 构造必须继续暴露的真实平台执行失败。
        let failed = Error::new(Errc::PlatformError, "native center failed");
        // 真实失败必须经过警告路径。
        assert!(report_center_on_screen_result("failed", Err(failed)));
        // 结束自动居中分类测试。
    }
    // 结束自动居中结果测试模块。
}

// 单元测试验证 app 到原生窗口的不可解释激活身份不会丢失或替换。
#[cfg(all(test, feature = "test-harness"))]
// 测试模块只访问 crate 私有窗口适配边界。
mod tests {
    // 引入当前模块的动作映射实现。
    use super::*;
    // 引入事件构造所需的坐标值。
    use crate::core::Point;
    // 使用真实内存窗口替身记录精确调用上下文。
    use crate::native::test_harness::FakeWindow;
    // 引入平台中立缩放方向供窗口动作转发断言使用。
    use crate::platform::windowing::WindowResizeEdge;
    // 引入原生事件与平台中立鼠标键。
    use crate::native::windowing::{
        // UiEvent 提供当前 PointerDown 的激活身份读取。
        event::UiEvent,
        // 输入值用于建立与标题栏相同的左键手势。
        input::{KeyMod, MouseButton},
        // 结束原生事件与输入类型导入。
    };
    // 使用简单节点建立可观察的 pressed、drag 与 keyboard focus 状态。
    use crate::ui::widgets::Label;

    // 验证同一个 PointerDown 的激活身份原样到达 PlatformWindow。
    #[test]
    // 测试以 Result 返回，避免隐藏任何窗口动作失败。
    fn native_move_forwards_exact_pointer_activation() -> Result<()> {
        // 创建稳定的不可解释测试身份。
        let activation = PointerActivationId::new(73);
        // 构造携带该身份的真实 UiEvent。
        let pointer_down = UiEvent::pointer_down(Point::new(8.0, 9.0), MouseButton::Left)
            // 模拟 Wayland seat 为同一 press 附着身份。
            .with_pointer_activation(activation);
        // 创建记录窗口动作的内存替身。
        let mut window = FakeWindow::new(7, "activation-forwarding", 320, 200);
        // 创建窗口拥有的 UI 树。
        let mut tree = WidgetTree::new();
        // 添加稳定根节点作为手势与键盘焦点目标。
        let target = tree.set_root(Box::new(Label::new("native move")));
        // 模拟当前 PointerDown 已建立 pressed 捕获。
        let began = tree
            // 访问交互 manager。
            .managers_mut()
            // 选择 pressed 状态所有者。
            .interaction
            // 绑定标题栏左键手势。
            .begin_pressed_pointer(Some(target), MouseButton::Left);
        // 当前没有冲突按键，手势建立必须成功。
        assert!(began);
        // 建立同一次 PointerDown 的潜在拖动状态。
        tree.managers_mut().drag.begin_gesture(
            // 绑定相同 UI 目标。
            Some(target),
            // 使用当前事件坐标。
            Point::new(8.0, 9.0),
            // 使用标题栏拖动左键。
            MouseButton::Left,
            // 本场景没有修饰键。
            KeyMod::NONE,
            // 结束潜在拖动建立。
        );
        // 独立建立键盘焦点以验证 pointer handoff 不伪造 WindowBlur。
        tree.managers_mut()
            // 访问焦点 manager。
            .focus
            // 绑定稳定焦点目标。
            .set_focused_component(Some(target));
        // 模拟 UI 在 PointerDown 同步边界产生原生移动动作。
        tree.pending_window_actions
            // WindowAction 本身仍不携带任何平台授权数据。
            .push(WindowAction::BeginMoveDrag);
        // 从真实 pending 动作入口转交当前事件身份并清理 UI 手势。
        apply_pending_window_actions(
            // 传入产生动作的同一 UI 树。
            &mut tree,
            // 使用本次测试窗口作为唯一动作接收者。
            &mut window,
            // 只转交当前原生事件的私有上下文。
            pointer_down.pointer_activation(),
            // 结束带身份 pending 动作调用。
        )?;
        // 原生接管后 pressed 捕获必须被清除。
        assert_eq!(tree.managers().interaction.pressed_component(), None);
        // 原生接管后潜在拖动必须被清除。
        assert!(!tree.managers().drag.is_potential());
        // pointer handoff 不得清除键盘焦点。
        assert_eq!(tree.managers().focus.focused_component(), Some(target));
        // 再排入一次不携带原生身份的拖动动作。
        tree.pending_window_actions
            // 保持与第一轮相同的公共动作语义。
            .push(WindowAction::BeginMoveDrag);
        // 非原生动作边界明确转交 None，不能复用前一次身份。
        apply_pending_window_actions(
            // 复用已完成手势清理的 UI 树。
            &mut tree,
            // 仍定向到同一测试窗口。
            &mut window,
            // 本轮没有可验证原生 PointerDown。
            None,
            // 结束无身份 pending 动作调用。
        )?;
        // FakeWindow 必须按顺序收到精确 Some(id) 与 None。
        assert_eq!(
            // 读取真实适配调用历史。
            window.state.begin_move_drag_activations,
            // 期望值不得解释或替换激活身份。
            vec![Some(activation), None],
            // 结束精确转发断言。
        );
        // 测试成功完成。
        Ok(())
        // 结束激活身份转发测试。
    }

    // 验证缩放方向与同一个 PointerDown 激活身份原样到达 PlatformWindow。
    #[test]
    fn native_resize_forwards_edge_and_pointer_activation() -> Result<()> {
        // 创建稳定的不可解释测试身份。
        let activation = PointerActivationId::new(91);
        // 创建记录窗口缩放动作的内存替身。
        let mut window = FakeWindow::new(8, "resize-forwarding", 320, 200);
        // 直接映射一次右下角缩放动作，隔离本测试关注的窄边界。
        apply_window_action(
            // 使用唯一的动作接收窗口。
            &mut window,
            // 保留调用热区选择的右下角方向。
            WindowAction::BeginResizeDrag(WindowResizeEdge::BottomRight),
            // 转交产生动作的同一次原生 PointerDown 身份。
            Some(activation),
        )?;
        // 测试替身必须收到精确方向与身份，不能退化或替换。
        assert_eq!(
            window.state.begin_resize_drag_activations,
            vec![(WindowResizeEdge::BottomRight, Some(activation))],
        );
        // 测试成功完成。
        Ok(())
    }
    // 结束窗口动作契约测试模块。
}
