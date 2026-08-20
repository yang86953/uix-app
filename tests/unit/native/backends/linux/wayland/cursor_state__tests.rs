// 引入被测 Widget 端口。
use super::*;

// 全部公开形状都必须在 Widget 内无损 round-trip。
#[test]
fn cursor_intent_round_trips_all_public_shapes() {
    // 枚举当前公开契约的完整形状集合。
    let cursors = [
        // 默认箭头。
        CursorType::Arrow,
        // 文本输入。
        CursorType::IBeam,
        // 十字准星。
        CursorType::Crosshair,
        // 可点击手形。
        CursorType::Hand,
        // 水平缩放。
        CursorType::ResizeH,
        // 垂直缩放。
        CursorType::ResizeV,
        // 东北到西南缩放。
        CursorType::ResizeNE,
        // 西北到东南缩放。
        CursorType::ResizeNW,
        // 移动。
        CursorType::Move,
        // 等待。
        CursorType::Wait,
        // 禁止操作。
        CursorType::NotAllowed,
        // 自定义候选仍需保持值身份。
        CursorType::Custom,
    ];
    // 每次提交后立即读取同一 owner 快照。
    for cursor in cursors {
        // 新 Widget 从干净状态开始。
        let state = WaylandCursorState::default();
        // 发布候选形状。
        state.commit_cursor(cursor);
        // 快照必须返回同一枚举值。
        assert_eq!(state.snapshot().cursor, cursor);
    }
}

// Enter serial 必须覆盖旧值并在 Leave 后彻底失效。
#[test]
fn enter_and_leave_bound_serial_to_current_focus() {
    // 构造无焦点默认状态。
    let state = WaylandCursorState::default();
    // 初始快照不得伪造 serial。
    assert_eq!(state.snapshot().enter_serial, None);
    // 最大 u32 serial 验证加一编码没有溢出。
    let entered = state.record_enter(u32::MAX);
    // Enter 快照必须携带事件原值。
    assert_eq!(entered.enter_serial, Some(u32::MAX));
    // Leave 失效同一授权。
    state.clear_enter();
    // 后续快照不得跨焦点复用 serial。
    assert_eq!(state.snapshot().enter_serial, None);
}

// 无焦点 intent 必须保留到下一次 Enter 重放。
#[test]
fn pending_shape_and_visibility_survive_until_enter() {
    // 构造可见默认箭头。
    let state = WaylandCursorState::default();
    // 在没有焦点时记录新形状。
    state.commit_cursor(CursorType::Hand);
    // 同时记录隐藏请求。
    state.commit_visibility(false);
    // 下一次 Enter 返回完整待应用状态。
    let entered = state.record_enter(42);
    // 新 serial 来自本次 Enter。
    assert_eq!(entered.enter_serial, Some(42));
    // 形状保持调用方最后一次请求。
    assert_eq!(entered.cursor, CursorType::Hand);
    // 隐藏状态同样跨无焦点阶段保留。
    assert!(!entered.visible);
}
