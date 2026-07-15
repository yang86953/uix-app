use crate::native::traits::input::ControlSize;
use crate::ui::style::StyleSet;
use crate::ui::theme::Theme;

/// 组件全局默认配置。
#[derive(Clone)]
pub struct ComponentConfig {
    /// 默认控件尺寸。
    pub size: ControlSize,
    /// 默认禁用状态。
    pub disabled: bool,
    /// 当前主题。
    pub theme: Option<Theme>,
    /// 组件级覆盖。
    pub overrides: ComponentOverrides,
}

impl PartialEq for ComponentConfig {
    fn eq(&self, other: &Self) -> bool {
        self.size == other.size
            && self.disabled == other.disabled
            && self.overrides == other.overrides
            && match (&self.theme, &other.theme) {
                (Some(left), Some(right)) => left.is_same_provider(right),
                (None, None) => true,
                _ => false,
            }
    }
}

impl Default for ComponentConfig {
    fn default() -> Self {
        Self {
            size: ControlSize::Medium,
            disabled: false,
            theme: None,
            overrides: ComponentOverrides::default(),
        }
    }
}

impl ComponentConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn component_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = Some(theme);
        self
    }

    pub fn overrides(mut self, overrides: ComponentOverrides) -> Self {
        self.overrides = overrides;
        self
    }
}

/// 应用级组件默认配置。
pub type Config = ComponentConfig;

/// 组件级属性覆盖。
#[derive(Clone, Default, PartialEq)]
pub struct ComponentOverrides {
    pub button: ButtonOverrides,
    pub input: InputOverrides,
    pub select: SelectOverrides,
    pub form: FormOverrides,
}

#[derive(Clone, Default, PartialEq)]
pub struct ButtonOverrides {
    pub style_set: Option<StyleSet>,
}

#[derive(Clone, Default, PartialEq)]
pub struct InputOverrides {
    pub prefix: Option<String>,
    pub suffix: Option<String>,
}

#[derive(Clone, Default, PartialEq)]
pub struct SelectOverrides {
    pub allow_search: Option<bool>,
}

#[derive(Clone, Default, PartialEq)]
pub struct FormOverrides {
    pub layout: Option<FormLayout>,
}

/// 表单布局模式。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FormLayout {
    Horizontal,
    Vertical,
    Inline,
}

/// 获取当前生效的组件配置。
pub fn use_config() -> ComponentConfig {
    crate::ui::foundation::provider_context::current_provider_context().config
}

/// 在作用域内使用指定配置执行闭包。
pub fn with_config<T>(config: &ComponentConfig, f: impl FnOnce() -> T) -> T {
    crate::ui::foundation::provider_context::with_component_config(config, f)
}

use crate::ui::view::{View, ViewNode};

#[derive(Clone, Default)]
struct ConfigPatch {
    size: Option<ControlSize>,
    disabled: Option<bool>,
    theme: Option<Theme>,
    overrides: Option<ComponentOverrides>,
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

impl<F> View for ConfigProvider<F>
where
    F: FnOnce() -> ViewNode + 'static,
{
    fn build(self) -> ViewNode {
        let config = self.patch.resolve();
        with_config(&config, self.child)
    }
}
