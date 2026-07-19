use super::{SelectionSnapshot, SnapshotFields};

impl SnapshotFields {
    pub(super) fn selection(&self) -> Option<SelectionSnapshot> {
        match self {
            Self::Radio {
                options, selected, ..
            } => Some(SelectionSnapshot {
                options: options.clone(),
                selected_indices: (*selected < options.len())
                    .then_some(*selected)
                    .into_iter()
                    .collect(),
                disabled_indices: Vec::new(),
                multiple: false,
                expanded: false,
            }),
            Self::Select {
                options,
                optgroups,
                selected,
                selected_multi,
                multiple,
                open,
                loading,
                ..
            } => {
                let options = if optgroups.is_empty() {
                    options.clone()
                } else {
                    optgroups
                        .iter()
                        .flat_map(|group| group.options.iter().cloned())
                        .collect()
                };
                let selected_indices = if *multiple {
                    selected_multi
                        .iter()
                        .copied()
                        .filter(|index| *index < options.len())
                        .collect()
                } else {
                    (*selected < options.len())
                        .then_some(*selected)
                        .into_iter()
                        .collect()
                };
                let disabled_indices = if *loading {
                    (0..options.len()).collect()
                } else {
                    Vec::new()
                };
                Some(SelectionSnapshot {
                    options,
                    selected_indices,
                    disabled_indices,
                    multiple: *multiple,
                    expanded: *open,
                })
            }
            Self::Segmented {
                options,
                selected,
                disabled_options,
                ..
            } => Some(SelectionSnapshot {
                options: options.clone(),
                selected_indices: (*selected < options.len())
                    .then_some(*selected)
                    .into_iter()
                    .collect(),
                disabled_indices: disabled_options
                    .iter()
                    .enumerate()
                    .filter_map(|(index, disabled)| disabled.then_some(index))
                    .filter(|index| *index < options.len())
                    .collect(),
                multiple: false,
                expanded: false,
            }),
            _ => None,
        }
    }
}
