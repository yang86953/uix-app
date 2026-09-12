//! Theme wrapper and TokenProvider supertrait — aggregates all token sub-traits.
//!
//! `TokenProvider` combines `IColorTokens`, `ITypographyTokens`,
//! `ISpacingTokens`, and `IBoxShadowTokens` into a single supertrait
//! with the `is_dark()` mode query. `Theme` wraps an `Arc<dyn TokenProvider>`
//! for runtime-polymorphic token injection.

use std::sync::Arc;



use crate::ui::theme::traits::{
    ThemeTokens, TokenProvider,
};

// ════════════════════════════════════════════════════════════════════════════
// TokenProvider — supertrait 聚合全部子 trait
// ════════════════════════════════════════════════════════════════════════════

// TokenProvider trait 定义已迁移至 ui::traits::theme。

// ════════════════════════════════════════════════════════════════════════════
// Theme
// ════════════════════════════════════════════════════════════════════════════

/// Theme wraps a token provider with light/dark mode.
/// Custom themes are injected by providing a `TokenProvider` implementation.
#[derive(Clone)]
pub struct Theme {
    provider: Arc<dyn TokenProvider>,
}

impl Theme {
    /// 通过已有 TokenProvider 创建 Theme（值语义，内部包装为 Arc）。
    pub fn new(provider: impl TokenProvider + 'static) -> Self {
        Self {
            provider: Arc::new(provider),
        }
    }

    /// 通过已有的 `Arc<dyn TokenProvider>` 创建 Theme。
    /// 与 `new()` 的区别在于不会重新包装 Arc，适用于需要共享同一 provider 的场景。
    pub fn from_arc(provider: Arc<dyn TokenProvider>) -> Self {
        Self { provider }
    }

    /// Creates a neutral light theme without component library resources.
    pub fn light() -> Self { Self::new(super::neutral::NeutralTokens(false)) }
    /// Creates a neutral dark theme without component library resources.
    pub fn dark() -> Self { Self::new(super::neutral::NeutralTokens(true)) }

    /// Overrides selected tokens while delegating every other value to this theme.
    pub fn patched(self, patch: super::TokenPatch) -> Self {
        let mut tokens = super::token_patch::ScopedThemeTokens::new(self.tokens_arc());
        tokens.replace_scope(None, Some(Arc::new(patch)));
        Self::new(tokens)
    }

    /// 返回当前主题使用的令牌提供器。
    pub fn tokens(&self) -> &dyn TokenProvider {
        &*self.provider
    }

    pub(crate) fn tokens_arc(&self) -> Arc<dyn ThemeTokens> {
        self.provider.clone()
    }

    /// 返回当前主题是否采用暗色模式。
    pub fn is_dark(&self) -> bool {
        ThemeTokens::is_dark(self.provider.as_ref())
    }

    pub(crate) fn is_same_provider(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.provider, &other.provider)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
}

impl std::fmt::Debug for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Theme")
            .field("is_dark", &self.is_dark())
            .finish()
    }
}
