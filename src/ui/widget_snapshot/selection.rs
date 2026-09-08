use super::{SelectionSnapshot, SnapshotFields};

impl SnapshotFields {
    /// 从快照字段中提取选择类控件（单选框/下拉/分段控件/页签）的统一选择状态。
    pub(super) fn selection(&self) -> Option<SelectionSnapshot> {
        match self {
            // 页签也是单选目标；索引来自当前稳定 key 顺序，不暴露组件私有状态。
            #[cfg(feature = "navigation")]
            Self::Tabs {
                tabs, active_index, ..
            } => Some(SelectionSnapshot {
                options: tabs.iter().map(|tab| tab.label.clone()).collect(),
                selected_indices: (*active_index < tabs.len())
                    .then_some(*active_index)
                    .into_iter()
                    .collect(),
                disabled_indices: Vec::new(),
                multiple: false,
                expanded: false,
            }),
            // 单选框：单选，选中索引超出选项数则视为未选中。
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
            // 下拉：支持分组选项展平、多选与加载态全禁用。
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
                // 存在分组时把各组选项展平为单一列表。
                let options = if optgroups.is_empty() {
                    options.clone()
                } else {
                    optgroups
                        .iter()
                        .flat_map(|group| group.options.iter().cloned())
                        .collect()
                };
                // 多选取选中集合，单选取单个索引；均过滤越界索引。
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
                // 加载中禁用全部选项。
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
            // 分段控件：单选，按禁用标记收集禁用索引并过滤越界。
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
            // 其余快照类型不具备选择语义，返回 None。
            _ => None,
        }
    }
}
