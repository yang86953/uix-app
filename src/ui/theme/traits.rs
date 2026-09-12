//! Design-system-independent theme values.
use super::style::BoxShadowDef;
use crate::draw::Color;
#[derive(Debug, Clone, PartialEq)]
pub enum TokenValue {
    Color(Color),
    Number(f32),
    Text(&'static str),
    Boolean(bool),
    Shadows(Vec<BoxShadowDef>),
}
/// Names and value meanings belong to the registering library/application.
pub trait ThemeTokens: Send + Sync {
    fn value(&self, name: &str) -> Option<TokenValue>;
    fn is_dark(&self) -> bool {
        matches!(self.value("uix.dark"), Some(TokenValue::Boolean(true)))
    }
    fn color(&self, name: &str, fallback: Color) -> Color {
        match self.value(name) {
            Some(TokenValue::Color(value)) => value,
            _ => fallback,
        }
    }
    fn number(&self, name: &str, fallback: f32) -> f32 {
        match self.value(name) {
            Some(TokenValue::Number(value)) => value,
            _ => fallback,
        }
    }
    fn text(&self, name: &str, fallback: &'static str) -> &'static str {
        match self.value(name) {
            Some(TokenValue::Text(value)) => value,
            _ => fallback,
        }
    }
    /// None requests conservative layout invalidation when the theme is replaced.
    fn layout_fingerprint(&self) -> Option<u64> {
        None
    }
}
pub trait TokenProvider: ThemeTokens {}
impl<T: ThemeTokens + ?Sized> TokenProvider for T {}
impl<T: ThemeTokens + ?Sized> ThemeTokens for std::sync::Arc<T> {
    fn value(&self, name: &str) -> Option<TokenValue> {
        self.as_ref().value(name)
    }
    fn is_dark(&self) -> bool {
        self.as_ref().is_dark()
    }
    fn layout_fingerprint(&self) -> Option<u64> {
        self.as_ref().layout_fingerprint()
    }
}
