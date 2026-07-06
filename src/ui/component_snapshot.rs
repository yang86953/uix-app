use std::any::{Any, TypeId};
use std::fmt;

use crate::core::EdgeInsets;
use crate::draw::spatial::PhysicalUnit;
use crate::draw::Color;
use crate::native::traits::input::ControlSize;
use crate::ui::layout::{AlignItems, GridTrack, JustifyContent};
use crate::ui::style::{Style, StyleSet};
use crate::ui::widget::WidgetId;
use crate::ui::widgets::{Button, Container, Grid, Input, Label};

#[derive(Debug, Clone, PartialEq)]
pub struct ComponentConfigSnapshot {
    pub id: WidgetId,
    pub widget_type: TypeId,
    pub fields: SnapshotFields,
}

impl ComponentConfigSnapshot {
    pub fn from_component(
        id: WidgetId,
        component: &dyn crate::ui::traits::WidgetComponent,
    ) -> Self {
        Self {
            id,
            widget_type: component.as_any().type_id(),
            fields: component.snapshot_fields(),
        }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotValue {
    Debug(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SnapshotFields {
    Unknown,
    Custom {
        widget: &'static str,
        fields: Vec<SnapshotField>,
    },
    Button {
        text: String,
        disabled: bool,
        block: bool,
        style_set: StyleSet,
        style: Style,
    },
    Label {
        text: String,
        font_size: f32,
        font_size_unit: Option<PhysicalUnit>,
        color: Option<Color>,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        style: Option<Style>,
    },
    Input {
        placeholder: String,
        input_size: ControlSize,
        disabled: bool,
        prefix: String,
        suffix: String,
        addon_before: String,
        addon_after: String,
        password: bool,
        password_visible: bool,
        clearable: bool,
        search: bool,
        textarea: bool,
        textarea_rows: usize,
    },
    Container {
        style: Style,
    },
    Grid {
        columns: Vec<GridTrack>,
        rows: Vec<GridTrack>,
        col_gap: f32,
        row_gap: f32,
        padding: EdgeInsets,
        bg_color: Option<Color>,
        border_color: Option<Color>,
        border_width: f32,
        border_radius: f32,
        align_items: AlignItems,
        justify_items: JustifyContent,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
    },
}

pub trait SnapshotSource {
    fn snapshot_fields(&self) -> SnapshotFields;
}

pub fn snapshot_fields_from_any(component: &dyn Any) -> SnapshotFields {
    if let Some(button) = component.downcast_ref::<Button>() {
        return button.snapshot_fields();
    }
    if let Some(label) = component.downcast_ref::<Label>() {
        return label.snapshot_fields();
    }
    if let Some(input) = component.downcast_ref::<Input>() {
        return input.snapshot_fields();
    }
    if let Some(container) = component.downcast_ref::<Container>() {
        return container.snapshot_fields();
    }
    if let Some(grid) = component.downcast_ref::<Grid>() {
        return grid.snapshot_fields();
    }
    SnapshotFields::Unknown
}

#[cfg(test)]
#[path = "../tests/ui/component_snapshot.rs"]
mod tests;
