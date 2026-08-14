use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

use crate::platform::windowing::ControlSize;
use crate::ui::component::traits::WidgetComponent;
pub use crate::ui::render_handler::{EmptyContext, EmptyRenderer, render_empty_for};
use crate::ui::theme::style::StyleSet;
use crate::ui::theme::{Theme, TokenPatch};

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
    /// 按组件类型应用的设计令牌补丁。
    pub component_tokens: ComponentTokenOverrides,
    /// 数据组件为空时使用的 View factory。
    pub empty_renderer: Option<EmptyRenderer>,
}

impl PartialEq for ComponentConfig {
    fn eq(&self, other: &Self) -> bool {
        self.size == other.size
            && self.disabled == other.disabled
            && self.overrides == other.overrides
            && self.component_tokens == other.component_tokens
            && match (&self.empty_renderer, &other.empty_renderer) {
                (Some(left), Some(right)) => left.is_same_renderer(right),
                (None, None) => true,
                _ => false,
            }
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
            component_tokens: ComponentTokenOverrides::default(),
            empty_renderer: None,
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

    pub fn component_tokens<T: WidgetComponent>(mut self, patch: TokenPatch) -> Self {
        self.component_tokens.insert::<T>(patch);
        self
    }
}

/// 应用级组件默认配置。
pub type Config = ComponentConfig;

pub(crate) const fn control_height(size: ControlSize) -> f32 {
    match size {
        ControlSize::Small => 24.0,
        ControlSize::Medium => 32.0,
        ControlSize::Large => 40.0,
    }
}

/// 按组件类型索引的设计令牌补丁集合。
#[derive(Clone, Default, PartialEq)]
pub struct ComponentTokenOverrides {
    patches: HashMap<TypeId, Arc<TokenPatch>>,
}

impl ComponentTokenOverrides {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<T: WidgetComponent>(&mut self, patch: TokenPatch) {
        self.patches.insert(TypeId::of::<T>(), Arc::new(patch));
    }

    pub fn with<T: WidgetComponent>(mut self, patch: TokenPatch) -> Self {
        self.insert::<T>(patch);
        self
    }

    pub(crate) fn get(&self, component: TypeId) -> Option<Arc<TokenPatch>> {
        self.patches.get(&component).cloned()
    }

    pub(crate) fn extend(&mut self, other: Self) {
        self.patches.extend(other.patches);
    }
}

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

/// 表单布局模式（Form / FormItem 与组件配置共用同一类型）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormLayout {
    Horizontal,
    Vertical,
    Inline,
}

/// 获取当前生效的组件配置。
pub fn use_config() -> ComponentConfig {
    crate::ui::component::provider_context::current_provider_context().config
}

/// 在作用域内使用指定配置执行闭包。
pub fn with_config<T>(config: &ComponentConfig, f: impl FnOnce() -> T) -> T {
    crate::ui::component::provider_context::with_component_config(config, f)
}
