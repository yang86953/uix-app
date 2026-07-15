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

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::ui::{EventResult, SystemEvent, WidgetTree};

component! {
    /// ConfigProvider — 为子树注入全局组件默认配置。
    ///
    /// 所有后代 widget 可通过 `use_config()` 读取当前配置。
    pub struct ConfigProvider {
        config: ComponentConfig,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(0.0, 0.0))
    }

    render => (&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {}

    on_event => (&mut self, _event: &SystemEvent) -> EventResult {
        EventResult::NotHandled
    }
}

impl Default for ConfigProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigProvider {
    pub fn new() -> Self {
        Self {
            config: ComponentConfig::default(),
        }
    }

    pub fn size(mut self, s: ControlSize) -> Self {
        self.config.size = s;
        self
    }

    pub fn disabled(mut self, v: bool) -> Self {
        self.config.disabled = v;
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.config.theme = Some(theme);
        self
    }
}

/// ConfigProvider 专用 WidgetNode 包装，在 build 时注入配置。
/// 使用方式：
/// ```ignore
/// ConfigProvider::new().size(Large).wrap(
///     tree! { Container::new() => [ button("OK").widget() ] }
/// )
/// ```
impl ConfigProvider {
    pub fn wrap(
        self,
        node: crate::ui::core::widget::WidgetNode,
    ) -> crate::ui::core::widget::WidgetNode {
        crate::ui::core::widget::WidgetNode::new(Box::new(self), vec![node])
    }
}
