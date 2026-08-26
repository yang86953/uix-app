// 引入当前私有播放状态机。
use super::*;
// 引入 typed 关键帧构造器。
use crate::ui::animation::Keyframe;

// 构造从零到十的单秒关键帧动画。
fn sample_animation() -> KeyframeAnimation<f32> {
    // 两端显式声明，避免基础值补帧影响断言。
    KeyframeAnimation::new(
        // 提供线性两端帧。
        [Keyframe::new(0.0, 0.0), Keyframe::new(1.0, 10.0)],
        // 单轮一秒。
        1.0,
    )
    // 测试输入始终非空且偏移有限。
    .expect("测试关键帧必须有效")
}

// 验证有限交替播放按真实最后一轮方向定格。
#[test]
fn alternate_count_finishes_at_last_reverse_endpoint() {
    // 固定起始时刻以控制跨轮推进。
    let now = Instant::now();
    // 构造两轮交替且保留终值的播放。
    let mut playback = AnimatedPlayback::new_keyframes(
        // 使用标准测试序列。
        sample_animation(),
        // 基础值不参与 forwards 终值。
        5.0,
        // 两轮交替后应回到起点。
        KeyframePlayback::new(1.0)
            // 总共播放两轮。
            .with_iterations(2)
            // 第二轮倒放。
            .with_direction(KeyframeDirection::Alternate),
        // 绑定固定起始时刻。
        now,
    );
    // 一帧跨过两轮并进入完成态。
    let (value, active) = playback.advance(now + Duration::from_secs(2), 2.0);
    // 有限两轮完成后不再活跃。
    assert!(!active);
    // 交替第二轮的终点必须是原始起点。
    assert_eq!(value, Some(0.0));
}

// 验证 none 填充在延迟期与完成后都恢复基础值。
#[test]
fn none_fill_uses_underlying_value_before_and_after_playback() {
    // 固定起始时刻以控制延迟截止点。
    let now = Instant::now();
    // 构造带一秒延迟且不填充的单轮播放。
    let mut playback = AnimatedPlayback::new_keyframes(
        // 使用标准测试序列。
        sample_animation(),
        // 保存区别于关键帧端点的基础值。
        5.0,
        // 单轮配置增加延迟并明确 none 填充。
        KeyframePlayback::new(1.0)
            // 等待一秒后开始。
            .with_delay(1.0)
            // 延迟前后均不保留关键帧值。
            .with_fill_mode(KeyframeFillMode::None),
        // 绑定固定起始时刻。
        now,
    );
    // 延迟期应直接读取基础值。
    assert_eq!(playback.value(), 5.0);
    // 截止点后跨过完整单轮。
    let (value, active) = playback.advance(now + Duration::from_secs(2), 2.0);
    // 单轮完成后停止续帧。
    assert!(!active);
    // 完成后立即恢复基础值。
    assert_eq!(value, Some(5.0));
}
