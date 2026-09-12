use std::any::TypeId;
use std::fmt;

use crate::core::WidgetId;
mod source;
mod payload;
pub use payload::{SnapshotModel, WidgetSnapshotFields};

pub use source::SnapshotSource;

fn non_empty(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}

#[derive(Debug, Clone, PartialEq)]
/// 单个组件在捕获时刻的类型化配置与派生语义快照。
pub struct WidgetConfigSnapshot {
    /// 组件在当前组件树中的稳定标识。
    pub id: WidgetId,
    /// 组件具体 Rust 类型的运行时标识。
    pub widget_type: TypeId,
    /// 组件类型专属的可比较配置字段。
    pub fields: WidgetSnapshotFields,
    #[doc(hidden)]
    pub accessibility_override: Option<AccessibilitySnapshot>,
}

impl WidgetConfigSnapshot {
    /// 从组件当前状态捕获配置字段并创建快照。
    pub fn from_widget(
        id: WidgetId,
        widget: &dyn crate::ui::widget_runtime::traits::Widget,
    ) -> Self {
        let fields = widget.snapshot_fields();
        Self::from_widget_fields(id, widget, fields)
    }

    pub(crate) fn from_widget_fields(
        id: WidgetId,
        widget: &dyn crate::ui::widget_runtime::traits::Widget,
        fields: WidgetSnapshotFields,
    ) -> Self {
        Self {
            id,
            widget_type: widget.as_any().type_id(),
            fields,
            accessibility_override: None,
        }
    }

    pub(crate) fn with_accessibility(mut self, accessibility: AccessibilitySnapshot) -> Self {
        self.accessibility_override = Some(accessibility);
        self
    }

    /// 返回显式无障碍覆写与组件默认语义合并后的快照。
    pub fn accessibility(&self) -> AccessibilitySnapshot {
        self.accessibility_override
            .clone()
            .unwrap_or_else(|| self.fields.accessibility())
    }

    /// 返回选择类组件的统一选择状态；非选择组件返回 `None`。
    pub fn selection(&self) -> Option<SelectionSnapshot> {
        self.fields.selection()
    }

