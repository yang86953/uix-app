use super::{ThemeTokens, TokenValue};
use crate::draw::Color;
pub(super) struct NeutralTokens(pub bool);
impl ThemeTokens for NeutralTokens {
    fn value(&self, key: &str) -> Option<TokenValue> {
        match key {
            "uix.dark" => Some(TokenValue::Boolean(self.0)),
            "uix.foreground" => Some(TokenValue::Color(if self.0 {
                Color::white()
            } else {
                Color::black()
            })),
            "uix.background" => Some(TokenValue::Color(if self.0 {
                Color::black()
            } else {
                Color::white()
            })),
            _ => None,
        }
    }
    fn layout_fingerprint(&self) -> Option<u64> {
        Some(0)
    }
}
