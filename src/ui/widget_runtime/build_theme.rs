//! 视图构建期的窗口主题作用域。
//!
//! `.uix` 与 Rust 构建闭包读取主题 token 时没有树引用可用；协调入口在
//! 捕获边界内安装所属树的当前令牌快照，让构建期读取与绘制期
//! `ctx.tokens()` 使用同一窗口主题。优先级与测量期一致：
//! 局部 Provider 主题 > 窗口主题 > 默认亮色。

use std::cell::RefCell;
use std::sync::{Arc, OnceLock};

use crate::ui::ThemeTokens;
use crate::ui::theme::Theme;

thread_local! {
    static BUILD_THEME: RefCell<Option<Arc<dyn ThemeTokens>>> = const { RefCell::new(None) };
}

// 构建可以嵌套（组件根内作用域重建）；退出时恢复外层窗口主题，不跨树污染。
pub(crate) struct BuildThemeScope(pub(crate) Option<Arc<dyn ThemeTokens>>);

impl BuildThemeScope {
    pub(crate) fn enter(theme: Arc<dyn ThemeTokens>) -> Self {
        Self(BUILD_THEME.with(|current| current.replace(Some(theme))))
    }
}

impl Drop for BuildThemeScope {
    fn drop(&mut self) {
        BUILD_THEME.with(|current| current.replace(self.0.take()));
    }
}

// 脱离窗口的构建调用使用稳定默认值，不逐次构建主题。
fn default_build_tokens() -> &'static Arc<dyn ThemeTokens> {
    static DEFAULT_THEME: OnceLock<Arc<dyn ThemeTokens>> = OnceLock::new();
    DEFAULT_THEME.get_or_init(|| Theme::light().tokens_arc())
}

/// 返回当前构建期有效主题令牌：局部 Provider 主题优先，其次窗口主题，
/// 脱离窗口的构建使用稳定默认亮色。
pub fn uix_effective_build_tokens() -> Arc<dyn ThemeTokens> {
    if let Some(theme) = crate::ui::use_context::<crate::ui::StyleScope>().theme {
        return theme.tokens_arc();
    }
    BUILD_THEME
        .with(|current| current.borrow().clone())
        .unwrap_or_else(|| default_build_tokens().clone())
}

#[cfg(test)]
#[path = "../../../tests-src/ui/widget_runtime/build_theme_tests.rs"]
mod tests;
