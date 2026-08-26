// 引入被测映射与公开形状。
use super::*;

// 标准形状必须与 cursor-shape v1 的 CSS 语义一致。
#[test]
fn maps_all_standard_cursor_types_to_protocol_shapes() {
    // 逐项锁定跨平台枚举与 Wayland 协议值。
    let mappings = [
        // 默认箭头。
        (CursorType::Arrow, Shape::Default),
        // 文本输入。
        (CursorType::IBeam, Shape::Text),
        // 十字准星。
        (CursorType::Crosshair, Shape::Crosshair),
        // 可点击手形。
        (CursorType::Hand, Shape::Pointer),
        // 水平缩放。
        (CursorType::ResizeH, Shape::EwResize),
        // 垂直缩放。
        (CursorType::ResizeV, Shape::NsResize),
        // 东北到西南缩放。
        (CursorType::ResizeNE, Shape::NeswResize),
        // 西北到东南缩放。
        (CursorType::ResizeNW, Shape::NwseResize),
        // 移动。
        (CursorType::Move, Shape::Move),
        // 等待。
        (CursorType::Wait, Shape::Wait),
        // 禁止操作。
        (CursorType::NotAllowed, Shape::NotAllowed),
    ];
    // 每个标准形状都必须映射成功且值相等。
    for (cursor, expected) in mappings {
        // 比较生成协议枚举。
        assert_eq!(protocol_shape(cursor).expect("standard shape"), expected);
    }
}

// Custom 没有枚举或位图 owner，必须保持稳定 capability absence。
#[test]
fn rejects_custom_cursor_without_fake_default_shape() {
    // 请求未实现的位图形状。
    let error = protocol_shape(CursorType::Custom).expect_err("custom must be rejected");
    // App 依赖 NotImplemented 对稳定缺失去重。
    assert_eq!(error.code(), Errc::NotImplemented);
}
