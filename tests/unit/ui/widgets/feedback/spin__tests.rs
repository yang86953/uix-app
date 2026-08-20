    // 复用被测加载指示器。
    use super::Spin;
    // 引入动画更新公开组件契约。
    use crate::ui::widget_runtime::traits::WidgetAnimation;
    // 引入延迟配置类型。
    use std::time::Duration;

    // 验证配置刷新在延迟未变化时保留运行时动画进度。
    #[test]
    // 测试名称说明运行时状态所有权边界。
    fn refresh_preserves_runtime_animation_progress() {
        // 创建带稳定延迟的当前运行时组件。
        let mut current = Spin::new().delay(Duration::from_secs(1));
        // 推进延迟计时直至指示器进入可见阶段。
        WidgetAnimation::update_animation(&mut current, 1.0);
        // 继续推进动画以建立非零运行时相位。
        WidgetAnimation::update_animation(&mut current, 0.25);
        // 记录刷新前的动画相位。
        let phase = current.phase();
        // 记录刷新前的延迟进度。
        let delay_elapsed = current.delay_elapsed;
        // 前置步骤必须确实建立非零动画相位。
        assert!(phase > 0.0);
        // 前置步骤必须确实完成延迟计时。
        assert!(delay_elapsed > 0.0);
        // 创建下一帧声明，只更新提示文字并保持延迟配置不变。
        let next = Spin::new()
            // 保持同一延迟配置。
            .delay(Duration::from_secs(1))
            // 更新声明提示文字。
            .tip("仍在加载");
        // 按组件树协调协议刷新声明字段。
        current.sync_from(next);
        // 声明刷新后动画相位必须继续保留。
        assert_eq!(current.phase(), phase);
        // 未改变延迟时计时进度必须继续保留。
        assert_eq!(current.delay_elapsed, delay_elapsed);
        // 声明字段仍必须更新为新的提示文字。
        assert_eq!(current.tip, "仍在加载");
    }
