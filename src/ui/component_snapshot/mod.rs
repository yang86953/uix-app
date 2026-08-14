use std::any::TypeId;
use std::fmt;

use crate::core::ComponentId;
use crate::ui::widgets::TransferItem;
// 反馈 capability 启用时才引入气泡确认框专属方位类型。
#[cfg(feature = "feedback")]
// 该类型只服务同步门控的公开快照模型。
use crate::ui::widgets::PopconfirmPlacement;
// 表格 capability 启用时才引入专属列配置类型。
#[cfg(feature = "table")]
// 这些类型仅用于同步门控的公开快照列模型。
use crate::ui::widgets::{Fixed, SortDirection, TableColumn};
// 树组件 capability 启用时才引入树节点公开模型。
#[cfg(feature = "tree-widgets")]
// 该类型只服务同步门控的树快照模型。
use crate::ui::widgets::TreeNode;

mod accessibility;
mod fields;
mod selection;
mod source;

pub use fields::SnapshotFields;
pub use source::{SnapshotSource, snapshot_fields_from_any};

// 反馈 capability 关闭时同步收缩气泡确认框快照模型。
#[cfg(feature = "feedback")]
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotPopconfirm {
    pub title: String,
    pub confirm_text: String,
    pub cancel_text: String,
    pub placement: PopconfirmPlacement,
    pub arrow: bool,
    pub icon: bool,
    pub visible: bool,
    pub focused_action: Option<usize>,
}

