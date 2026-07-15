use crate::ui::widgets::{
    AnchorItem, BadgeStatus, BreadcrumbItem, Date, OptGroup, ProgressMode, SelectableItem,
};

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

pub(super) fn image_accessibility(
    alt: &str,
    fallback: &str,
    src: &str,
    preview: bool,
    preview_open: bool,
) -> AccessibilitySnapshot {
    let role = if preview {
        AccessibilityRole::Button
    } else {
        AccessibilityRole::Image
    };
    AccessibilitySnapshot::named(role, first_non_empty([alt, fallback, src])).with_state(
        AccessibilityState {
            expanded: preview.then_some(preview_open),
            ..AccessibilityState::default()
        },
    )
}

pub(super) fn badge_accessibility(
    count: i32,
    max: i32,
    dot: bool,
    status: Option<BadgeStatus>,
    show_zero: bool,
    text: &str,
) -> AccessibilitySnapshot {
    let visible =
        dot || status.is_some() || !text.is_empty() || count > 0 || (count == 0 && show_zero);
    if !visible {
        return AccessibilitySnapshot::new(AccessibilityRole::None);
    }

    let numeric = !dot && status.is_none() && text.is_empty();
    let name = if !text.is_empty() {
        text.to_owned()
    } else if numeric {
        format!("{}{}", count.min(max), if count > max { "+" } else { "" })
    } else {
        String::new()
    };
    AccessibilitySnapshot::named(AccessibilityRole::Status, name).with_state(AccessibilityState {
        value_now: numeric.then_some(count as f64),
        value_min: numeric.then_some(0.0),
        value_max: numeric.then_some(max as f64),
        ..AccessibilityState::default()
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn select_accessibility(
    plain_options: &[String],
    optgroups: &[OptGroup],
    selected: usize,
    selected_multi: &[usize],
    placeholder: &str,
    disabled: bool,
    multiple: bool,
    open: bool,
    search_query: &str,
) -> AccessibilitySnapshot {
    let options = if optgroups.is_empty() {
        plain_options.iter().map(String::as_str).collect::<Vec<_>>()
    } else {
        optgroups
            .iter()
            .flat_map(|group| group.options.iter().map(String::as_str))
            .collect()
    };
    let selected_value = if multiple {
        let selected = selected_multi
            .iter()
            .filter_map(|index| options.get(*index).copied())
            .collect::<Vec<_>>();
        (!selected.is_empty()).then(|| selected.join(", "))
    } else {
        options.get(selected).map(|option| (*option).to_owned())
    };
    let value_text = if !open || search_query.is_empty() {
        selected_value
    } else {
        Some(search_query.to_owned())
    };
    AccessibilitySnapshot::named(AccessibilityRole::Combobox, placeholder).with_state(
        AccessibilityState {
            disabled,
            value_text,
            ..AccessibilityState::default()
        },
    )
}

pub(super) fn date_range_accessibility(
    placeholder: &str,
    start: Option<&String>,
    end: Option<&String>,
) -> AccessibilitySnapshot {
    AccessibilitySnapshot::named(AccessibilityRole::Combobox, placeholder).with_state(
        AccessibilityState {
            value_text: start
                .zip(end)
                .map(|(start, end)| format!("{start} / {end}")),
            ..AccessibilityState::default()
        },
    )
}

pub(super) fn result_accessibility(
    title: &str,
    subtitle: &str,
    extra_text: &str,
) -> AccessibilitySnapshot {
    let action = !extra_text.is_empty();
    let summary = first_non_empty([title, subtitle]);
    AccessibilitySnapshot::named(
        if action {
            AccessibilityRole::Button
        } else {
            AccessibilityRole::Status
        },
        if action {
            extra_text.to_owned()
        } else {
            summary.clone()
        },
    )
    .with_state(AccessibilityState {
        value_text: (action && !summary.is_empty()).then_some(summary),
        ..AccessibilityState::default()
    })
}

pub(super) fn tag_accessibility(
    text: &str,
    closable: bool,
    checkable: bool,
    checked: bool,
) -> AccessibilitySnapshot {
    let role = if checkable {
        AccessibilityRole::Checkbox
    } else if closable {
        AccessibilityRole::Button
    } else {
        AccessibilityRole::Generic
    };
    AccessibilitySnapshot::named(role, text).with_state(AccessibilityState {
        checked: checkable.then_some(checked),
        ..AccessibilityState::default()
    })
}

pub(super) fn progress_accessibility(mode: ProgressMode) -> AccessibilitySnapshot {
    let state = match mode {
        ProgressMode::Determinate(progress) => {
            let progress = if progress.is_finite() {
                progress.clamp(0.0, 1.0)
            } else {
                0.0
            };
            AccessibilityState {
                value_now: Some(progress as f64),
                value_min: Some(0.0),
                value_max: Some(1.0),
                ..AccessibilityState::default()
            }
        }
        ProgressMode::Indeterminate => AccessibilityState::default(),
    };
    AccessibilitySnapshot::new(AccessibilityRole::ProgressBar).with_state(state)
}

pub(super) fn first_non_empty<const N: usize>(values: [&str; N]) -> String {
    values
        .into_iter()
        .find(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}
