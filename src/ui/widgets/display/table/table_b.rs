//! [`Table`] 组件实现（二）：布局、绘制与交互 — table 子模块。

use crate::core::Rect;
use crate::draw::Radius;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::{SnapshotFields, SnapshotTableColumn, SnapshotTableColumnGroup};
use std::collections::HashSet;

use super::config::merge_table_columns;
// 引入共享列区绘制层级与列几何快照。
use super::Table;
use super::geometry::{COLUMN_PAINT_ORDER, TableColumnGeometry};
use super::types::{
    COLUMN_RESIZE_HANDLE_HALF_WIDTH, MIN_RESIZABLE_COLUMN_WIDTH, SortDirection,
    TABLE_PAGINATION_GAP, TABLE_PAGINATION_HEIGHT, TABLE_PAGINATION_INSET,
    TABLE_PAGINATION_ITEM_SIZE, TABLE_PAGINATION_LABEL_WIDTH, TableChange, TableResizeDrag,
    finite_nonnegative,
};

impl Table {
    pub(crate) fn selection_width(&self) -> f32 {
        if self.selection { 32.0 } else { 0.0 }
    }

    pub(crate) fn loading_body_rect(&self, frame: Rect) -> Rect {
        let header_height = (self.total_header_height() + 1.0).min(frame.h);
        Rect::new(
            frame.x,
            frame.y + header_height,
            frame.w,
            (frame.h - header_height).max(0.0),
        )
    }

    pub(crate) fn loading_spinner_bounds(&self, frame: Rect) -> Rect {
        let body = self.loading_body_rect(frame);
        let radius = 10.0_f32.min(body.w.min(body.h) * 0.3);
        let dot_radius = radius * 0.18;
        let extent = radius + dot_radius + 1.0;
        Rect::new(
            body.x + body.w * 0.5 - extent,
            body.y + body.h * 0.5 - extent,
            extent * 2.0,
            extent * 2.0,
        )
    }

    pub(crate) fn paint_loading_overlay(&self, frame: Rect, ctx: &mut PaintContext) {
        let body = self.loading_body_rect(frame);
        if body.w <= 0.0 || body.h <= 0.0 {
            return;
        }
        ctx.fill_rect(body, ctx.tokens().color_text().with_alpha(30), None);
        let radius = 10.0_f32.min(body.w.min(body.h) * 0.3);
        if radius <= 0.0 {
            return;
        }
        let cx = body.x + body.w * 0.5;
        let cy = body.y + body.h * 0.5;
        let dot_radius = radius * 0.18;
        let primary = ctx.tokens().color_primary();
        for index in 0..8 {
            let angle = self.loading_phase + index as f32 * std::f32::consts::TAU / 8.0;
            let opacity = 0.25 + index as f32 / 8.0 * 0.75;
            ctx.fill_circle(
                cx + angle.cos() * radius,
                cy + angle.sin() * radius,
                dot_radius,
                primary.with_alpha((primary.a as f32 * opacity) as u8),
            );
        }
    }

    pub(crate) fn total_header_height(&self) -> f32 {
        if self.column_groups.is_empty() {
            self.header_h
        } else {
            self.header_h * 2.0
        }
    }

    pub(crate) fn pagination_height(&self) -> f32 {
        if self.pagination.is_some() {
            TABLE_PAGINATION_HEIGHT
        } else {
            0.0
        }
    }

    pub(crate) fn pagination_total_pages(&self) -> usize {
        self.pagination
            .as_ref()
            .map_or(1, |(_, total, page_size, _)| {
                total.div_ceil((*page_size).max(1)).max(1)
            })
    }

    pub(crate) fn pagination_current(&self) -> usize {
        self.pagination.as_ref().map_or(1, |(current, _, _, _)| {
            (*current).clamp(1, self.pagination_total_pages())
        })
    }

    pub(crate) fn pagination_controls(&self, frame: Rect) -> Option<(Rect, Rect, Rect)> {
        self.pagination.as_ref()?;
        let footer_height = TABLE_PAGINATION_HEIGHT.min(frame.h.max(0.0));
        let inset = TABLE_PAGINATION_INSET.min(frame.w.max(0.0) * 0.1);
        let available = (frame.w - inset * 2.0).max(0.0);
        let gap = TABLE_PAGINATION_GAP.min(available * 0.05);
        let label_width = TABLE_PAGINATION_LABEL_WIDTH.min(available * 0.5);
        let item_size = TABLE_PAGINATION_ITEM_SIZE
            .min(((available - label_width - gap * 2.0) * 0.5).max(0.0))
            .min(footer_height);
        let controls_width = item_size * 2.0 + label_width + gap * 2.0;
        let x = frame.x + (frame.w - inset - controls_width).max(0.0);
        let footer_y = frame.y + frame.h - footer_height;
        let y = footer_y + (footer_height - item_size) * 0.5;
        let previous = Rect::new(x, y, item_size, item_size);
        let label = Rect::new(previous.x + previous.w + gap, y, label_width, item_size);
        let next = Rect::new(label.x + label.w + gap, y, item_size, item_size);
        Some((previous, label, next))
    }

