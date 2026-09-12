//! `ui/coordination/render_handler.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

#[test]
fn empty_render_handler_sidecars_report_every_renderer_absent() {
    let table = RenderHandlerTable::default();
    let widget = WidgetId::new(7);

    assert!(!table.contains_virtual_scroll_item(widget));
    assert!(!table.contains_select_options(widget));
    #[cfg(feature = "table")]
    {
        assert!(!table.contains_table_expand(widget));
        assert!(!table.contains_table_cells(widget));
    }
}
