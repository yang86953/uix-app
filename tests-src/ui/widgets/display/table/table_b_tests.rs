//! `src/ui/widgets/display/table/table_b.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl Table） ——

impl Table {
    // 单元测试保留可跨表格原位修改持有的独立几何快照。
    #[cfg(test)]
    pub(crate) fn column_geometry(
        &self,
        origin_x: f32,
        viewport_width: f32,
    ) -> TableColumnGeometry {
        TableColumnGeometry::new(
            &self.columns,
            origin_x,
            viewport_width,
            self.selection_width(),
            self.horizontal_scroll.get(),
        )
    }

    // 测试目标保留表格横向滚动偏移观测入口，供表格交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn horizontal_scroll_offset(&self) -> f32 {
        self.horizontal_scroll.get()
    }

    // 测试目标保留分页控制区域观测入口，供表格交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn pagination_controls_for_test(&self, frame: Rect) -> Option<(Rect, Rect, Rect)> {
        self.pagination_controls(frame)
    }
}
