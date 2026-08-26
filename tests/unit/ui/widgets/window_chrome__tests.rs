// 引入当前模块的窗口控制构造器与快照类型。
use super::*;
// 引入构造指针事件所需的平台中立输入值。
use crate::core::Point;
// 引入空修饰键掩码。
use crate::platform::windowing::KeyMod;

// 提取组合节点直接子项的窗口动作顺序。
fn child_controls(node: &ViewNode) -> Vec<WindowControl> {
    // 把每个交互包装器快照投影为窗口动作。
    node.children
        // 按源码顺序遍历直接子项。
        .iter()
        // 要求每个子项都是窗口控制交互包装器。
        .map(|child| match child.widget.snapshot_fields() {
            // 返回快照持有的窗口动作。
            SnapshotFields::WindowControl { control, .. } => control,
            // 非窗口控制子项表示组合契约被破坏。
            _ => panic!("window_controls 只能包含窗口控制子项"),
        })
        // 收集稳定动作顺序。
        .collect()
}

// 验证默认三按钮集合保持文档顺序。
#[test]
fn standard_controls_preserve_documented_order() {
    // 构造全部标准窗口控制。
    let controls = window_controls(true, true, true);
    // 顺序必须固定为最小化、最大化/还原、关闭。
    assert_eq!(
        // 提取实际动作顺序。
        child_controls(&controls),
        // 声明文档化动作顺序。
        vec![
            // 首项为最小化。
            WindowControl::Minimize,
            // 次项为最大化/还原。
            WindowControl::MaximizeRestore,
            // 末项为关闭。
            WindowControl::Close,
        ]
    );
    // 每个标准动作必须保留非空的默认无障碍名称。
    assert!(controls.children.iter().all(
        // 检查每个交互包装器的快照语义。
        |child| matches!(
            // 读取组件公开快照字段。
            child.widget.snapshot_fields(),
            // 只接受带非空名称的窗口控制快照。
            SnapshotFields::WindowControl { accessible_name, .. } if !accessible_name.is_empty()
        )
    ));
}

// 验证显示开关只影响对应动作节点。
#[test]
fn standard_controls_respect_visibility_flags() {
    // 只构造最大化/还原控制。
    let maximize_only = window_controls(false, true, false);
    // 组合中只能保留最大化/还原动作。
    assert_eq!(
        // 提取选择性组合动作。
        child_controls(&maximize_only),
        // 声明唯一预期动作。
        vec![WindowControl::MaximizeRestore]
    );
    // 全部关闭时仍返回合法的空行视图。
    let hidden = window_controls(false, false, false);
    // 空组合不能残留任何交互节点。
    assert!(hidden.children.is_empty());
}

// 验证公开缩放热区同时表达光标与精确原生动作。
#[test]
fn resize_region_maps_cursor_and_pointer_action() {
    // 声明全部方向与标准平台缩放光标的完整映射。
    let cursor_cases = [
        // 上边使用垂直缩放光标。
        (WindowResizeEdge::Top, CursorType::ResizeV),
        // 下边使用垂直缩放光标。
        (WindowResizeEdge::Bottom, CursorType::ResizeV),
        // 左边使用水平缩放光标。
        (WindowResizeEdge::Left, CursorType::ResizeH),
        // 右边使用水平缩放光标。
        (WindowResizeEdge::Right, CursorType::ResizeH),
        // 左上角使用西北到东南缩放光标。
        (WindowResizeEdge::TopLeft, CursorType::ResizeNW),
        // 右上角使用东北到西南缩放光标。
        (WindowResizeEdge::TopRight, CursorType::ResizeNE),
        // 左下角使用东北到西南缩放光标。
        (WindowResizeEdge::BottomLeft, CursorType::ResizeNE),
        // 右下角使用西北到东南缩放光标。
        (WindowResizeEdge::BottomRight, CursorType::ResizeNW),
    ];
    // 逐一验证公开热区附带正确光标。
    for (edge, expected_cursor) in cursor_cases {
        // 使用无外观内容构造当前方向的窗口缩放热区。
        let region = window_resize_region(edge, ViewNode::leaf(Container::new()));
        // 热区必须显式保存与方向一致的平台光标。
        assert_eq!(region.cursor, Some(expected_cursor));
    }
    // 直接构造同一方向的交互组件以验证事件动作。
    let mut interaction = WindowInteractionRegion::resize(WindowResizeEdge::BottomRight);
    // 主按钮按下必须由缩放热区消费。
    assert_eq!(
        // 提交平台中立的左键 PointerDown。
        interaction.on_event(&SystemEvent::PointerDown {
            // 坐标由窗口管理器使用当前原生事件解释。
            pos: Point::new(3.0, 4.0),
            // 只允许主按钮启动原生缩放。
            button: MouseButton::Left,
            // 本场景没有键盘修饰键。
            mods: KeyMod::NONE,
        }),
        // 缩放热区必须终止子节点传播。
        EventResult::Handled,
    );
    // 动作必须保留原始右下角方向，不能退化为通用移动。
    assert_eq!(
        interaction.take_window_action(),
        Some(WindowAction::BeginResizeDrag(WindowResizeEdge::BottomRight)),
    );
}
