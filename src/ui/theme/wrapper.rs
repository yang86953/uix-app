//! Theme wrapper and TokenProvider supertrait — aggregates all token sub-traits.
//!
//! `TokenProvider` combines `IColorTokens`, `ITypographyTokens`,
//! `ISpacingTokens`, and `IBoxShadowTokens` into a single supertrait
//! with the `is_dark()` mode query. `Theme` wraps an `Arc<dyn TokenProvider>`
//! for runtime-polymorphic token injection.

use std::sync::Arc;

use super::color_tokens::IColorTokens;
use super::spacing_tokens::{IBoxShadowTokens, ISpacingTokens};
use super::typography_tokens::ITypographyTokens;

// ════════════════════════════════════════════════════════════════════════════
// TokenProvider — supertrait 聚合全部子 trait
// ════════════════════════════════════════════════════════════════════════════

/// Abstract design token provider — full Ant Design 5 token surface.
///
/// Aggregates domain-specific sub-traits via supertrait bounds.
/// Implement this trait (or implement all sub-traits separately) to inject
/// a custom design system. All color and shadow methods require explicit
/// implementation; typography, spacing, motion, and breakpoint methods
/// provide Ant Design 5 light mode defaults.
pub trait TokenProvider:
    IColorTokens + ITypographyTokens + ISpacingTokens + IBoxShadowTokens + Send + Sync
{
    /// Whether the active theme is dark mode.
    fn is_dark(&self) -> bool {
        false
    }
}

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
    pub fn new(provider: impl TokenProvider + 'static) -> Self {
        Self {
            provider: Arc::new(provider),
        }
    }

    pub fn antd_light() -> Self {
        Self::new(super::design_tokens::DesignTokens::antd_light())
    }

    pub fn antd_dark() -> Self {
        Self::new(super::design_tokens::DesignTokens::antd_dark())
    }

    pub fn tokens(&self) -> &dyn TokenProvider {
        &*self.provider
    }

    pub fn is_dark(&self) -> bool {
        self.provider.is_dark()
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::antd_light()
    }
}

impl std::fmt::Debug for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Theme")
            .field("is_dark", &self.is_dark())
            .finish()
    }
}
