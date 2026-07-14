//! Node-authored render and child-factory handlers kept outside widget storage.

use std::collections::HashMap;

use crate::core::ComponentId;
use crate::ui::core::widget::WidgetNode;
use crate::ui::foundation::virtual_scroll::VirtualScrollRenderer;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::display::table::{ExpandRenderer, TableRow};

pub(crate) enum RenderHandlerRegistration {
    TableExpand(ExpandRenderer),
    VirtualScrollItem(VirtualScrollRenderer),
}

#[derive(Default)]
pub(crate) struct RenderHandlerTable {
    table_expand: HashMap<ComponentId, ExpandRenderer>,
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
        Some((start..end).map(renderer).collect())
    }

    pub(crate) fn clear_component(&mut self, component: ComponentId) {
        self.table_expand.remove(&component);
        self.virtual_scroll_item.remove(&component);
    }

    pub(crate) fn clear(&mut self) {
        self.table_expand.clear();
        self.virtual_scroll_item.clear();
    }

    pub(crate) fn contains_table_expand(&self, component: ComponentId) -> bool {
        self.table_expand.contains_key(&component)
    }

    pub(crate) fn contains_virtual_scroll_item(&self, component: ComponentId) -> bool {
        self.virtual_scroll_item.contains_key(&component)
    }
}