    pub(crate) fn pagination_page_at_point(&self, point: crate::core::Point) -> Option<usize> {
        let (previous, _, next) = self.pagination_controls(self.local_frame())?;
        let current = self.pagination_current();
        if previous.contains(point) {
            Some(current.saturating_sub(1).max(1))
        } else if next.contains(point) {
            Some(current.saturating_add(1).min(self.pagination_total_pages()))
        } else {
            None
        }
    }

    pub(crate) fn commit_page_change(&mut self, page: usize) {
        let Some((current, _, page_size, callback)) = self.pagination.as_ref() else {
            return;
        };
        let page = page.clamp(1, self.pagination_total_pages());
        if page == *current {
            return;
        }
        self.current_page.set(page - 1);
        callback(page);
        self.pending_change.replace(Some(
            TableChange {
                sort_column: None,
                sort_direction: SortDirection::None,
                page: page - 1,
                page_size: *page_size,
            }
            .payload(),
        ));
    }

    pub(crate) fn paint_pagination(&self, frame: Rect, ctx: &mut PaintContext) {
        let Some((previous, label, next)) = self.pagination_controls(frame) else {
            return;
        };
        let current = self.pagination_current();
        let total_pages = self.pagination_total_pages();
        let footer = Rect::new(
            frame.x,
            frame.y + (frame.h - TABLE_PAGINATION_HEIGHT).max(0.0),
            frame.w,
            TABLE_PAGINATION_HEIGHT.min(frame.h),
        );
        let border = ctx.tokens().color_border();
        let text = ctx.tokens().color_text();
        let secondary = ctx.tokens().color_text_secondary();
        let bg = ctx.tokens().color_bg_container();
        let radius = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        ctx.fill_rect(footer, bg, None);
        ctx.fill_rect(Rect::new(footer.x, footer.y, footer.w, 1.0), border, None);
        for button in [previous, next] {
            ctx.fill_rect(button, bg, radius);
            ctx.stroke_rect(button, border, 1.0, radius);
        }
        crate::ui::widgets::Icon::paint_in_frame(
            ctx,
            "chevron-left",
            previous,
            if current <= 1 { secondary } else { text },
            14.0,
        );
        ctx.text_center(&format!("{current} / {total_pages}"), label, text, 13.0);
        crate::ui::widgets::Icon::paint_in_frame(
            ctx,
            "chevron-right",
            next,
            if current >= total_pages {
                secondary
            } else {
                text
            },
            14.0,
        );
    }

    pub(crate) fn leaf_header_contains(&self, y: f32, column: Option<usize>) -> bool {
        if y < 0.0 || y >= self.total_header_height() {
            return false;
        }
        if self.column_groups.is_empty() {
            return true;
        }
        let Some(column) = column else {
            return false;
        };
        let grouped = self
            .column_groups
            .iter()
            .find(|group| column >= group.start && column < group.start + group.len)
            .is_some_and(|group| group.title.is_some());
        !grouped || y >= self.header_h
    }

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

    pub(crate) fn column_at_x(&self, x: f32) -> Option<usize> {
        let width = self
            .last_frame
            .get()
            .map(|frame| frame.w)
            .unwrap_or_else(|| self.columns.iter().map(|column| column.width).sum());
        self.column_geometry(0.0, width).column_at(x)
    }

