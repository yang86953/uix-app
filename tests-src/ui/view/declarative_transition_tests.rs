//! `ui/view/declarative_transition.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::Theme;
use crate::ui::widget_runtime::build_theme::BuildThemeScope;
use crate::ui::set_reduced_motion;
use crate::ui::widget_runtime::config::{WidgetConfig, with_config};
use crate::ui::widgets::combinators::space;

// 减少动效是进程级开关，相关行为测试串行执行避免互相干扰。
static MOTION_LOCK: Mutex<()> = Mutex::new(());

// 在局部 Provider 主题作用域内执行闭包。
fn with_provider_theme<R>(theme: Theme, f: impl FnOnce() -> R) -> R {
    let config = WidgetConfig {
        theme: Some(theme),
        ..WidgetConfig::default()
    };
    with_config(&config, f)
}

// 创建只声明语义主题背景色的目标节点。
fn themed_node(background: ColorValue) -> ViewNode {
    space(0.0).background_color(background)
}

// 固定线性缓动的播放配置。
fn spec(duration: f64) -> UixTransitionSpec {
    UixTransitionSpec {
        duration,
        delay: 0.0,
        easing: Easing::Linear,
    }
}

fn container_of(theme: &Theme) -> Color {
    theme.tokens().color_bg_container()
}

// 首次挂载：主题 token 目标按当前有效主题静止呈现，零时长直接到目标。
#[test]
fn first_mount_token_target_is_static_at_current_theme() {
    let dark = Theme::antd_dark();
    let view = with_provider_theme(dark.clone(), || {
        let node = themed_node(ColorValue::neutral(NeutralRole::BgContainer));
        let state = uix_transition_state(
            &node,
            &[UixTransitionProperty::BackgroundColor],
        )
        .expect("主题 token 目标应可解析");
        uix_apply_transition(
            themed_node(ColorValue::neutral(NeutralRole::BgContainer)),
            &state,
            spec(0.0),
        )
        .expect("首挂应用应成功")
    });
    // 首挂即静止在暗色主题解析值，不伪装入场过渡。
    assert_eq!(
        view.style.background,
        Some(ColorValue::Custom(container_of(&dark)))
    );
}

// 主题切换：从当前呈现值 retarget 到新主题目标，不从旧起点重放。
#[test]
fn theme_switch_retargets_from_presented_value() {
    let _guard = MOTION_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let dark = Theme::antd_dark();
    let light = Theme::antd_light();
    assert_ne!(
        container_of(&dark),
        container_of(&light),
        "内置亮暗主题主色必须不同"
    );
    // 暗色主题下先零时长到达目标。
    let state = with_provider_theme(dark.clone(), || {
        let node = themed_node(ColorValue::neutral(NeutralRole::BgContainer));
        let state = uix_transition_state(
            &node,
            &[UixTransitionProperty::BackgroundColor],
        )
        .expect("主题 token 目标应可解析");
        uix_apply_transition(
            themed_node(ColorValue::neutral(NeutralRole::BgContainer)),
            &state,
            spec(0.0),
        )
        .expect("首挂应用应成功");
        state
    });
    // 切换亮色主题并给足时长：retarget 当帧仍呈现暗色值。
    let mid_flight = with_provider_theme(light.clone(), || {
        uix_apply_transition(
            themed_node(ColorValue::neutral(NeutralRole::BgContainer)),
            &state,
            spec(60.0),
        )
        .expect("进行中应用应成功")
    });
    assert_eq!(
        mid_flight.style.background,
        Some(ColorValue::Custom(container_of(&dark)))
    );
    // 同目标零时长是立即完成策略：进行中过渡直接收束到目标值
    // （亮色容器色），不因目标相等被跳过。
    let in_flight_again = with_provider_theme(light.clone(), || {
        uix_apply_transition(
            themed_node(ColorValue::neutral(NeutralRole::BgContainer)),
            &state,
            spec(0.0),
        )
        .expect("同目标应用应成功")
    });
    assert_eq!(
        in_flight_again.style.background,
        Some(ColorValue::Custom(container_of(&light)))
    );
    // 收束后状态静止，定时帧请求释放。
    assert!(state.is_settled_for_test());
    // 新主题目标（悬停色）经零时长重定向同样直接落到解析后的目标值。
    let retargeted = with_provider_theme(light.clone(), || {
        uix_apply_transition(
            themed_node(ColorValue::neutral(NeutralRole::BgElevated)),
            &state,
            spec(0.0),
        )
        .expect("重定向应用应成功")
    });
    assert_eq!(
        retargeted.style.background,
        Some(ColorValue::Custom(light.tokens().color_bg_elevated()))
    );
}

// 重复同目标不重启动画：已完成状态保持静止。
#[test]
fn same_target_does_not_restart_animation() {
    let _guard = MOTION_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let dark = Theme::antd_dark();
    let state = with_provider_theme(dark.clone(), || {
        let node = themed_node(ColorValue::neutral(NeutralRole::BgContainer));
        let state = uix_transition_state(
            &node,
            &[UixTransitionProperty::BackgroundColor],
        )
        .expect("主题 token 目标应可解析");
        uix_apply_transition(
            themed_node(ColorValue::neutral(NeutralRole::BgContainer)),
            &state,
            spec(0.0),
        )
        .expect("首挂应用应成功");
        state
    });
    assert!(state.is_settled_for_test());
    // 同目标重新应用长时长：不得产生新的进行中播放。
    let view = with_provider_theme(dark.clone(), || {
        uix_apply_transition(
            themed_node(ColorValue::neutral(NeutralRole::BgContainer)),
            &state,
            spec(30.0),
        )
        .expect("同目标应用应成功")
    });
    assert_eq!(
        view.style.background,
        Some(ColorValue::Custom(container_of(&dark)))
    );
    assert!(state.is_settled_for_test());
}

