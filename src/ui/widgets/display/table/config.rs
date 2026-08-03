use super::types::{ColumnGroupRange, TableColumn, TableColumnGroup};

pub(super) fn merge_table_columns(
    current: &[TableColumn],
    next: Vec<TableColumn>,
    table_sortable: bool,
) -> Vec<TableColumn> {
    next.into_iter()
        .enumerate()
        .map(|(idx, mut next_col)| {
            if let Some(current_col) = current.get(idx) {
                if current_col.resizable && next_col.resizable {
                    next_col.width = current_col.width;
                }
                if table_sortable || next_col.sortable {
                    next_col.sort_direction = current_col.sort_direction;
                }
                if next_col.filterable {
                    for (label, active) in &mut next_col.filters {
                        if let Some((_, current_active)) = current_col
                            .filters
                            .iter()
                            .find(|(current_label, _)| current_label == label)
                        {
                            *active = *current_active;
                        }
                    }
                }
            }
            next_col
        })
        .collect()
}

pub(super) fn flatten_column_groups(
    groups: Vec<TableColumnGroup>,
) -> (Vec<TableColumn>, Vec<ColumnGroupRange>) {
    let mut columns = Vec::new();
    let mut ranges = Vec::new();
    for group in groups {
        let start = columns.len();
        let len = group.columns.len();
        columns.extend(group.columns);
        if len > 0 {
            ranges.push(ColumnGroupRange {
                title: group.title,
                start,
                len,
            });
        }
    }
    (columns, ranges)
}
