//! `native/backends/linux/windowing/wayland/pointer_axis.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::PointerAxisFrame;

// 离散刻度必须覆盖 compositor 同帧附带的连续距离。
#[test]
fn discrete_steps_are_authoritative() {
    let mut frame = PointerAxisFrame::default();
    frame.record_continuous(false, 15.0);
    frame.record_discrete(false, 1);
    assert_eq!(frame.take_normalized(), Some((0.0, 1.0)));
    assert_eq!(frame.take_normalized(), None);
}

// 无离散事件的触控板距离必须换算为与滚轮相同的逻辑单位。
#[test]
fn continuous_distance_is_normalized() {
    let mut frame = PointerAxisFrame::default();
    frame.record_continuous(true, -7.5);
    frame.record_continuous(false, 30.0);
    assert_eq!(frame.take_normalized(), Some((-0.5, 2.0)));
}

// v8 高精度刻度必须覆盖旧离散值和连续距离，并保留分数步长。
#[test]
fn value120_is_the_authoritative_high_resolution_unit() {
    let mut frame = PointerAxisFrame::default();
    frame.record_continuous(false, 15.0);
    frame.record_discrete(false, 1);
    frame.record_value120(false, 60);
    assert_eq!(frame.take_normalized(), Some((0.0, 0.5)));
}
