//! [`Table`] 组件实现（二）：布局、绘制与交互 — table 子模块。

use crate::core::Rect;
use crate::draw::Radius;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{SnapshotFields, SnapshotTableColumn, SnapshotTableColumnGroup};
use std::cell::Ref;
use std::collections::HashSet;
use std::fmt::Write as _;

use super::config::merge_table_columns;
// 引入共享列区绘制层级与列几何快照。
use super::ResolvedTableVisual;
use super::Table;
use super::geometry::{COLUMN_PAINT_ORDER, TableColumnGeometry};
use super::types::{SortDirection, TableChange, TableResizeDrag, finite_nonnegative};

impl Table {
    pub(crate) fn selection_width(&self) -> f32 {
        if self.selection {
            self.visual.geometry.selection_width
        } else {
            0.0
        }
    }

    pub(crate) fn loading_body_rect(&self, frame: Rect) -> Rect {
        let header_height =
            (self.total_header_height() + self.visual.geometry.body_separator).min(frame.h);
        Rect::new(
            frame.x,
            frame.y + header_height,
            frame.w,
            (frame.h - header_height).max(0.0),
        )
    }

    pub(crate) fn loading_spinner_bounds(&self, frame: Rect) -> Rect {
        let body = self.loading_body_rect(frame);
        let loading = self.visual.loading;
        let radius = loading
            .radius
            .min(body.w.min(body.h) * loading.radius_ratio);
        let dot_radius = radius * loading.dot_radius_ratio;
        let extent = radius + dot_radius + loading.extent_padding;
        Rect::new(
            body.x + body.w * 0.5 - extent,
            body.y + body.h * 0.5 - extent,
            extent * 2.0,
            extent * 2.0,
        )
    }

    // 每帧只求一次相位三角函数，其余圆点通过 UIX 声明的固定角度递推。
    #[inline]
    fn for_each_loading_dot_offset(
        phase: f32,
        radius: f32,
        count: usize,
        step_sin: f32,
        step_cos: f32,
        mut visit: impl FnMut(usize, f32, f32),
    ) {
        // 第一个圆点直接使用当前相位，避免跨帧积累递推误差。
        let (mut sin, mut cos) = phase.sin_cos();
        for index in 0..count {
            visit(index, cos * radius, sin * radius);
            if index + 1 == count {
                break;
            }
            // 复数乘法执行固定角度旋转，不创建临时集合。
            let next_sin = sin * step_cos + cos * step_sin;
            let next_cos = cos * step_cos - sin * step_sin;
            sin = next_sin;
            cos = next_cos;
        }
    }

    pub(crate) fn paint_loading_overlay(
        &self,
        frame: Rect,
        ctx: &mut PaintContext,
        resolved: ResolvedTableVisual,
    ) {
        let body = self.loading_body_rect(frame);
        if body.w <= 0.0 || body.h <= 0.0 {
            return;
        }
        let loading = self.visual.loading;
        ctx.fill_rect(body, resolved.text.with_alpha(loading.overlay_alpha), None);
        let radius = loading
            .radius
            .min(body.w.min(body.h) * loading.radius_ratio);
        if radius <= 0.0 {
            return;
        }
        let cx = body.x + body.w * self.visual.frame.center_ratio;
        let cy = body.y + body.h * self.visual.frame.center_ratio;
        let dot_radius = radius * loading.dot_radius_ratio;
        Self::for_each_loading_dot_offset(
            self.loading_phase,
            radius,
            loading.dot_count,
            loading.step_sin,
            loading.step_cos,
            |index, dx, dy| {
                let opacity = loading.opacity_base
                    + index as f32 / loading.dot_count as f32 * loading.opacity_range;
                ctx.fill_circle(
                    cx + dx,
                    cy + dy,
                    dot_radius,
                    resolved
                        .primary
                        .with_alpha((resolved.primary.a as f32 * opacity) as u8),
                );
            },
        );
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
            self.visual.pagination.height
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
        let pagination = self.visual.pagination;
        let footer_height = pagination.height.min(frame.h.max(0.0));
        let inset = pagination
            .inset
            .min(frame.w.max(0.0) * pagination.inset_ratio);
        let available = (frame.w - inset * 2.0).max(0.0);
        let gap = pagination.gap.min(available * pagination.gap_ratio);
        let label_width = pagination
            .label_width
            .min(available * pagination.label_width_ratio);
        let item_size = pagination
            .item_size
            .min(((available - label_width - gap * 2.0) * pagination.center_ratio).max(0.0))
            .min(footer_height);
        let controls_width = item_size * 2.0 + label_width + gap * 2.0;
        let x = frame.x + (frame.w - inset - controls_width).max(0.0);
        let footer_y = frame.y + frame.h - footer_height;
        let y = footer_y + (footer_height - item_size) * pagination.center_ratio;
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

    fn pagination_label(&self, current: usize, total: usize) -> Ref<'_, str> {
        let needs_refresh = {
            let cache = self.pagination_label_cache.borrow();
            cache.value.is_empty() || cache.current != current || cache.total != total
        };
        if needs_refresh {
            let mut cache = self.pagination_label_cache.borrow_mut();
            cache.current = current;
            cache.total = total;
            cache.value.clear();
            let _ = write!(cache.value, "{current} / {total}");
        }
        Ref::map(self.pagination_label_cache.borrow(), |cache| {
            cache.value.as_str()
        })
    }

