//! Provider 组件的 View 集成（System 私有边界契约的 DSL 面）。
//!
//! # SMC 边界（SMC-04）
//!
//! `ConfigProvider` / `LocaleProvider` 是 DSL 层注入组件（为一个 View 子树
//! 注入配置/语言）：归 view Module，组件侧只保留纯配置/语言数据与访问器，
//! 依赖方向为 `view → component`，消除 component → view 反向边。
//! 空态渲染回调（`EmptyRenderer`）因跨 component/widgets 消费归 System
//! 私有边界（`render_handler`）。

use crate::platform::windowing::ControlSize;
use crate::ui::component::config::{
    use_config, with_config, ComponentConfig, ComponentOverrides, ComponentTokenOverrides,
};
use crate::ui::component::locale::{en_us, with_locale, zh_cn, Locale};
use crate::ui::component::traits::WidgetComponent;
use crate::ui::render_handler::{EmptyContext, EmptyRenderer};
use crate::ui::theme::{Theme, TokenPatch};
use crate::ui::view::{View, ViewNode};

#[derive(Clone, Default)]
struct ConfigPatch {
    size: Option<ControlSize>,
    disabled: Option<bool>,
    theme: Option<Theme>,
    overrides: Option<ComponentOverrides>,
    component_tokens: ComponentTokenOverrides,
    empty_renderer: Option<EmptyRenderer>,
}

impl ConfigPatch {
    fn resolve(self) -> ComponentConfig {
        let mut config = use_config();
        if let Some(size) = self.size {
            config.size = size;
        }
        if let Some(disabled) = self.disabled {
            config.disabled = disabled;
        }
        if let Some(theme) = self.theme {
            config.theme = Some(theme);
        }
        if let Some(overrides) = self.overrides {
            config.overrides = overrides;
        }
        config.component_tokens.extend(self.component_tokens);
        if let Some(renderer) = self.empty_renderer {
            config.empty_renderer = Some(renderer);
        }
        config
    }
}

#[doc(hidden)]
pub struct MissingConfigProviderChild;

/// 为一个 View 子树注入组件默认配置。
pub struct ConfigProvider<F = MissingConfigProviderChild> {
    patch: ConfigPatch,
    child: F,
}

impl ConfigProvider<MissingConfigProviderChild> {
    pub fn new() -> Self {
        Self {
            patch: ConfigPatch::default(),
            child: MissingConfigProviderChild,
        }
    }
}

impl Default for ConfigProvider<MissingConfigProviderChild> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F> ConfigProvider<F> {
    pub fn component_size(mut self, size: ControlSize) -> Self {
        self.patch.size = Some(size);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.patch.disabled = Some(disabled);
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.patch.theme = Some(theme);
        self
    }

    pub fn overrides(mut self, overrides: ComponentOverrides) -> Self {
        self.patch.overrides = Some(overrides);
        self
    }

    pub fn component_tokens<T: WidgetComponent>(mut self, patch: TokenPatch) -> Self {
        self.patch.component_tokens.insert::<T>(patch);
        self
    }

    pub fn render_empty<R, V>(mut self, renderer: R) -> Self
    where
        R: Fn(EmptyContext) -> V + Send + Sync + 'static,
        V: View,
    {
        self.patch.empty_renderer = Some(EmptyRenderer::new(renderer));
        self
    }

    pub fn child<G, V>(self, child: G) -> ConfigProvider<impl FnOnce() -> ViewNode>
    where
        G: FnOnce() -> V + 'static,
        V: View,
    {
        ConfigProvider {
            patch: self.patch,
            child: move || child().build(),
        }
    }
}

#[doc(hidden)]
pub struct MissingLocaleProviderChild;

/// 为一个 View 子树注入国际化文案。
pub struct LocaleProvider<F = MissingLocaleProviderChild> {
    locale: Locale,
    child: F,
}

impl LocaleProvider<MissingLocaleProviderChild> {
    pub fn new(locale: Locale) -> Self {
        Self {
            locale,
            child: MissingLocaleProviderChild,
        }
    }

    pub fn zh_cn() -> Self {
        Self::new(zh_cn())
    }

    pub fn en_us() -> Self {
        Self::new(en_us())
    }
}

impl<F> LocaleProvider<F> {
    pub fn child<G, V>(self, child: G) -> LocaleProvider<impl FnOnce() -> ViewNode>
    where
        G: FnOnce() -> V + 'static,
        V: View,
    {
        LocaleProvider {
            locale: self.locale,
            child: move || child().build(),
        }
    }
}

impl<F> View for LocaleProvider<F>
where
    F: FnOnce() -> ViewNode + 'static,
{
    fn build(self) -> ViewNode {
        with_locale(&self.locale, self.child)
    }
}

impl<F> View for ConfigProvider<F>
where
    F: FnOnce() -> ViewNode + 'static,
{
    fn build(self) -> ViewNode {
        let config = self.patch.resolve();
        with_config(&config, self.child)
    }
}
