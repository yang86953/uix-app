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
        .set_focused_widget(Some(target));
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
    assert_eq!(tree.managers().interaction.pressed_widget(), None);
    // 原生接管后潜在拖动必须被清除。
    assert!(!tree.managers().drag.is_potential());
    // pointer handoff 不得清除键盘焦点。
    assert_eq!(tree.managers().focus.focused_widget(), Some(target));
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