    pub(crate) fn paint_pagination(
        &self,
        frame: Rect,
        ctx: &mut PaintContext,
        resolved: ResolvedTableVisual,
    ) {
        let Some((previous, label, next)) = self.pagination_controls(frame) else {
            return;
        };
        let current = self.pagination_current();
        let total_pages = self.pagination_total_pages();
        let footer = Rect::new(
            frame.x,
            frame.y + (frame.h - self.visual.pagination.height).max(0.0),
            frame.w,
            self.visual.pagination.height.min(frame.h),
        );
        let pagination = self.visual.pagination;
        let radius = Some(Radius::uniform(resolved.radius));
        ctx.fill_rect(footer, resolved.alternate_background, None);
        ctx.fill_rect(
            Rect::new(footer.x, footer.y, footer.w, pagination.divider_width),
            resolved.border,
            None,
        );
        for button in [previous, next] {
            ctx.fill_rect(button, resolved.alternate_background, radius);
            ctx.stroke_rect(button, resolved.border, pagination.button_stroke, radius);
        }
        crate::ui::widgets::Icon::paint_in_frame(
            ctx,
            pagination.previous_icon,
            previous,
            if current <= 1 {
                resolved.text_secondary
            } else {
                resolved.text
            },
            pagination.icon_size,
        );
        let page_label = self.pagination_label(current, total_pages);
        ctx.text_center(
            &page_label,
            label,
            resolved.text,
            pagination.label_font_size,
        );
        crate::ui::widgets::Icon::paint_in_frame(
            ctx,
            pagination.next_icon,
            next,
            if current >= total_pages {
                resolved.text_secondary
            } else {
                resolved.text
            },
            pagination.icon_size,
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

    // 生产热路径借用 Table 自有缓存，调用方在只读阶段内消费。
    pub(crate) fn column_geometry_ref(
        &self,
        origin_x: f32,
        viewport_width: f32,
    ) -> Ref<'_, TableColumnGeometry> {
        let selection_width = self.selection_width();
        let scroll_x = self.horizontal_scroll.get();
        {
            let mut cache = self.column_geometry_cache.borrow_mut();
            cache.resolve(
                &self.columns,
                origin_x,
                viewport_width,
                selection_width,
                scroll_x,
            );
        }
        Ref::map(self.column_geometry_cache.borrow(), |cache| {
            cache.geometry()
        })
    }

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

    pub(crate) fn column_at_x(&self, x: f32) -> Option<usize> {
        let width = self
            .last_frame
            .get()
            .map(|frame| frame.w)
            .unwrap_or_else(|| self.columns.iter().map(|column| column.width).sum());
        self.column_geometry_ref(0.0, width).column_at(x)
    }

    pub(crate) fn resize_handle_at_point(&self, point: crate::core::Point) -> Option<usize> {
        let frame = self.local_frame();
        if !frame.contains(point) {
            return None;
        }
        let geometry = self.column_geometry_ref(0.0, frame.w);
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
                                && (point.x - edge).abs()
                                    <= self.visual.geometry.resize_handle_half_width
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
        let width = (drag.start_width + pointer_x - drag.start_x)
            .max(self.visual.geometry.min_resizable_column_width);
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
            .map(|frame| self.column_geometry_ref(0.0, frame.w).max_scroll_x)
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
                    - self.visual.geometry.body_separator
                    - self.pagination_height())
                .max(0.0)
            })
            .unwrap_or(self.visual.geometry.default_height)
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
        if pos_y < header_height
            || pos_y >= header_height + self.visual.geometry.body_separator + viewport_height
        {
            return None;
        }
        let mut local_y = pos_y - header_height - self.visual.geometry.body_separator
            + self.body_scroll.scroll_offset();
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
        pos_x >= (width - self.visual.geometry.selection_width).max(0.0)
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
        self.visual = next.visual;
        self.row_h_authored = next.row_h_authored;
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
            - self.visual.geometry.body_separator
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
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/display/table/table_b__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
