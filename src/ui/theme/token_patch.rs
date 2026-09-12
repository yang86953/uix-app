//! Immutable token overlays. Value keys and schemas are external to the framework.
use super::{ThemeTokens, TokenValue};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TokenPatch {
    values: BTreeMap<String, TokenValue>,
}
impl TokenPatch {
    pub fn insert(&mut self, key: impl Into<String>, value: TokenValue) {
        self.values.insert(key.into(), value);
    }
    pub fn with(mut self, key: impl Into<String>, value: TokenValue) -> Self {
        self.insert(key, value);
        self
    }
    pub fn get(&self, key: &str) -> Option<&TokenValue> {
        self.values.get(key)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&str, &TokenValue)> {
        self.values.iter().map(|(k, v)| (k.as_str(), v))
    }
}
impl ThemeTokens for TokenPatch {
    fn value(&self, key: &str) -> Option<TokenValue> {
        self.get(key).cloned()
    }
}
pub(crate) struct ScopedThemeTokens {
    root: Arc<dyn ThemeTokens>,
    theme: Option<Arc<dyn ThemeTokens>>,
    patch: Option<Arc<TokenPatch>>,
}
pub(crate) struct TokenScope {
    pub theme: Option<Arc<dyn ThemeTokens>>,
    pub patch: Option<Arc<TokenPatch>>,
}
impl ScopedThemeTokens {
    pub(crate) fn new(root: Arc<dyn ThemeTokens>) -> Self {
        Self {
            root,
            theme: None,
            patch: None,
        }
    }
    pub(crate) fn replace_scope(
        &mut self,
        theme: Option<Arc<dyn ThemeTokens>>,
        patch: Option<Arc<TokenPatch>>,
    ) -> TokenScope {
        TokenScope {
            theme: std::mem::replace(&mut self.theme, theme),
            patch: std::mem::replace(&mut self.patch, patch),
        }
    }
    pub(crate) fn restore_scope(&mut self, scope: TokenScope) {
        self.theme = scope.theme;
        self.patch = scope.patch;
    }
}
impl ThemeTokens for ScopedThemeTokens {
    fn value(&self, key: &str) -> Option<TokenValue> {
        self.patch.as_ref().and_then(|p| p.value(key)).or_else(|| {
            self.theme
                .as_deref()
                .unwrap_or(self.root.as_ref())
                .value(key)
        })
    }
}