    pub(crate) fn resize_handle_at_point(&self, point: crate::core::Point) -> Option<usize> {
        let frame = self.local_frame();
        if !frame.contains(point) {
            return None;
        }
        let geometry = self.column_geometry(0.0, frame.w);
        // 按绘制层级逆序检查句柄，使重叠边缘优先选择视觉最上层列区。
        for zone in COLUMN_PAINT_ORDER.into_iter().rev() {
            // 获取当前列区在表头中的可见裁剪范围。
            let Some(clip) = geometry.clip_for(zone, point.y, 1.0) else {
                // 跳过没有可见面积的列区。
                continue;
                // 结束不可见列区分支。
            };
            // 在当前绘制层内选择距离指针最近的可调整句柄。
            let candidate = geometry
                .columns
                .iter()
                .filter(|laid_out| {
                    // 排除其他绘制层中的列。
                    laid_out.zone == zone
                        // 只保留声明为可调整宽度的列。
                        && self
                            .columns
                            .get(laid_out.index)
                            .is_some_and(|column| column.resizable)
                        // 分组表头的上层区域不能调整叶列宽度。
                        && self.leaf_header_contains(point.y, Some(laid_out.index))
                        // 只接受落在当前列区可见裁剪内的列右边缘。
                        && {
                            // 计算当前列的右边缘位置。
                            let edge = laid_out.x + laid_out.width;
                            // 同时核对边缘可见性与句柄命中半径。
                            edge >= clip.x
                                && edge <= clip.x + clip.w
                                && (point.x - edge).abs() <= COLUMN_RESIZE_HANDLE_HALF_WIDTH
                        }
                    // 结束当前列候选过滤。
                })
                // 同一绘制层内仍以几何距离决定句柄。
                .min_by(|left, right| {
                    // 计算左候选到指针的距离。
                    let left_distance = (point.x - (left.x + left.width)).abs();
                    // 计算右候选到指针的距离。
                    let right_distance = (point.x - (right.x + right.width)).abs();
                    // 返回距离较近的候选顺序。
                    left_distance.total_cmp(&right_distance)
                    // 结束同层距离比较。
                });
            // 当前最高可见层存在句柄时立即返回，避免落到被遮挡的低层列。
            if let Some(column) = candidate {
                // 返回最高层候选的原始列索引。
                return Some(column.index);
                // 结束最高层候选分支。
            }
            // 继续检查下一绘制层。
        }
        // 所有可见列区都没有句柄时返回空。
        None
    }

    pub(crate) fn resize_column_to_pointer(&mut self, drag: TableResizeDrag, pointer_x: f32) {
        let Some(column) = self.columns.get_mut(drag.column) else {
            self.resize_drag.set(None);
            return;
        };
        let width = (drag.start_width + pointer_x - drag.start_x).max(MIN_RESIZABLE_COLUMN_WIDTH);
        if (column.width - width).abs() <= 0.01 {
            return;
        }
        column.width = width;
        self.horizontal_scroll_requires_paint.set(true);
        self.layout_requested.set(true);
    }

    pub(crate) fn restore_column_width(&mut self, drag: TableResizeDrag) {
        let Some(column) = self.columns.get_mut(drag.column) else {
            return;
        };
        if (column.width - drag.start_width).abs() <= 0.01 {
            return;
        }
        column.width = drag.start_width;
        self.clamp_horizontal_scroll();
        self.horizontal_scroll_requires_paint.set(true);
        self.layout_requested.set(true);
    }

    pub(crate) fn cancel_column_resize(&mut self) -> bool {
        let Some(drag) = self.resize_drag.replace(None) else {
            return false;
        };
        self.restore_column_width(drag);
        true
    }

    pub(crate) fn clamp_horizontal_scroll(&self) {
        self.horizontal_scroll.set(
            self.horizontal_scroll
                .get()
                .min(self.horizontal_max_scroll()),
        );
    }

    pub(crate) fn horizontal_max_scroll(&self) -> f32 {
        self.last_frame
            .get()
            .map(|frame| self.column_geometry(0.0, frame.w).max_scroll_x)
            .unwrap_or(0.0)
    }

