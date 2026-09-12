//! `src/ui/widgets/display/table/geometry.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl TableColumnGeometry） ——

impl TableColumnGeometry {
    #[cfg(test)]
    pub(crate) fn new(
        columns: &[TableColumn],
        origin_x: f32,
        viewport_width: f32,
        selection_width: f32,
        scroll_x: f32,
    ) -> Self {
        let mut geometry = Self::default();
        geometry.resolve(columns, origin_x, viewport_width, selection_width, scroll_x);
        geometry
    }
}