    /// 返回该组件当前语义角色对应的 ARIA 规范名称。
    pub fn aria_role(&self) -> Option<&'static str> {
        self.accessibility().aria_role()
    }

    /// 生成该组件当前语义状态对应的 ARIA 属性集合。
    pub fn aria_attributes(&self) -> Vec<AriaAttribute> {
        self.accessibility().aria_attributes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// 选择类组件的统一、可比较运行状态。
pub struct SelectionSnapshot {
    /// 按组件声明顺序保存的候选项显示文本。
    pub options: Vec<String>,
    /// 当前选中候选项在 [`Self::options`] 中的索引。
    pub selected_indices: Vec<usize>,
    /// 当前禁用候选项在 [`Self::options`] 中的索引。
    pub disabled_indices: Vec<usize>,
    /// 是否允许同时选中多个候选项。
    pub multiple: bool,
    /// 候选项弹层或树形列表当前是否展开。
    pub expanded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 组件对无障碍与自动化消费者公开的稳定语义角色。
pub enum AccessibilityRole {
    /// 明确排除当前节点及其子树的无障碍角色。
    None,
    /// 没有更具体交互语义的通用容器角色。
    Generic,
    /// 需要立即播报的重要提示角色。
    Alert,
    /// 可触发单次动作的按钮角色。
    Button,
    /// 表示二态或混合选中状态的复选框角色。
    Checkbox,
    /// 组合输入框与可展开候选列表的角色。
    Combobox,
    /// 阻断或承载独立交互上下文的对话框角色。
    Dialog,
    /// 为相关控件提供语义分组的角色。
    Group,
    /// 表示内容层级标题的角色。
    Heading,
    /// 表示具有替代文本的图片角色。
    Image,
    /// 表示同类条目集合的列表角色。
    List,
    /// 表示动作或导航选项集合的菜单角色。
    Menu,
    /// 表示页面或应用导航区域的角色。
    Navigation,
    /// 表示有界任务进度值的角色。
    ProgressBar,
    /// 表示互斥单选项集合的角色。
    RadioGroup,
    /// 表示内容或控件分隔边界的角色。
    Separator,
    /// 表示在连续范围内选择值的滑块角色。
    Slider,
    /// 表示可逐步增减数值的输入角色。
    SpinButton,
    /// 表示应礼貌播报的当前状态角色。
    Status,
    /// 表示即时生效开关状态的角色。
    Switch,
    /// 表示行列结构数据的表格角色。
    Table,
    /// 表示一组可切换页签的角色。
    TabList,
    /// 表示普通只读文字内容的角色。
    Text,
    /// 表示可编辑文字输入的角色。
    TextBox,
    /// 表示可展开层级节点集合的树角色。
    Tree,
}

impl AccessibilityRole {
    /// ARIA 规范角色名（WAI-ARIA 拼写，如 `progressbar`/`spinbutton`/`tablist`）。
    ///
    /// 与 `automation_name`（snake_case 自动化/协议稳定名）是两套不同命名：
    /// 本方法输出 ARIA 规范名用于 Web 语义；自动化快照与 Agent 线协议使用
    /// `automation_name`。
    pub fn aria_role(self) -> Option<&'static str> {
        match self {
            Self::None | Self::Generic | Self::Text => None,
            Self::Alert => Some("alert"),
            Self::Button => Some("button"),
            Self::Checkbox => Some("checkbox"),
            Self::Combobox => Some("combobox"),
            Self::Dialog => Some("dialog"),
            Self::Group => Some("group"),
            Self::Heading => Some("heading"),
            Self::Image => Some("img"),
            Self::List => Some("list"),
            Self::Menu => Some("menu"),
            Self::Navigation => Some("navigation"),
            Self::ProgressBar => Some("progressbar"),
            Self::RadioGroup => Some("radiogroup"),
            Self::Separator => Some("separator"),
            Self::Slider => Some("slider"),
            Self::SpinButton => Some("spinbutton"),
            Self::Status => Some("status"),
            Self::Switch => Some("switch"),
            Self::Table => Some("table"),
            Self::TabList => Some("tablist"),
            Self::TextBox => Some("textbox"),
            Self::Tree => Some("tree"),
        }
    }

    /// 自动化/线协议稳定角色名（snake_case，如 `progress_bar`/`spin_button`/`tab_list`）。
    ///
    /// 供自动化快照（`ui::automation`，test-harness feature）与 Agent 协议
    /// （`agent_protocol::wire`，agent-control feature）共用，是角色名的单一
    /// 事实来源；与 [`Self::aria_role`] 的 ARIA 规范拼写（`progressbar` 等）
    /// 不同，两套命名各自稳定，勿互相替换。
    #[cfg_attr(
        not(any(test, feature = "test-harness", feature = "agent-control")),
        allow(dead_code)
    )]
    pub(crate) fn automation_name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Generic => "generic",
            Self::Alert => "alert",
            Self::Button => "button",
            Self::Checkbox => "checkbox",
            Self::Combobox => "combobox",
            Self::Dialog => "dialog",
            Self::Group => "group",
            Self::Heading => "heading",
            Self::Image => "image",
            Self::List => "list",
            Self::Menu => "menu",
            Self::Navigation => "navigation",
            Self::ProgressBar => "progress_bar",
            Self::RadioGroup => "radio_group",
            // 主题分隔线使用稳定的自动化角色名称。
            Self::Separator => "separator",
            Self::Slider => "slider",
            Self::SpinButton => "spin_button",
            Self::Status => "status",
            Self::Switch => "switch",
            Self::Table => "table",
            Self::TabList => "tab_list",
            Self::Text => "text",
            Self::TextBox => "text_box",
            Self::Tree => "tree",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
/// 无障碍角色在捕获时刻的通用交互与取值状态。
pub struct AccessibilityState {
    /// 当前控件是否禁止交互。
    pub disabled: bool,
    /// 可选的二态选中状态；不适用时为 `None`。
    pub checked: Option<bool>,
    /// 可选的展开状态；不适用时为 `None`。
    pub expanded: Option<bool>,
    /// 可选的条目选择状态；不适用时为 `None`。
    pub selected: Option<bool>,
    /// 面向用户的当前值文本。
    pub value_text: Option<String>,
    /// 当前数值；不适用或不可公开时为 `None`。
    pub value_now: Option<f64>,
    /// 当前数值范围的下界。
    pub value_min: Option<f64>,
    /// 当前数值范围的上界。
    pub value_max: Option<f64>,
    /// 文本输入是否支持多行内容。
    pub multiline: bool,
    /// 文本输入是否承载敏感密码值。
    pub password: bool,
    /// 当前控件是否要求用户提供值。
    pub required: bool,
}

