use std::any::{Any, TypeId};
use std::fmt;

use crate::core::EdgeInsets;
use crate::draw::spatial::PhysicalUnit;
use crate::draw::Color;
use crate::native::traits::input::ControlSize;
use crate::ui::layout::{AlignItems, FlexDirection, GridTrack, JustifyContent};
use crate::ui::style::{Style, StyleSet};
use crate::ui::widget::WidgetId;
use crate::ui::widgets::{
    Avatar, Badge, BadgeStatus, Button, Calendar, Card, Checkbox, Container, Divider,
    DividerDirection, DividerOrientation, Empty, FloatButton, Grid, Icon, Image, Input,
    InputNumber, Label, Radio, RadioDirection, Rate, Skeleton, SkeletonShape, Slider, Space,
    SpaceSize, Switch, Tag, TagColor, Timeline, TimelineItem, Typography, TypographyType,
};

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
    Space {
        direction: FlexDirection,
        space_size: SpaceSize,
        wrap: bool,
        justify: JustifyContent,
        align: AlignItems,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        flex_grow: f32,
    },
    Divider {
        text: Option<String>,
        orientation: DividerOrientation,
        direction: DividerDirection,
        color: Option<Color>,
        text_size: f32,
        dashed: bool,
    },
    Icon {
        name: String,
        size: f32,
    },
    Typography {
        content: String,
        type_: TypographyType,
        disabled: bool,
        mark: bool,
        code: bool,
        underline: bool,
        delete: bool,
        strong: bool,
        italic: bool,
        copyable: bool,
        color_override: Option<Color>,
    },
    Checkbox {
        checked: bool,
        disabled: bool,
        label: String,
    },
    Radio {
        options: Vec<String>,
        selected: usize,
        disabled: bool,
        direction: RadioDirection,
        item_h: f32,
    },
    Switch {
        checked: bool,
        disabled: bool,
        size: f32,
    },
    Slider {
        min: f32,
        max: f32,
        step: f32,
        value: f32,
    },
    Rate {
        count: usize,
        value: usize,
        half: bool,
        disabled: bool,
        clearable: bool,
        character: String,
    },
    InputNumber {
        value: f64,
        min: f64,
        max: f64,
        step: f64,
        placeholder: String,
        disabled: bool,
    },
    Avatar {
        text: String,
        size: f32,
        bg_color: Option<Color>,
        text_color: Option<Color>,
        square: bool,
        src: String,
    },
    Badge {
        count: i32,
        max: i32,
        dot: bool,
        color: Option<Color>,
        size: f32,
        status: Option<BadgeStatus>,
        show_zero: bool,
        text: String,
        offset_x: f32,
        offset_y: f32,
        offset_unit: Option<(PhysicalUnit, PhysicalUnit)>,
    },
    Card {
        title: Option<String>,
        bordered: bool,
        hoverable: bool,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        padding: f32,
        elevation: u8,
        flex_grow: f32,
        actions: Vec<String>,
    },
    Empty {
        description: String,
        icon_name: String,
        image: String,
    },
    Image {
        src: String,
        alt: String,
        fallback: String,
        width: f32,
        height: f32,
        radius: f32,
        preview: bool,
        fit: bool,
    },
    Tag {
        text: String,
        color: TagColor,
        closable: bool,
        font_size: f32,
        custom_color: Option<Color>,
        checkable: bool,
    },
    Timeline {
        items: Vec<TimelineItem>,
        pending: bool,
        reverse: bool,
    },
    Calendar {
        cell_size: f32,
        year_jump: bool,
    },
    Skeleton {
        shape: SkeletonShape,
        width: f32,
        height: f32,
    },
    FloatButton {
        icon: String,
        tooltip: String,
        badge_count: i32,
        size: f32,
        x: f32,
        y: f32,
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
    if let Some(space) = component.downcast_ref::<Space>() {
        return space.snapshot_fields();
    }
    if let Some(divider) = component.downcast_ref::<Divider>() {
        return divider.snapshot_fields();
    }
    if let Some(icon) = component.downcast_ref::<Icon>() {
        return icon.snapshot_fields();
    }
    if let Some(typography) = component.downcast_ref::<Typography>() {
        return typography.snapshot_fields();
    }
    if let Some(checkbox) = component.downcast_ref::<Checkbox>() {
        return checkbox.snapshot_fields();
    }
    if let Some(radio) = component.downcast_ref::<Radio>() {
        return radio.snapshot_fields();
    }
    if let Some(switch) = component.downcast_ref::<Switch>() {
        return switch.snapshot_fields();
    }
    if let Some(slider) = component.downcast_ref::<Slider>() {
        return slider.snapshot_fields();
    }
    if let Some(rate) = component.downcast_ref::<Rate>() {
        return rate.snapshot_fields();
    }
    if let Some(input_number) = component.downcast_ref::<InputNumber>() {
        return input_number.snapshot_fields();
    }
    if let Some(avatar) = component.downcast_ref::<Avatar>() {
        return avatar.snapshot_fields();
    }
    if let Some(badge) = component.downcast_ref::<Badge>() {
        return badge.snapshot_fields();
    }
    if let Some(card) = component.downcast_ref::<Card>() {
        return card.snapshot_fields();
    }
    if let Some(empty) = component.downcast_ref::<Empty>() {
        return empty.snapshot_fields();
    }
    if let Some(image) = component.downcast_ref::<Image>() {
        return image.snapshot_fields();
    }
    if let Some(tag) = component.downcast_ref::<Tag>() {
        return tag.snapshot_fields();
    }
    if let Some(timeline) = component.downcast_ref::<Timeline>() {
        return timeline.snapshot_fields();
    }
    if let Some(calendar) = component.downcast_ref::<Calendar>() {
        return calendar.snapshot_fields();
    }
    if let Some(skeleton) = component.downcast_ref::<Skeleton>() {
        return skeleton.snapshot_fields();
    }
    if let Some(float_button) = component.downcast_ref::<FloatButton>() {
        return float_button.snapshot_fields();
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
