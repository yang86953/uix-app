//! `ui/widget_runtime/build_theme.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;
use crate::ui::theme::Theme;
use crate::ui::widget_runtime::config::{WidgetConfig, with_config};

// 无作用域且无局部 Provider 时使用稳定默认亮色。
#[test]
fn defaults_to_light_without_scope_or_provider() {
    let config = WidgetConfig::default();
    let tokens = with_config(&config, uix_effective_build_tokens);
    assert!(!tokens.is_dark());
}

// 窗口作用域存在且无局部 Provider 时读取窗口主题。
#[test]
fn window_scope_tokens_apply_without_provider() {
    let config = WidgetConfig::default();
    let _scope = BuildThemeScope::enter(Theme::antd_dark().tokens_arc());
    let tokens = with_config(&config, uix_effective_build_tokens);
    assert!(tokens.is_dark());
}

// 局部 Provider 主题优先于窗口作用域，与绘制期叠加次序一致。
#[test]
fn provider_theme_overrides_window_scope() {
    let _scope = BuildThemeScope::enter(Theme::antd_dark().tokens_arc());
    let config = WidgetConfig {
        theme: Some(Theme::antd_light()),
        ..WidgetConfig::default()
    };
    let tokens = with_config(&config, uix_effective_build_tokens);
    assert!(!tokens.is_dark());
}
