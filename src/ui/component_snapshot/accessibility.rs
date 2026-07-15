use crate::ui::widgets::{AnchorItem, BreadcrumbItem, Date, SelectableItem};

use super::{AccessibilityRole, AccessibilitySnapshot, AccessibilityState, SnapshotTransferItem};

pub(super) fn transfer_accessibility(
    source: &[SnapshotTransferItem],
    target: &[SnapshotTransferItem],
) -> AccessibilitySnapshot {
    let selected = source
        .iter()
        .chain(target)
        .filter(|item| item.selected)
        .count();
    AccessibilitySnapshot::new(AccessibilityRole::List).with_state(AccessibilityState {
        value_text: Some(format!(
            "{} source; {} target; {selected} selected",
            source.len(),
            target.len()
        )),
        ..AccessibilityState::default()
    })
}

pub(super) fn splitter_accessibility(
    ratios: &[f32],
    active_handle: usize,
) -> AccessibilitySnapshot {
    let boundary = ratios
        .iter()
        .take(active_handle.saturating_add(1))
        .copied()
        .sum::<f32>()
        .clamp(0.0, 1.0);
    AccessibilitySnapshot::new(AccessibilityRole::Slider).with_state(AccessibilityState {
        value_text: Some(
            ratios
                .iter()
                .map(|ratio| format!("{ratio:.6}"))
                .collect::<Vec<_>>()
                .join(","),
        ),
        value_now: Some(f64::from(boundary)),
        value_min: Some(0.0),
        value_max: Some(1.0),
        ..AccessibilityState::default()
    })
}

pub(super) fn carousel_accessibility(current: usize, slide_count: usize) -> AccessibilitySnapshot {
    AccessibilitySnapshot::new(AccessibilityRole::Group).with_state(AccessibilityState {
        value_text: (slide_count > 0).then(|| format!("Slide {} of {slide_count}", current + 1)),
        value_now: (slide_count > 0).then_some((current + 1) as f64),
        value_min: (slide_count > 0).then_some(1.0),
        value_max: (slide_count > 0).then_some(slide_count as f64),
        ..AccessibilityState::default()
    })
}

pub(super) fn calendar_accessibility(
    year: i32,
    month: usize,
    selected: Option<Date>,
    focused_day: usize,
) -> AccessibilitySnapshot {
    let focused = Date::new(year, month, focused_day);
    let value_text = selected.map_or_else(
        || focused.format(),
        |selected| format!("{}; focused {}", selected.format(), focused.format()),
    );
    AccessibilitySnapshot::new(AccessibilityRole::Group).with_state(AccessibilityState {
        value_text: Some(value_text),
        value_now: Some(focused.day as f64),
        value_min: Some(1.0),
        value_max: Some(crate::ui::widgets::input::date_picker::days_in_month(year, month) as f64),
        ..AccessibilityState::default()
    })
}

pub(super) fn theme_toggle_accessibility(dark: bool) -> AccessibilitySnapshot {
    AccessibilitySnapshot::named(AccessibilityRole::Button, "Theme").with_state(
        AccessibilityState {
            checked: Some(dark),
            value_text: Some(if dark { "dark" } else { "light" }.to_string()),
            ..AccessibilityState::default()
        },
    )
}

pub(super) fn breadcrumb_accessibility(items: &[BreadcrumbItem]) -> AccessibilitySnapshot {
    let active = items
        .iter()
        .find(|item| item.active)
        .or_else(|| items.first());
    AccessibilitySnapshot::new(AccessibilityRole::Navigation).with_state(AccessibilityState {
        value_text: active.map(|item| item.title.clone()),
        value_now: active
            .and_then(|active| items.iter().position(|item| item == active))
            .map(|index| (index + 1) as f64),
        value_min: (!items.is_empty()).then_some(1.0),
        value_max: (!items.is_empty()).then_some(items.len() as f64),
        ..AccessibilityState::default()
    })
}

pub(super) fn pagination_accessibility(
    current: usize,
    total: usize,
    page_size: usize,
) -> AccessibilitySnapshot {
    let page_count = total.div_ceil(page_size);
    AccessibilitySnapshot::new(AccessibilityRole::Navigation).with_state(AccessibilityState {
        value_text: Some(format!(
            "Page {current} of {page_count}; {page_size} per page"
        )),
        value_now: Some(current as f64),
        value_min: Some(1.0),
        value_max: Some(page_count.max(1) as f64),
        ..AccessibilityState::default()
    })
}

pub(super) fn anchor_accessibility(
    items: &[AnchorItem],
    active_index: usize,
) -> AccessibilitySnapshot {
    AccessibilitySnapshot::new(AccessibilityRole::Navigation).with_state(AccessibilityState {
        value_text: items.get(active_index).map(|item| item.label.clone()),
        value_now: (!items.is_empty()).then_some((active_index + 1) as f64),
        value_min: (!items.is_empty()).then_some(1.0),
        value_max: (!items.is_empty()).then_some(items.len() as f64),
        ..AccessibilityState::default()
    })
}

pub(super) fn selectable_list_accessibility(
    items: &[SelectableItem],
    active_index: usize,
) -> AccessibilitySnapshot {
    AccessibilitySnapshot::new(AccessibilityRole::List).with_state(AccessibilityState {
        value_text: items.get(active_index).map(|item| item.text.clone()),
        value_now: (!items.is_empty()).then_some((active_index + 1) as f64),
        value_min: (!items.is_empty()).then_some(1.0),
        value_max: (!items.is_empty()).then_some(items.len() as f64),
        ..AccessibilityState::default()
    })
}

pub(super) fn back_top_accessibility(visible: bool) -> AccessibilitySnapshot {
    AccessibilitySnapshot::named(AccessibilityRole::Button, "Back to top").with_state(
        AccessibilityState {
            disabled: !visible,
            ..AccessibilityState::default()
        },
    )
}

pub(super) fn first_non_empty<const N: usize>(values: [&str; N]) -> String {
    values
        .into_iter()
        .find(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}