    // 测试目标保留表格横向滚动偏移观测入口，供表格交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn horizontal_scroll_offset(&self) -> f32 {
        self.horizontal_scroll.get()
    }

    pub(crate) fn body_viewport_height(&self) -> f32 {
        self.last_frame
            .get()
            .map(|f| {
                (finite_nonnegative(f.h)
                    - self.total_header_height()
                    - 1.0
                    - self.pagination_height())
                .max(0.0)
            })
            .unwrap_or(300.0)
    }

    pub(crate) fn expanded_child_row(&self) -> Option<usize> {
        self.expanded_child_row.get()
    }

    pub(crate) fn mark_expanded_child_materialized(&self) {
        self.expanded_child_row.set(self.expanded_row.get());
    }

    pub(crate) fn row_index_at_y(&self, pos_y: f32) -> Option<usize> {
        let header_height = self.total_header_height();
        let viewport_height = self.body_viewport_height();
        if pos_y < header_height || pos_y >= header_height + 1.0 + viewport_height {
            return None;
        }
        let mut local_y = pos_y - header_height - 1.0 + self.body_scroll.scroll_offset();
        if local_y < 0.0 {
            return None;
        }
        if self.expandable {
            if let Some(expanded) = self.expanded_row.get() {
                let expand_start = (expanded + 1) as f32 * self.row_h;
                if local_y >= expand_start && local_y < expand_start + self.expand_height {
                    return None;
                }
                if local_y >= expand_start + self.expand_height {
                    local_y -= self.expand_height;
                }
            }
        }
        let row = (local_y / self.row_h) as usize;
        if row < self.rows.len() {
            Some(row)
        } else {
            None
        }
    }

    pub(crate) fn expand_toggle_hit(&self, pos_x: f32) -> bool {
        let width = self
            .last_frame
            .get()
            .map(|frame| frame.w)
            .unwrap_or_else(|| self.columns.iter().map(|column| column.width).sum());
        pos_x >= (width - 32.0).max(0.0)
    }

    pub(crate) fn body_content_height(&self) -> f32 {
        self.rows.len() as f32 * self.row_h
            + if self.expandable && self.expanded_row.get().is_some() {
                self.expand_height
            } else {
                0.0
            }
    }

    pub(crate) fn visible_row_range(&self, viewport_height: f32) -> (usize, usize) {
        if !self.virtual_scroll {
            return (0, self.rows.len());
        }
        let mut logical_offset = self.body_scroll.scroll_offset();
        if self.expandable {
            if let Some(expanded) = self.expanded_row.get() {
                let expand_start = (expanded + 1) as f32 * self.row_h;
                if logical_offset > expand_start {
                    logical_offset = (logical_offset - self.expand_height).max(expand_start);
                }
            }
        }
        crate::ui::virtualization::virtual_scroll::virtual_list_index_range(
            self.rows.len(),
            self.row_h,
            logical_offset,
            viewport_height,
            2,
        )
    }

    pub(crate) fn push_scroll_delta(&self, dx: f32, dy: f32) {
        if dx.abs() <= 0.01 && dy.abs() <= 0.01 {
            return;
        }
        let current = self.scroll_delta_strip.get();
        self.scroll_delta_strip
            .set((current.0 + dx, current.1 + dy));
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Table {
            columns: self
                .columns
                .iter()
                .map(SnapshotTableColumn::from_table_column)
                .collect(),
            column_groups: self
                .column_groups
                .iter()
                .map(|group| SnapshotTableColumnGroup {
                    title: group.title.clone(),
                    start: group.start,
                    len: group.len,
                })
                .collect(),
            rows: self.rows.clone(),
            row_keys: self.row_keys.clone(),
            view_columns: self.view_columns.clone(),
            row_h: self.row_h,
            header_h: self.header_h,
            expandable: self.expandable,
            expand_height: self.expand_height,
            sortable: self.sortable,
            selection: self.selection,
            bordered: self.bordered,
            loading: self.loading,
            selected_row: self.selected_row.get(),
            checked_rows: self.checked_rows.clone(),
            empty_text: self.empty_text.clone(),
            current_page: self.pagination.as_ref().map(|(current, _, _, _)| *current),
            total: self.pagination.as_ref().map(|(_, total, _, _)| *total),
            page_size: self.page_size,
            virtual_scroll: self.virtual_scroll,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let selected_key = self
            .selected_row
            .get()
            .and_then(|row| self.row_keys.get(row))
            .cloned();
        let checked_keys = self
            .checked_rows
            .iter()
            .filter_map(|row| self.row_keys.get(*row).cloned())
            .collect::<HashSet<_>>();
        let expanded_key = self
            .expanded_row
            .get()
            .and_then(|row| self.row_keys.get(row))
            .cloned();
        self.columns = merge_table_columns(self.columns.as_slice(), next.columns, next.sortable);
        for column in &mut self.columns {
            column.width = finite_nonnegative(column.width);
        }
        self.column_groups = next.column_groups;
        self.rows = next.rows;
        self.row_keys = next.row_keys;
        self.view_columns = next.view_columns;
        self.materialized_cell_range.set(None);
        self.row_h = Self::normalized_row_height(next.row_h);
        self.header_h = next.header_h;
        self.fixed_width = next.fixed_width;
        self.fixed_height = next.fixed_height;
        self.expandable = next.expandable;
        self.expand_height = finite_nonnegative(next.expand_height);
        self.sortable = next.sortable;
        self.selection = next.selection;
        self.bordered = next.bordered;
        self.empty_text = next.empty_text;
        self.page_size = next.page_size;
        self.current_page.set(next.current_page.get());
        self.pagination = next.pagination;
        self.row_click = next.row_click;
        self.virtual_scroll = next.virtual_scroll;
        if self.selection {
            self.checked_rows = self
                .row_keys
                .iter()
                .enumerate()
                .filter_map(|(index, key)| checked_keys.contains(key).then_some(index))
                .collect();
        } else {
            self.checked_rows.clear();
        }
        self.selected_row
            .set(selected_key.and_then(|key| self.row_keys.iter().position(|item| item == &key)));
        self.expanded_row
            .set(expanded_key.and_then(|key| self.row_keys.iter().position(|item| item == &key)));
        self.hover_row.set(None);
        self.pressed_action.set(None);
        self.hover_resize_column.set(None);
        if self.resize_drag.get().is_some_and(|drag| {
            !self
                .columns
                .get(drag.column)
                .is_some_and(|column| column.resizable)
        }) {
            self.resize_drag.set(None);
        }
        self.expanded_child_row.set(None);
        let max = (self.body_content_height() - self.body_viewport_height()).max(0.0);
        self.body_scroll
            .set_scroll_offset(self.body_scroll.scroll_offset().min(max));
        self.horizontal_scroll.set(
            self.horizontal_scroll
                .get()
                .min(self.horizontal_max_scroll()),
        );
    }

    pub(crate) fn cell_view_range_for_frame(&self, frame: Rect) -> (usize, usize) {
        let viewport_height = (finite_nonnegative(frame.h)
            - self.total_header_height()
            - 1.0
            - self.pagination_height())
        .max(0.0);
        let (visible_start, end) = self.visible_row_range(viewport_height);
        let mut start = visible_start;
        if self.has_spans() {
            for row in visible_start..end {
                for &column in &self.view_columns {
                    if let Some((anchor_row, _)) = self.cell_anchor(row, column) {
                        start = start.min(anchor_row);
                    }
                }
            }
        }
        (start, end)
    }

    // 测试目标保留分页控制区域观测入口，供表格交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn pagination_controls_for_test(&self, frame: Rect) -> Option<(Rect, Rect, Rect)> {
        self.pagination_controls(frame)
    }

    pub(crate) fn view_columns(&self) -> &[usize] {
        &self.view_columns
    }

    pub(crate) fn row_keys(&self) -> &[String] {
        &self.row_keys
    }

    pub(crate) fn needs_cell_refresh(
        &self,
        range: (usize, usize),
        mounted_children: usize,
    ) -> bool {
        self.materialized_cell_range.get() != Some(range)
            || mounted_children
                != range
                    .1
                    .saturating_sub(range.0)
                    .saturating_mul(self.view_columns.len())
    }

    pub(crate) fn mark_cells_materialized(&self, range: (usize, usize)) {
        self.materialized_cell_range.set(Some(range));
    }
}

