use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

use crate::draw::painting::TokenPatch;
use crate::native::traits::input::ControlSize;
use crate::ui::style::StyleSet;
use crate::ui::theme::Theme;
use crate::ui::traits::WidgetComponent;

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

    pub fn render_empty<F, V>(mut self, renderer: F) -> Self
    where
        F: Fn(EmptyContext) -> V + Send + Sync + 'static,
        V: View,
    {
        self.empty_renderer = Some(EmptyRenderer::new(renderer));
        self
    }
}

/// 应用级组件默认配置。
pub type Config = ComponentConfig;

/// 标识正在请求空态 View 的数据组件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmptyContext {
    component_name: &'static str,
}

impl EmptyContext {
    fn of<T: WidgetComponent>() -> Self {
        let full_name = std::any::type_name::<T>();
        Self {
            component_name: full_name.rsplit("::").next().map_or(full_name, |name| name),
        }
    }

    pub fn component_name(self) -> &'static str {
        self.component_name
    }
}

/// 由 `ComponentConfig` 持有的可克隆空态 View factory。
#[derive(Clone)]
pub struct EmptyRenderer {
    renderer: Arc<dyn Fn(EmptyContext) -> ViewNode + Send + Sync>,
}

impl EmptyRenderer {
    pub fn new<F, V>(renderer: F) -> Self
    where
        F: Fn(EmptyContext) -> V + Send + Sync + 'static,
        V: View,
    {
        Self {
            renderer: Arc::new(move |context| renderer(context).build()),
        }
    }

    pub fn render<T: WidgetComponent>(&self) -> ViewNode {
        (self.renderer)(EmptyContext::of::<T>())
    }

    fn is_same_renderer(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.renderer, &other.renderer)
    }
}

/// 为指定组件类型构建当前配置的空态 View。
pub fn render_empty_for<T: WidgetComponent>() -> Option<ViewNode> {
    use_config()
        .empty_renderer
        .map(|renderer| renderer.render::<T>())
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

    fn extend(&mut self, other: Self) {
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

impl<F> View for ConfigProvider<F>
where
    F: FnOnce() -> ViewNode + 'static,
{
    fn build(self) -> ViewNode {
        let config = self.patch.resolve();
        with_config(&config, self.child)
    }
}