// 进行中过渡遇到零时长或减少动效的同目标刷新：立即完成策略收束到
// 目标并释放定时帧，不因目标相等跳过。
#[test]
fn in_flight_same_target_finishes_immediately_under_policy() {
    let _guard = MOTION_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let dark = Theme::antd_dark();
    let light = Theme::antd_light();
    // 暗色首挂静止后切亮色主题，开启长时过渡（当帧呈现仍为暗色值）。
    let state = with_provider_theme(dark.clone(), || {
        let node = themed_node(ColorValue::neutral(NeutralRole::BgContainer));
        let state = uix_transition_state(
            &node,
            &[UixTransitionProperty::BackgroundColor],
        )
        .expect("主题 token 目标应可解析");
        uix_apply_transition(
            themed_node(ColorValue::neutral(NeutralRole::BgContainer)),
            &state,
            spec(0.0),
        )
        .expect("首挂应用应成功");
        state
    });
    let mid_flight = with_provider_theme(light.clone(), || {
        uix_apply_transition(
            themed_node(ColorValue::neutral(NeutralRole::BgContainer)),
            &state,
            spec(60.0),
        )
        .expect("进行中应用应成功")
    });
    assert_eq!(
        mid_flight.style.background,
        Some(ColorValue::Custom(container_of(&dark)))
    );
    assert!(!state.is_settled_for_test());
    // 同目标、时长不变，但启用减少动效：立即完成落到亮色目标。
    set_reduced_motion(true);
    let finished = with_provider_theme(light.clone(), || {
        uix_apply_transition(
            themed_node(ColorValue::neutral(NeutralRole::BgContainer)),
            &state,
            spec(60.0),
        )
        .expect("同目标策略应用应成功")
    });
    set_reduced_motion(false);
    assert_eq!(
        finished.style.background,
        Some(ColorValue::Custom(container_of(&light)))
    );
    assert!(state.is_settled_for_test());
}
// 减少动效开启后，主题切换重定向直接落到目标值。
#[test]
fn reduced_motion_jumps_to_target() {
    let _guard = MOTION_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let dark = Theme::antd_dark();
    let light = Theme::antd_light();
    let state = with_provider_theme(dark.clone(), || {
        let node = themed_node(ColorValue::neutral(NeutralRole::BgContainer));
        let state = uix_transition_state(
            &node,
            &[UixTransitionProperty::BackgroundColor],
        )
        .expect("主题 token 目标应可解析");
        uix_apply_transition(
            themed_node(ColorValue::neutral(NeutralRole::BgContainer)),
            &state,
            spec(0.0),
        )
        .expect("首挂应用应成功");
        state
    });
    set_reduced_motion(true);
    let view = with_provider_theme(light.clone(), || {
        let applied = uix_apply_transition(
            themed_node(ColorValue::neutral(NeutralRole::BgContainer)),
            &state,
            spec(30.0),
        )
        .expect("减少动效应用应成功");
        assert!(state.is_settled_for_test());
        applied
    });
    set_reduced_motion(false);
    // 开启后新过渡不占用帧调度，直接呈现亮色目标。
    assert_eq!(
        view.style.background,
        Some(ColorValue::Custom(container_of(&light)))
    );
}

// 窗口主题作用域参与解析：脱离局部 Provider 时按作用域主题取值。
#[test]
fn window_build_scope_resolves_token_targets() {
    let _scope = BuildThemeScope::enter(Theme::antd_dark().tokens_arc());
    let dark_container = Theme::antd_dark().tokens().color_bg_container();
    let view = {
        let node = themed_node(ColorValue::neutral(NeutralRole::BgContainer));
        let state =
            uix_transition_state(&node, &[UixTransitionProperty::BackgroundColor])
                .expect("主题 token 目标应可解析");
        uix_apply_transition(
            themed_node(ColorValue::neutral(NeutralRole::BgContainer)),
            &state,
            spec(0.0),
        )
        .expect("应用应成功")
    };
    assert_eq!(
        view.style.background,
        Some(ColorValue::Custom(dark_container))
    );
}

// 缺失背景颜色保持明确契约拒绝，错误文本可定位字段。
#[test]
fn missing_background_reports_contract_error() {
    let error = uix_transition_state(
        &space(0.0),
        &[UixTransitionProperty::BackgroundColor],
    )
    .expect_err("缺失背景必须报告契约错配");
    assert!(error.to_string().contains("backgroundColor"));
}

// —— 自源文件移入的扩展 impl（impl UixDeclarativeTransition） ——

impl UixDeclarativeTransition {
    // 测试探针：全部字段是否处于静止（无进行中的过渡播放）。
    #[cfg(test)]
    pub(crate) fn is_settled_for_test(&self) -> bool {
        // 中毒锁照常恢复，探针不参与事务语义。
        let values = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        fn settled<T>(slot: &Option<TransitionSlot<T>>) -> bool
        where
            T: crate::ui::animation::traits::Animatable + Sync,
        {
            slot.as_ref().is_none_or(|slot| slot.source.is_finished())
        }
        settled(&values.width)
            && settled(&values.height)
            && settled(&values.border_radius)
            && settled(&values.opacity)
            && settled(&values.color)
            && settled(&values.background_color)
    }
}
