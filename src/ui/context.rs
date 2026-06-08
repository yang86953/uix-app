use crate::ui::theme::{DesignTokens, Theme, TokenProvider};
use std::sync::Arc;

/// Aggregated system references injected into the widget tree.
/// Uses DI pattern — no global singletons.
#[derive(Default, Clone)]
pub struct WidgetContext {
    theme: Option<Theme>,
    dpi_scale: f32,
    frame_requester: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl WidgetContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = Some(theme);
    }

    pub fn theme(&self) -> Option<&Theme> {
        self.theme.as_ref()
    }

    /// Convenience: get the token provider from the current theme.
    pub fn tokens(&self) -> &dyn TokenProvider {
        static DEFAULT_TOKENS: std::sync::LazyLock<DesignTokens> =
            std::sync::LazyLock::new(DesignTokens::antd_light);
        self.theme
            .as_ref()
            .map(|t| t.tokens())
            .unwrap_or(&*DEFAULT_TOKENS)
    }

    pub fn set_dpi_scale(&mut self, scale: f32) {
        self.dpi_scale = scale;
    }

    pub fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }

    pub fn set_frame_requester(&mut self, f: Arc<dyn Fn() + Send + Sync>) {
        self.frame_requester = Some(f);
    }

    pub fn request_frame(&self) {
        if let Some(requester) = &self.frame_requester {
            requester();
        }
    }
}

// Theme is re-exported from the theme module via ui::mod.rs
