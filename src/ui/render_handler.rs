//! Node-authored render and child-factory handlers kept outside widget storage.

use std::collections::HashMap;

use crate::core::ComponentId;
use crate::ui::core::widget::WidgetNode;
use crate::ui::foundation::virtual_scroll::VirtualScrollRenderer;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::display::table::{ExpandRenderer, TableCellRenderer, TableRow};
use crate::ui::widgets::input::select::SelectOptionRenderer;

pub(crate) enum RenderHandlerRegistration {
    TableExpand(ExpandRenderer),
    TableCells(TableCellRenderer),
    SelectOptions(SelectOptionRenderer),
    VirtualScrollItem(VirtualScrollRenderer),
}

#[derive(Default)]
pub(crate) struct RenderHandlerTable {
    table_expand: HashMap<ComponentId, ExpandRenderer>,
    table_cells: HashMap<ComponentId, TableCellRenderer>,
    select_options: HashMap<ComponentId, SelectOptionRenderer>,
    virtual_scroll_item: HashMap<ComponentId, VirtualScrollRenderer>,
}

impl RenderHandlerTable {
    pub(crate) fn replace_component(
        &mut self,
        component: ComponentId,
        handlers: Vec<RenderHandlerRegistration>,
    ) {
        self.clear_component(component);
        for handler in handlers {
            match handler {
                RenderHandlerRegistration::TableExpand(renderer) => {
                    self.table_expand.insert(component, renderer);
                }
                RenderHandlerRegistration::TableCells(renderer) => {
                    self.table_cells.insert(component, renderer);
                }
                RenderHandlerRegistration::SelectOptions(renderer) => {
                    self.select_options.insert(component, renderer);
                }
                RenderHandlerRegistration::VirtualScrollItem(renderer) => {
                    self.virtual_scroll_item.insert(component, renderer);
                }
            }
        }
    }

    pub(crate) fn render_table_expand_view(
        &self,
        component: ComponentId,
        row: &TableRow,
    ) -> Option<ViewNode> {
        let renderer = self.table_expand.get(&component)?;
        Some(ViewAdapter::capture_root(|| renderer(row)))
    }

    pub(crate) fn render_table_expand_widget(
        &self,
        component: ComponentId,
        row: &TableRow,
    ) -> Option<WidgetNode> {
        self.render_table_expand_view(component, row)
            .map(ViewAdapter::expand)
    }

    pub(crate) fn render_virtual_scroll_items(
        &mut self,
        component: ComponentId,
        start: usize,
        end: usize,
    ) -> Option<Vec<WidgetNode>> {
        let renderer = self.virtual_scroll_item.get_mut(&component)?;
        Some(
            (start..end)
                .map(|index| ViewAdapter::capture_root(|| renderer(index)))
                .map(ViewAdapter::expand)
                .collect(),
        )
    }

    pub(crate) fn render_table_cells(
        &self,
        component: ComponentId,
        range: (usize, usize),
        row_keys: &[String],
        view_columns: &[usize],
    ) -> Option<Vec<ViewNode>> {
        let renderer = self.table_cells.get(&component)?;
        let mut cells = Vec::with_capacity(
            range
                .1
                .saturating_sub(range.0)
                .saturating_mul(view_columns.len()),
        );
        for row in range.0..range.1 {
            let Some(row_key) = row_keys.get(row) else {
                continue;
            };
            for (renderer_index, &column) in view_columns.iter().enumerate() {
                let cell = ViewAdapter::capture_root(|| renderer(row, renderer_index));
                cells.push(cell.key(format!("table-cell:{row_key}:{column}")));
            }
        }
        Some(cells)
    }

    pub(crate) fn render_select_options(
        &self,
        component: ComponentId,
        indices: &[usize],
        labels: &[String],
    ) -> Option<Vec<ViewNode>> {
        let renderer = self.select_options.get(&component)?;
        Some(
            labels
                .iter()
                .map(|label| ViewAdapter::capture_root(|| renderer(label)))
                .zip(indices)
                .map(|(view, index)| view.key(format!("select-option:{index}")))
                .collect(),
        )
    }

    pub(crate) fn clear_component(&mut self, component: ComponentId) {
        self.table_expand.remove(&component);
        self.table_cells.remove(&component);
        self.select_options.remove(&component);
        self.virtual_scroll_item.remove(&component);
    }

    pub(crate) fn clear(&mut self) {
        self.table_expand.clear();
        self.table_cells.clear();
        self.select_options.clear();
        self.virtual_scroll_item.clear();
    }

    pub(crate) fn contains_table_expand(&self, component: ComponentId) -> bool {
        self.table_expand.contains_key(&component)
    }

    pub(crate) fn contains_virtual_scroll_item(&self, component: ComponentId) -> bool {
        self.virtual_scroll_item.contains_key(&component)
    }

    pub(crate) fn contains_table_cells(&self, component: ComponentId) -> bool {
        self.table_cells.contains_key(&component)
    }

    pub(crate) fn contains_select_options(&self, component: ComponentId) -> bool {
        self.select_options.contains_key(&component)
    }
}
