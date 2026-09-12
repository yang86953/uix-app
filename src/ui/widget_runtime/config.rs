use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

use crate::platform::windowing::ControlSize;
pub use crate::ui::render_handler::{EmptyContext, EmptyRenderer, render_empty_for};
use crate::ui::theme::style::StyleSet;
use crate::ui::theme::{Theme, TokenPatch};
use crate::ui::widget_runtime::traits::Widget;

/// 组件全局默认配置。
#[derive(Clone)]
pub struct WidgetConfig {
    /// 默认控件尺寸。
    pub size: ControlSize,
    /// 默认禁用状态。
    pub disabled: bool,
    /// 当前主题。
    pub theme: Option<Theme>,
    /// 组件级覆盖。
    pub overrides: WidgetOverrides,
    /// 按组件类型应用的设计令牌补丁。
    pub widget_tokens: WidgetTokenOverrides,
    /// 数据组件为空时使用的 View factory。
    pub empty_renderer: Option<EmptyRenderer>,
}

impl PartialEq for WidgetConfig {
    fn eq(&self, other: &Self) -> bool {
        self.size == other.size
            && self.disabled == other.disabled
            && self.overrides == other.overrides
            && self.widget_tokens == other.widget_tokens
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

impl Default for WidgetConfig {
    fn default() -> Self {
        Self {
            size: ControlSize::Medium,
            disabled: false,
            theme: None,
            overrides: WidgetOverrides::default(),
            widget_tokens: WidgetTokenOverrides::default(),
            empty_renderer: None,
        }
    }
}

impl WidgetConfig {
    /// 创建中等尺寸、启用交互且不含主题或组件覆盖的默认配置。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置作用域内组件默认使用的标准控件尺寸。
    pub fn widget_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// 设置作用域内组件默认是否禁用交互。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 设置作用域内组件解析令牌时使用的主题 Provider。
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// 替换作用域内全部窄组件属性覆盖。
    pub fn overrides(mut self, overrides: WidgetOverrides) -> Self {
        self.overrides = overrides;
        self
    }

    /// 为指定组件类型插入或替换设计令牌补丁。
    pub fn widget_tokens<T: Widget>(mut self, patch: TokenPatch) -> Self {
        self.widget_tokens.insert::<T>(patch);
        self
    }
}

/// 应用级组件默认配置。
pub type Config = WidgetConfig;

pub const fn control_height(size: ControlSize) -> f32 {
    match size {
        ControlSize::Small => 24.0,
        ControlSize::Medium => 32.0,
        ControlSize::Large => 40.0,
    }
}

/// 按组件类型索引的设计令牌补丁集合。
#[derive(Clone, Default, PartialEq)]
pub struct WidgetTokenOverrides {
    patches: HashMap<TypeId, Arc<TokenPatch>>,
}

impl WidgetTokenOverrides {
    /// 创建不含任何组件类型补丁的集合。
    pub fn new() -> Self {
        Self::default()
    }

    /// 为指定组件类型插入或替换设计令牌补丁。
    pub fn insert<T: Widget>(&mut self, patch: TokenPatch) {
        self.patches.insert(TypeId::of::<T>(), Arc::new(patch));
    }

    /// 消费集合并为指定组件类型插入或替换设计令牌补丁。
    pub fn with<T: Widget>(mut self, patch: TokenPatch) -> Self {
        self.insert::<T>(patch);
        self
    }

    pub(crate) fn get(&self, widget: TypeId) -> Option<Arc<TokenPatch>> {
        self.patches.get(&widget).cloned()
    }

    pub(crate) fn extend(&mut self, other: Self) {
        self.patches.extend(other.patches);
    }
}

/// 组件级属性覆盖。
#[derive(Clone, Default, PartialEq)]
pub struct WidgetOverrides {
    /// 按钮组件使用的默认属性覆盖。
    pub button: ButtonOverrides,
    /// 输入框组件使用的默认属性覆盖。
    pub input: InputOverrides,
    /// 选择器组件使用的默认属性覆盖。
    pub select: SelectOverrides,
    /// 表单组件使用的默认属性覆盖。
    pub form: FormOverrides,
}

/// 按钮组件可从 Provider 继承的默认属性覆盖。
#[derive(Clone, Default, PartialEq)]
pub struct ButtonOverrides {
    /// 替换按钮变体解析结果的可选样式集合。
    pub style_set: Option<StyleSet>,
}

/// 输入框组件可从 Provider 继承的默认属性覆盖。
#[derive(Clone, Default, PartialEq)]
pub struct InputOverrides {
    /// 显示在输入内容前方的可选文本。
    pub prefix: Option<String>,
    /// 显示在输入内容后方的可选文本。
    pub suffix: Option<String>,
}

/// 选择器组件可从 Provider 继承的默认属性覆盖。
#[derive(Clone, Default, PartialEq)]
pub struct SelectOverrides {
    /// 是否允许在选项中搜索；空值保留组件自身默认行为。
    pub allow_search: Option<bool>,
}

/// 表单组件可从 Provider 继承的默认属性覆盖。
#[derive(Clone, Default, PartialEq)]
pub struct FormOverrides {
    /// 表单及表单项使用的可选默认布局模式。
    pub layout: Option<FormLayout>,
}

/// 表单布局模式（Form / FormItem 与组件配置共用同一类型）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormLayout {
    /// 标签和控件沿水平方向并排布局。
    Horizontal,
    /// 标签和控件沿垂直方向堆叠布局。
    Vertical,
    /// 表单项按内容宽度在同一行内排列。
    Inline,
}

/// 获取当前生效的组件配置。
pub fn use_config() -> WidgetConfig {
    crate::ui::widget_runtime::provider_context::current_provider_context()
        .config()
        .clone()
}

/// 在作用域内使用指定配置执行闭包。
pub fn with_config<T>(config: &WidgetConfig, f: impl FnOnce() -> T) -> T {
    crate::ui::widget_runtime::provider_context::with_widget_config(config, f)
}
