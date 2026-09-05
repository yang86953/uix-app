//! 测量期沿用绘制期的窗口主题、子树主题和组件补丁优先级。

use std::any::TypeId;
use std::cell::RefCell;
use std::sync::{Arc, OnceLock};

use crate::ui::ThemeTokens;
use crate::ui::theme::{ScopedThemeTokens, Theme};

thread_local! {
    static LAYOUT_THEME: RefCell<Option<Arc<dyn ThemeTokens>>> = const { RefCell::new(None) };
}

// 布局可以嵌套；退出或展开时恢复原窗口主题，不能污染另一个树。
pub(crate) struct LayoutThemeScope(Option<Arc<dyn ThemeTokens>>);

impl LayoutThemeScope {
    pub(crate) fn enter(theme: Arc<dyn ThemeTokens>) -> Self {
        Self(LAYOUT_THEME.with(|current| current.replace(Some(theme))))
    }
}

impl Drop for LayoutThemeScope {
    fn drop(&mut self) {
        LAYOUT_THEME.with(|current| current.replace(self.0.take()));
    }
}

pub(crate) fn with_measurement_tokens<W: 'static, R>(f: impl FnOnce(&dyn ThemeTokens) -> R) -> R {
    let context = super::provider_context::current_provider_context();
    let config = context.config();
    let theme = config.theme.as_ref().map(Theme::tokens_arc);
    let root = theme
        .clone()
        .or_else(|| LAYOUT_THEME.with(|current| current.borrow().clone()));
    let root = root.unwrap_or_else(|| {
        // 脱离窗口的公开 measure 调用使用稳定默认值，不逐次构建主题。
        static DEFAULT_THEME: OnceLock<Arc<dyn ThemeTokens>> = OnceLock::new();
        Arc::clone(DEFAULT_THEME.get_or_init(|| Theme::antd_light().tokens_arc()))
    });
    let mut tokens = ScopedThemeTokens::new(root);
    tokens.replace_scope(theme, config.widget_tokens.get(TypeId::of::<W>()));
    f(&tokens)
}

// 字号令牌变化会改变测量结果；仅色板变化仍走既有重绘路径。
pub(crate) fn font_sizes_changed(current: &dyn ThemeTokens, next: &dyn ThemeTokens) -> bool {
    use crate::ui::theme::style::TypographyToken;
    [
        TypographyToken::Small,
        TypographyToken::Body,
        TypographyToken::Large,
        TypographyToken::XLarge,
        TypographyToken::Heading1,
        TypographyToken::Heading2,
        TypographyToken::Heading3,
        TypographyToken::Heading4,
        TypographyToken::Heading5,
    ]
    .into_iter()
    // 按位比较让相同的非有限输入也保持稳定，不反复请求布局。
    .any(|token| token.resolve(current).to_bits() != token.resolve(next).to_bits())
}
