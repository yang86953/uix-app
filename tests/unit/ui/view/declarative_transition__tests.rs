// 引入当前模块全部私有实现。
use super::*;
// 引入最小文本节点构造器。
use crate::ui::label;

// 构造零延迟线性播放配置。
const fn spec(duration: f64) -> UixTransitionSpec {
    // 返回确定性测试配置。
    UixTransitionSpec {
        // 使用调用方时长。
        duration,
        // 测试不使用延迟。
        delay: 0.0,
        // 使用线性曲线避免额外采样语义。
        easing: Easing::Linear,
    }
}

// 验证首次挂载保持目标值且后续目标变化复用同一 source 原位重定向。
#[test]
fn declarative_transition_retargets_without_animating_first_mount() {
    // 构造首次固定宽度目标。
    let first = label("过渡").width(20.0);
    // 从首次最终样式创建静止状态。
    let state = uix_transition_state(&first, &[UixTransitionProperty::Width])
        // 合法固定宽度必须成功。
        .expect("固定宽度应建立 transition 状态");
    // 首次应用不得启动或改变目标值。
    let mounted = uix_apply_transition(first, &state, spec(1.0))
        // 首次目标应合法。
        .expect("首次 transition 应用应成功");
    // 首次挂载保持最终宽度。
    assert_eq!(mounted.style.width, Some(20.0));
    // 构造同身份 reconcile 的新目标。
    let next = label("过渡").width(80.0);
    // 非零时长 retarget 后当前帧仍从旧值开始。
    let transitioning = uix_apply_transition(next, &state, spec(1.0))
        // 新固定目标应合法。
        .expect("变化目标应原位重定向");
    // 当前帧保留旧值，后续由窗口帧推进。
    assert_eq!(transitioning.style.width, Some(20.0));
    // 再次重定向到即时目标以证明复用同一持久化状态。
    let final_view = uix_apply_transition(label("过渡").width(40.0), &state, spec(0.0))
        // 即时 retarget 应成功。
        .expect("即时目标应复用同一 source");
    // 零时长立即提交最新目标。
    assert_eq!(final_view.style.width, Some(40.0));
}

// 验证颜色字段只接受具体颜色并能即时重定向。
#[test]
fn declarative_transition_retargets_concrete_colors() {
    // 构造具体前景与背景初值。
    let first = label("颜色")
        // 设置具体前景色。
        .color(Color::from_rgb(10, 20, 30))
        // 设置具体背景色。
        .background_color(Color::from_rgb(30, 20, 10));
    // 同时选择两种颜色字段。
    let state = uix_transition_state(
        // 借用首次目标。
        &first,
        // 传入闭合字段集合。
        &[
            // 前景色字段。
            UixTransitionProperty::Color,
            // 背景色字段。
            UixTransitionProperty::BackgroundColor,
        ],
    )
    // 两种具体颜色都应成功。
    .expect("具体颜色应建立 transition 状态");
    // 构造新的具体颜色目标并使用零时长提交。
    let applied = uix_apply_transition(
        // 生成新目标节点。
        label("颜色")
            // 设置新前景色。
            .color(Color::from_rgb(100, 110, 120))
            // 设置新背景色。
            .background_color(Color::from_rgb(120, 110, 100)),
        // 复用持久化状态。
        &state,
        // 立即提交便于确定断言。
        spec(0.0),
    )
    // 具体目标重定向应成功。
    .expect("具体颜色应原位重定向");
    // 前景色必须更新到新目标。
    assert_eq!(
        applied.style.color,
        ColorValue::Custom(Color::from_rgb(100, 110, 120))
    );
    // 背景色必须更新到新目标。
    assert_eq!(
        applied.style.background,
        Some(ColorValue::Custom(Color::from_rgb(120, 110, 100)))
    );
}

// 验证缺少固定尺寸时返回受控错误而非静默跳过。
#[test]
fn declarative_transition_rejects_missing_fixed_dimensions() {
    // 自然尺寸文本没有显式宽度。
    let view = label("自然宽度");
    // 请求宽度 transition 必须失败。
    let error = uix_transition_state(&view, &[UixTransitionProperty::Width])
        // 缺失宽度不能伪装支持。
        .expect_err("自然宽度不能建立固定宽度过渡");
    // 返回稳定闭合错误。
    assert_eq!(error, UixTransitionError::MissingWidth);
}
