//! Runtime theme switching independent of a concrete palette.
use super::Theme;
use crate::draw::Color;
use crate::ui::{ThemeTokens, TokenProvider, TokenValue};
use std::sync::atomic::{AtomicBool, Ordering};
#[derive(Debug)]
pub struct ModeTokens {
    light: Theme,
    dark_theme: Theme,
    dark: AtomicBool,
}
impl ModeTokens {
    pub fn new(light: Theme, dark: Theme, is_dark: bool) -> Self {
        Self {
            light,
            dark_theme: dark,
            dark: AtomicBool::new(is_dark),
        }
    }
    pub fn set_mode(&self, dark: bool) {
        self.dark.store(dark, Ordering::Release);
    }
    /// Returns the currently selected immutable theme.
    pub fn snapshot(&self) -> Theme {
        if self.dark.load(Ordering::Acquire) {
            self.dark_theme.clone()
        } else {
            self.light.clone()
        }
    }
    fn active(&self) -> &dyn TokenProvider {
        if self.dark.load(Ordering::Acquire) {
            self.dark_theme.tokens()
        } else {
            self.light.tokens()
        }
    }
}
impl ThemeTokens for ModeTokens {
    fn value(&self, key: &str) -> Option<TokenValue> {
        self.active().value(key)
    }
    fn layout_fingerprint(&self) -> Option<u64> {
        self.active().layout_fingerprint()
    }
}
