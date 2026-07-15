use crate::ui::widgets::Date;

use super::{AccessibilityRole, AccessibilitySnapshot, AccessibilityState};

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

pub(super) fn first_non_empty<const N: usize>(values: [&str; N]) -> String {
    values
        .into_iter()
        .find(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}
