use std::any::TypeId;
use std::fmt;

use crate::core::ComponentId;
use crate::ui::widgets::{Fixed, SortDirection, TableColumn, TransferItem, TreeNode};

mod fields;
mod source;

pub use fields::SnapshotFields;
pub use source::{snapshot_fields_from_any, SnapshotSource};

fn non_empty(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComponentConfigSnapshot {
    pub id: ComponentId,
    pub widget_type: TypeId,
    pub fields: SnapshotFields,
}

impl ComponentConfigSnapshot {
    pub fn from_component(
        id: ComponentId,
        component: &dyn crate::ui::traits::WidgetComponent,
    ) -> Self {
        Self {
            id,
            widget_type: component.as_any().type_id(),
            fields: component.snapshot_fields(),
        }
    }

    pub fn accessibility(&self) -> AccessibilitySnapshot {
        self.fields.accessibility()
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
    Generic,
    Alert,
    Button,
    Checkbox,
    Combobox,
    Dialog,
    Group,
    Image,
    List,
    Menu,
    Navigation,
    ProgressBar,
    RadioGroup,
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
    pub fn aria_role(self) -> Option<&'static str> {
        match self {
            Self::Generic | Self::Text => None,
            Self::Alert => Some("alert"),
            Self::Button => Some("button"),
            Self::Checkbox => Some("checkbox"),
            Self::Combobox => Some("combobox"),
            Self::Dialog => Some("dialog"),
            Self::Group => Some("group"),
            Self::Image => Some("img"),
            Self::List => Some("list"),
            Self::Menu => Some("menu"),
            Self::Navigation => Some("navigation"),
            Self::ProgressBar => Some("progressbar"),
            Self::RadioGroup => Some("radiogroup"),
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
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AccessibilityState {
    pub disabled: bool,
    pub checked: Option<bool>,
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
}

impl AccessibilitySnapshot {
    pub fn new(role: AccessibilityRole) -> Self {
        Self {
            role,
            name: None,
            state: AccessibilityState::default(),
        }
    }

    pub fn named(role: AccessibilityRole, name: impl Into<String>) -> Self {
        Self {
            role,
            name: non_empty(name.into()),
            state: AccessibilityState::default(),
        }
    }

    pub fn with_state(mut self, state: AccessibilityState) -> Self {
        self.state = state;
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

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotTreeNode {
    pub title: String,
    pub key: String,
    pub icon: String,
    pub children: Vec<SnapshotTreeNode>,
    pub disabled: bool,
    pub checkable: bool,
    pub draggable: bool,
    pub is_leaf: bool,
}

impl SnapshotTreeNode {
    pub fn from_tree_node(node: &TreeNode) -> Self {
        Self {
            title: node.title.clone(),
            key: node.key.clone(),
            icon: node.icon.clone(),
            children: node.children.iter().map(Self::from_tree_node).collect(),
            disabled: node.disabled,
            checkable: node.checkable,
            draggable: node.draggable,
            is_leaf: node.is_leaf,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotCollapsePanel {
    pub header: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotTableColumn {
    pub title: String,
    pub width: f32,
    pub sortable: bool,
    pub sort_direction: SortDirection,
    pub filterable: bool,
    pub filters: Vec<String>,
    pub fixed: Option<Fixed>,
}

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
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotTransferItem {
    pub key: String,
    pub title: String,
}

impl SnapshotTransferItem {
    pub fn from_transfer_item(item: &TransferItem) -> Self {
        Self {
            key: item.key.clone(),
            title: item.title.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotValue {
    Debug(String),
}