impl AccessibilityState {
    /// 创建仅设置禁用标志、其余字段使用默认值的状态。
    pub fn disabled(disabled: bool) -> Self {
        Self {
            disabled,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// 一个名称稳定、值已序列化的 ARIA 属性。
pub struct AriaAttribute {
    /// ARIA 规范属性名。
    pub name: &'static str,
    /// 属性的字符串表示值。
    pub value: String,
}

impl AriaAttribute {
    /// 使用静态属性名和可转换字符串的值创建属性。
    pub fn new(name: &'static str, value: impl Into<String>) -> Self {
        Self {
            name,
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
/// 单个组件合并后的无障碍角色、名称、状态与扩展属性。
pub struct AccessibilitySnapshot {
    /// 组件当前公开的稳定语义角色。
    pub role: AccessibilityRole,
    /// 可选的非空无障碍名称。
    pub name: Option<String>,
    /// 角色当前公开的交互与取值状态。
    pub state: AccessibilityState,
    /// 调用方提供、可覆盖派生结果的扩展 ARIA 属性。
    pub attributes: Vec<AriaAttribute>,
}

impl AccessibilitySnapshot {
    /// 使用指定角色和空名称、默认状态创建快照。
    pub fn new(role: AccessibilityRole) -> Self {
        Self {
            role,
            name: None,
            state: AccessibilityState::default(),
            attributes: Vec::new(),
        }
    }

    /// 使用指定角色和名称创建快照；空名称会归一化为 `None`。
    pub fn named(role: AccessibilityRole, name: impl Into<String>) -> Self {
        Self {
            role,
            name: non_empty(name.into()),
            state: AccessibilityState::default(),
            attributes: Vec::new(),
        }
    }

    /// 替换快照的通用交互与取值状态。
    pub fn with_state(mut self, state: AccessibilityState) -> Self {
        self.state = state;
        self
    }

    /// 按名称插入或替换一个扩展 ARIA 属性。
    pub fn with_attribute(mut self, attribute: AriaAttribute) -> Self {
        self.attributes.retain(|item| item.name != attribute.name);
        self.attributes.push(attribute);
        self
    }

    /// 依次插入或替换一组扩展 ARIA 属性。
    pub fn with_attributes(mut self, attributes: impl IntoIterator<Item = AriaAttribute>) -> Self {
        for attribute in attributes {
            self = self.with_attribute(attribute);
        }
        self
    }

    /// 返回当前角色对应的 ARIA 规范名称。
    pub fn aria_role(&self) -> Option<&'static str> {
        self.role.aria_role()
    }

    /// 生成名称和状态属性，并用同名扩展属性覆盖派生值。
    pub fn aria_attributes(&self) -> Vec<AriaAttribute> {
        let mut attributes = Vec::new();

        if let Some(name) = self.name.as_ref() {
            attributes.push(AriaAttribute::new("aria-label", name.clone()));
        }
        if self.state.disabled {
            attributes.push(AriaAttribute::new("aria-disabled", "true"));
        }
        if let Some(checked) = self.state.checked {
            attributes.push(AriaAttribute::new("aria-checked", checked.to_string()));
        }
        if let Some(expanded) = self.state.expanded {
            attributes.push(AriaAttribute::new("aria-expanded", expanded.to_string()));
        }
        if let Some(selected) = self.state.selected {
            attributes.push(AriaAttribute::new("aria-selected", selected.to_string()));
        }
        if let Some(value) = self.state.value_now {
            attributes.push(AriaAttribute::new("aria-valuenow", value.to_string()));
        }
        if let Some(value) = self.state.value_min {
            attributes.push(AriaAttribute::new("aria-valuemin", value.to_string()));
        }
        if let Some(value) = self.state.value_max {
            attributes.push(AriaAttribute::new("aria-valuemax", value.to_string()));
        }
        if let Some(value) = self.state.value_text.as_ref() {
            attributes.push(AriaAttribute::new("aria-valuetext", value.clone()));
        }
        if self.state.multiline {
            attributes.push(AriaAttribute::new("aria-multiline", "true"));
        }
        if self.state.required {
            attributes.push(AriaAttribute::new("aria-required", "true"));
        }

        for attribute in &self.attributes {
            if let Some(existing) = attributes
                .iter_mut()
                .find(|existing| existing.name == attribute.name)
            {
                *existing = attribute.clone();
            } else {
                attributes.push(attribute.clone());
            }
        }

        attributes
    }
}

#[derive(Debug, Clone, PartialEq)]
/// 自定义组件快照中的一个具名诊断字段。
pub struct SnapshotField {
    /// 字段在组件快照契约中的稳定名称。
    pub name: &'static str,
    /// 字段捕获时刻的可比较值。
    pub value: SnapshotValue,
}

impl SnapshotField {
    /// 通过 `Debug` 表示捕获任意字段值。
    pub fn debug<T: fmt::Debug>(name: &'static str, value: &T) -> Self {
        Self {
            name,
            value: SnapshotValue::Debug(format!("{value:?}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// 自定义组件字段可保存的快照值。
pub enum SnapshotValue {
    /// 字段通过 Rust `Debug` 格式捕获的文本值。
    Debug(String),
}

// 无障碍快照与角色双命名映射专项测试。
