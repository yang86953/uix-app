use super::tree_core::WidgetTree;
use super::WidgetCore;
use crate::core::ComponentId;
use crate::ui::foundation::virtual_scroll::VirtualScroll;
use crate::ui::view::ViewAdapter;
use crate::ui::widgets::display::table::Table;
use crate::ui::widgets::input::Select;

impl WidgetTree {
    pub(crate) fn table_expand_view(&self, id: ComponentId) -> Option<crate::ui::view::ViewNode> {
        let table = self.get(id)?.component().as_any().downcast_ref::<Table>()?;
        let row = table.rows.get(table.expanded_row()?)?;
        self.render_handler_table.render_table_expand_view(id, row)
    }

    pub(crate) fn mark_table_expand_materialized(&self, id: ComponentId) {
        if let Some(table) = self
            .get(id)
            .and_then(|node| node.component().as_any().downcast_ref::<Table>())
        {
            table.mark_expanded_child_materialized();
        }
    }

    pub(crate) fn refresh_table_expand_component(&mut self, id: ComponentId) -> bool {
        if !self.render_handler_table.contains_table_expand(id) {
            return false;
        }

        let Some((expanded_row, materialized_row, child_count, row)) =
            self.get(id).and_then(|node| {
                let table = node.component().as_any().downcast_ref::<Table>()?;
                let expanded_row = table.expanded_row();
                let row = expanded_row.and_then(|index| table.rows.get(index).cloned());
                Some((
                    expanded_row,
                    table.expanded_child_row(),
                    node.children().len(),
                    row,
                ))
            })
        else {
            return false;
        };
        let expected_children = usize::from(row.is_some());
        if expanded_row == materialized_row && child_count == expected_children {
            return false;
        }

        let children = row
            .as_ref()
            .and_then(|row| {
                self.render_handler_table
                    .render_table_expand_widget(id, row)
            })
            .into_iter()
            .collect();
        self.set_children(id, children);
        self.mark_table_expand_materialized(id);
        self.bind_orphan_pending_states();
        self.bind_pending_effects();
        true
    }

    pub(crate) fn refresh_virtual_scroll_component(
        &mut self,
        id: ComponentId,
        viewport_height: Option<f32>,
    ) -> bool {
        if !self.render_handler_table.contains_virtual_scroll_item(id) {
            return false;
        }

        let Some((range, needs_refresh)) = self.get(id).and_then(|node| {
            let scroll = node.component().as_any().downcast_ref::<VirtualScroll>()?;
            let height = viewport_height
                .filter(|height| *height > 0.0)
                .unwrap_or_else(|| scroll.configured_viewport_height());
            let range = scroll.scroll_range(height);
            Some((
                range,
                scroll.needs_child_refresh(height, node.children().len()),
            ))
        }) else {
            return false;
        };
        if !needs_refresh {
            return false;
        }

        let children = self
            .render_handler_table
            .render_virtual_scroll_items(id, range.0, range.1)
            .unwrap_or_default();
        self.set_children(id, children);
        if let Some(scroll) = self
            .get(id)
            .and_then(|node| node.component().as_any().downcast_ref::<VirtualScroll>())
        {
            scroll.mark_children_materialized(range);
        }
        self.bind_orphan_pending_states();
        self.bind_pending_effects();
        true
    }

    pub(crate) fn refresh_table_cell_component(&mut self, id: ComponentId) -> bool {
        if !self.render_handler_table.contains_table_cells(id) {
            return false;
        }

        let Some((range, needs_refresh)) = self.get(id).and_then(|node| {
            let table = node.component().as_any().downcast_ref::<Table>()?;
            let range = table.cell_view_range_for_frame(node.frame());
            Some((
                range,
                table.needs_cell_refresh(range, node.children().len()),
            ))
        }) else {
            return false;
        };
        if !needs_refresh {
            return false;
        }

        let Some((row_keys, view_columns)) = self.get(id).and_then(|node| {
            let table = node.component().as_any().downcast_ref::<Table>()?;
            Some((table.row_keys().to_vec(), table.view_columns().to_vec()))
        }) else {
            return false;
        };

        let children = self
            .render_handler_table
            .render_table_cells(id, range, &row_keys, &view_columns)
            .unwrap_or_default();
        let changed = ViewAdapter::reconcile_dynamic_children(self, id, children);
        if let Some(table) = self
            .get(id)
            .and_then(|node| node.component().as_any().downcast_ref::<Table>())
        {
            table.mark_cells_materialized(range);
        }
        self.bind_orphan_pending_states();
        self.bind_pending_effects();
        changed
    }

    pub(crate) fn refresh_select_option_component(&mut self, id: ComponentId) -> bool {
        if !self.render_handler_table.contains_select_options(id) {
            return false;
        }

        let Some((indices, labels, needs_refresh)) = self.get(id).and_then(|node| {
            let select = node.component().as_any().downcast_ref::<Select>()?;
            let indices = select.custom_option_indices();
            let labels = select.custom_option_labels(&indices);
            let needs_refresh = select.needs_custom_option_refresh(&indices, node.children().len());
            Some((indices, labels, needs_refresh))
        }) else {
            return false;
        };
        if !needs_refresh {
            return false;
        }

        let children = self
            .render_handler_table
            .render_select_options(id, &indices, &labels)
            .unwrap_or_default();
        let changed = ViewAdapter::reconcile_dynamic_children(self, id, children);
        if let Some(select) = self
            .get(id)
            .and_then(|node| node.component().as_any().downcast_ref::<Select>())
        {
            select.mark_custom_options_materialized(indices);
        }
        self.bind_orphan_pending_states();
        self.bind_pending_effects();
        changed
    }

    pub(crate) fn invalidate_select_option_component(&self, id: ComponentId) {
        if let Some(select) = self
            .get(id)
            .and_then(|node| node.component().as_any().downcast_ref::<Select>())
        {
            select.invalidate_custom_option_materialization();
        }
    }

    pub(crate) fn has_table_expand_renderer(&self, id: ComponentId) -> bool {
        self.render_handler_table.contains_table_expand(id)
    }

    pub(crate) fn has_table_cell_renderer(&self, id: ComponentId) -> bool {
        self.render_handler_table.contains_table_cells(id)
    }

    pub(crate) fn has_select_option_renderer(&self, id: ComponentId) -> bool {
        self.render_handler_table.contains_select_options(id)
    }

    #[cfg(test)]
    pub(crate) fn has_virtual_scroll_renderer(&self, id: ComponentId) -> bool {
        self.render_handler_table.contains_virtual_scroll_item(id)
    }
}