fn non_empty(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComponentConfigSnapshot {
    pub id: ComponentId,
    pub widget_type: TypeId,
    pub fields: SnapshotFields,
    #[doc(hidden)]
    pub accessibility_override: Option<AccessibilitySnapshot>,
}

impl ComponentConfigSnapshot {
    pub fn from_component(
        id: ComponentId,
        component: &dyn crate::ui::component::traits::WidgetComponent,
    ) -> Self {
        let fields = component.snapshot_fields();
        Self::from_component_fields(id, component, fields)
    }

    pub(crate) fn from_component_fields(
        id: ComponentId,
        component: &dyn crate::ui::component::traits::WidgetComponent,
        fields: SnapshotFields,
    ) -> Self {
        Self {
            id,
            widget_type: component.as_any().type_id(),
            fields,
            accessibility_override: None,
        }
    }

    pub(crate) fn with_accessibility(mut self, accessibility: AccessibilitySnapshot) -> Self {
        self.accessibility_override = Some(accessibility);
        self
    }

    pub fn accessibility(&self) -> AccessibilitySnapshot {
        self.accessibility_override
            .clone()
            .unwrap_or_else(|| self.fields.accessibility())
    }

    pub fn selection(&self) -> Option<SelectionSnapshot> {
        self.fields.selection()
    }

    pub fn aria_role(&self) -> Option<&'static str> {
        self.accessibility().aria_role()
    }

    pub fn aria_attributes(&self) -> Vec<AriaAttribute> {
        self.accessibility().aria_attributes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionSnapshot {
    pub options: Vec<String>,
    pub selected_indices: Vec<usize>,
    pub disabled_indices: Vec<usize>,
    pub multiple: bool,
    pub expanded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessibilityRole {
    None,
    Generic,
    Alert,
    Button,
    Checkbox,
    Combobox,
    Dialog,
    Group,
    Heading,
    Image,
    List,
    Menu,
    Navigation,
    ProgressBar,
    RadioGroup,
    Separator,
    Slider,
    SpinButton,
    Status,
    Switch,
    Table,
    TabList,
    Text,
    TextBox,
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
pub struct AccessibilityState {
    pub disabled: bool,
    pub checked: Option<bool>,
    pub expanded: Option<bool>,
    pub selected: Option<bool>,
    pub value_text: Option<String>,
    pub value_now: Option<f64>,
    pub value_min: Option<f64>,
    pub value_max: Option<f64>,
    pub multiline: bool,
    pub password: bool,
    pub required: bool,
}

impl AccessibilityState {
    pub fn disabled(disabled: bool) -> Self {
        Self {
            disabled,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AriaAttribute {
    pub name: &'static str,
    pub value: String,
}

impl AriaAttribute {
    pub fn new(name: &'static str, value: impl Into<String>) -> Self {
        Self {
            name,
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AccessibilitySnapshot {
    pub role: AccessibilityRole,
    pub name: Option<String>,
    pub state: AccessibilityState,
    pub attributes: Vec<AriaAttribute>,
}

impl AccessibilitySnapshot {
    pub fn new(role: AccessibilityRole) -> Self {
        Self {
            role,
            name: None,
            state: AccessibilityState::default(),
            attributes: Vec::new(),
        }
    }

    pub fn named(role: AccessibilityRole, name: impl Into<String>) -> Self {
        Self {
            role,
            name: non_empty(name.into()),
            state: AccessibilityState::default(),
            attributes: Vec::new(),
        }
    }

    pub fn with_state(mut self, state: AccessibilityState) -> Self {
        self.state = state;
        self
    }

    pub fn with_attribute(mut self, attribute: AriaAttribute) -> Self {
        self.attributes.retain(|item| item.name != attribute.name);
        self.attributes.push(attribute);
        self
    }

    pub fn with_attributes(mut self, attributes: impl IntoIterator<Item = AriaAttribute>) -> Self {
        for attribute in attributes {
            self = self.with_attribute(attribute);
        }
        self
    }

    pub fn aria_role(&self) -> Option<&'static str> {
        self.role.aria_role()
    }

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
pub struct SnapshotField {
    pub name: &'static str,
    pub value: SnapshotValue,
}

impl SnapshotField {
    pub fn debug<T: fmt::Debug>(name: &'static str, value: &T) -> Self {
        Self {
            name,
            value: SnapshotValue::Debug(format!("{value:?}")),
        }
    }
}

// 树组件 capability 关闭时同步收缩树节点快照模型。
#[cfg(feature = "tree-widgets")]
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotTreeNode {
    pub title: String,
    pub key: String,
    pub icon: String,
    pub children: Vec<SnapshotTreeNode>,
    pub disabled: bool,
    pub checkable: bool,
    pub checked: bool,
    pub draggable: bool,
    pub is_leaf: bool,
}

// 树组件 capability 启用时才提供树节点快照转换。
#[cfg(feature = "tree-widgets")]
impl SnapshotTreeNode {
    pub fn from_tree_node(node: &TreeNode) -> Self {
        Self {
            title: node.title.clone(),
            key: node.key.clone(),
            icon: node.icon.clone(),
            children: node.children.iter().map(Self::from_tree_node).collect(),
            disabled: node.disabled,
            checkable: node.checkable,
            checked: node.checked,
            draggable: node.draggable,
            is_leaf: node.is_leaf,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotCollapsePanel {
    pub header: String,
    pub content: String,
    pub expanded: bool,
}

// 表格 capability 关闭时同步收缩公开列快照模型。
#[cfg(feature = "table")]
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotTableColumn {
    pub title: String,
    pub width: f32,
    pub sortable: bool,
    pub sort_direction: SortDirection,
    pub filterable: bool,
    pub filters: Vec<String>,
    pub fixed: Option<Fixed>,
    pub resizable: bool,
}

// 表格 capability 启用时才提供列配置到快照的转换。
#[cfg(feature = "table")]
impl SnapshotTableColumn {
    pub fn from_table_column(column: &TableColumn) -> Self {
        Self {
            title: column.title.clone(),
            width: column.width,
            sortable: column.sortable,
            sort_direction: column.sort_direction,
            filterable: column.filterable,
            filters: column
                .filters
                .iter()
                .map(|(label, _active)| label.clone())
                .collect(),
            fixed: column.fixed,
            resizable: column.resizable,
        }
    }
}

// 表格 capability 关闭时同步收缩公开列组快照模型。
#[cfg(feature = "table")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotTableColumnGroup {
    pub title: Option<String>,
    pub start: usize,
    pub len: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotTransferItem {
    pub key: String,
    pub title: String,
    pub selected: bool,
}

impl SnapshotTransferItem {
    pub fn from_transfer_item(item: &TransferItem) -> Self {
        Self {
            key: item.key.clone(),
            title: item.title.clone(),
            selected: item.selected,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotValue {
    Debug(String),
}

// 无障碍快照与角色双命名映射专项测试。
#[cfg(test)]
// 从仓库测试目录引入，保持快照实现文件低于行数上限。
#[path = "../../../tests/unit/ui/component_snapshot/accessibility_snapshot_tests.rs"]
// 将外置测试作为快照模块的私有子模块编译。
mod accessibility_snapshot_tests;