// 仅在单元测试中编译表格列宽句柄契约。
#[cfg(test)]
// 将固定列重叠句柄回归收拢在交互几何模块。
mod tests {
    // 复用表格实现与父模块已导入的几何类型。
    use super::*;
    // 引入固定列回归测试使用的列类型。
    use super::super::types::TableColumn;
    // 引入固定列命中测试使用的坐标类型。
    use crate::core::Point;

    // 标记固定列重叠句柄绘制层级契约。
    #[test]
    // 验证重合边缘命中最后绘制的右固定列句柄。
    fn overlapping_fixed_resize_handles_hit_topmost_painted_zone() {
        // 构造宽八十像素的可调整左固定列。
        let left = TableColumn::new("左列", 80.0)
            // 启用左列调整宽度交互。
            .resizable(true)
            // 将第一列固定到视口左侧。
            .fixed(super::super::types::Fixed::Left);
        // 构造右固定区中边缘落在八十像素处的第一列。
        let right_inner = TableColumn::new("右内列", 20.0)
            // 启用右内列调整宽度交互。
            .resizable(true)
            // 将第二列固定到视口右侧。
            .fixed(super::super::types::Fixed::Right);
        // 构造右固定区最外侧的第二列。
        let right_outer = TableColumn::new("右外列", 20.0)
            // 将第三列固定到视口右侧。
            .fixed(super::super::types::Fixed::Right);
        // 在一百像素视口中让左列和右内列的右边缘同为八十像素。
        let table = Table::new()
            // 安装会形成重叠边缘的三列。
            .columns(vec![left, right_inner, right_outer])
            // 固定视口尺寸以触发左右固定区重叠。
            .size(100.0, 80.0);
        // 重合点必须选择绘制层级更高的右固定列句柄。
        assert_eq!(
            table.resize_handle_at_point(Point::new(80.0, 10.0)),
            Some(1)
        );
        // 结束固定列重叠句柄命中契约。
    }
    // 结束表格列宽句柄测试模块。
}
