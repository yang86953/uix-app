//! 数据表格组件：静态列 / 数据驱动 / 虚拟滚动 / 分页与排序。
//!
//! 子模块划分（P2 行数治理）：`types` 公开类型、`builder` 构建器、
//! `table_a` / `table_b` 组件实现、`tests` 测试。

use crate::core::{Constraints, Rect, Size};
use crate::draw::Radius;
use crate::ui::virtualization::virtual_scroll::VirtualListScroll;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{
    EventResult, KeyCode, LayoutChild, SemanticEvent, SystemEvent, WidgetId, WidgetTree,
};
use crate::widget;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;

mod builder;
// 收拢自定义单元格子树的完整 frame 与片段裁剪布局。
mod child_layout;
mod config;
pub(crate) mod geometry;
mod header;
mod presentation;
mod table_a;
mod table_b;
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/display/table/tests.rs"]
mod tests;
pub(crate) mod types;

// 引入共享列区绘制层级与列几何快照。
pub use builder::TableBuilder;
use geometry::COLUMN_PAINT_ORDER;
use presentation::*;
pub(crate) use presentation::{ResolvedTableVisual, TableVisual};
pub(crate) use types::TableCellRenderer;
use types::{
    ColumnGroupRange, TablePaginationCallback, TablePointerAction, TableResizeDrag,
    TableRowClickCallback, finite_nonnegative,
};
pub use types::{
    DataTable, ExpandRenderer, Fixed, SortDirection, TableChange, TableColumn, TableColumnGroup,
    TableDataColumn, TableDataError, TableEmptyRenderer, TablePagination, TableRow,
};

widget! {
    /// 拥有列、行、选择、展开、排序与虚拟滚动状态的数据表格组件。
    pub struct Table {
        columns: Vec<TableColumn>,
        column_groups: Vec<ColumnGroupRange>,
        pub(crate) rows: Vec<TableRow>,
        row_keys: Vec<String>,
        view_columns: Vec<usize>,
        materialized_cell_range: Cell<Option<(usize, usize)>>,
        pub(crate) row_h: f32,
        // 标记行高是否由 Rust 调用方显式覆盖。
        #[snapshot(skip)]
        pub(crate) row_h_authored: bool,
        header_h: f32,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        selected_row: Cell<Option<usize>>,
        hover_row: Cell<Option<usize>>,
        /// 多选：选中行索引集合。
        checked_rows: Vec<usize>,
        /// 扩展行：当前展开的行索引。
        expanded_row: Cell<Option<usize>>,
        expanded_child_row: Cell<Option<usize>>,
        expandable: bool,
        expand_height: f32,
        sortable: bool,
        selection: bool,
        bordered: bool,
        loading: bool,
        loading_phase: f32,
        loading_dirty: bool,
        /// 空状态文案。
        empty_text: String,
        /// 当前分页。
        current_page: Cell<usize>,
        page_size: usize,
        pagination: Option<(usize, usize, usize, TablePaginationCallback)>,
        row_click: Option<TableRowClickCallback>,
        pending_change: RefCell<Option<String>>,
        virtual_scroll: bool,
        pub(crate) body_scroll: VirtualListScroll,
        horizontal_scroll: Cell<f32>,
        horizontal_scroll_requires_paint: Cell<bool>,
        scroll_delta_strip: Cell<(f32, f32)>,
        pub(crate) last_frame: Cell<Option<Rect>>,
        layout_requested: Cell<bool>,
        focused: bool,
        pressed_action: Cell<Option<TablePointerAction>>,
        resize_drag: Cell<Option<TableResizeDrag>>,
        hover_resize_column: Cell<Option<usize>>,
        // 全部实例共享 UIX 声明固化后的只读视觉配置。
        #[snapshot(skip)]
        pub(crate) visual: &'static TableVisual,
        // 复用分页标签字符串，页码未变化时不产生临时分配。
        #[snapshot(skip)]
        pub(crate) pagination_label_cache: RefCell<TablePaginationLabelCache>,
    }

    tab_index => (&self) -> i32 {
        i32::from(!self.loading && (!self.rows.is_empty() || self.pagination.is_some()))
    }

    measure => (&self, constraints: Constraints) -> Size {
        let w: f32 = self
            .columns
            .iter()
            .map(|column| finite_nonnegative(column.width))
            .sum::<f32>()
            + self.selection_width();
        let extra = if self.expandable && self.expanded_row.get().is_some() { self.expand_height } else { 0.0 };
        let body_h = self.rows.len() as f32 * self.row_h + extra;
        let body_separator = if !self.rows.is_empty() || self.loading {
            self.visual.geometry.body_separator
        } else {
            0.0
        };
        let h = self.total_header_height()
            + body_separator
            + body_h
            + self.pagination_height();
        constraints.clamp(Size::new(
            self.fixed_width.unwrap_or(w),
            self.fixed_height
                .unwrap_or(h.max(self.visual.geometry.min_height)),
        ))
    }

    scroll_delta_for_dirty => (&self) -> Option<(f32, f32)> {
        if self.horizontal_scroll_requires_paint.replace(false) {
            self.scroll_delta_strip.set((0.0, 0.0));
            return None;
        }
        let delta = self.scroll_delta_strip.get();
        if delta.0.abs() > 0.01 || delta.1.abs() > 0.01 {
            self.scroll_delta_strip.set((0.0, 0.0));
            Some(delta)
        } else {
            None
        }
    }

    scroll_composite_viewport => (&self, frame: Rect) -> Option<Rect> {
        let header_height =
            self.total_header_height() + self.visual.geometry.body_separator;
        let body_height = (frame.h - header_height - self.pagination_height()).max(0.0);
        (body_height > 0.0).then(|| {
            Rect::new(frame.x, frame.y + header_height, frame.w, body_height)
        })
    }

    viewport_scroll_offset => (&self) -> Option<(f32, f32)> {
        Some((0.0, self.body_scroll.scroll_offset()))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if self.loading {
            let changed = self.pressed_action.replace(None).is_some()
                | self.hover_row.replace(None).is_some()
                | self.hover_resize_column.replace(None).is_some()
                | self.cancel_column_resize();
            if matches!(event, SystemEvent::FocusOut) {
                self.focused = false;
            }
            let blocks_interaction = match event {
                SystemEvent::PointerDown { pos, .. }
                | SystemEvent::PointerUp { pos, .. }
                | SystemEvent::PointerMove { pos, .. }
                | SystemEvent::Wheel { pos, .. } => self.local_frame().contains(*pos),
                SystemEvent::KeyDown { .. } => self.focused,
                _ => false,
            };
            return if changed || blocks_interaction || matches!(event, SystemEvent::FocusOut) {
                EventResult::Handled
            } else {
                EventResult::NotHandled
            };
        }
        match event {
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.pressed_action.set(None);
                self.cancel_column_resize();
                self.hover_resize_column.set(None);
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } if self.focused && self.pagination.is_some() => {
                let current = self.pagination_current();
                let next = match key {
                    KeyCode::Left | KeyCode::PageUp => current.saturating_sub(1).max(1),
                    KeyCode::Right | KeyCode::PageDown => {
                        current.saturating_add(1).min(self.pagination_total_pages())
                    }
                    KeyCode::Home => 1,
                    KeyCode::End => self.pagination_total_pages(),
                    _ => return EventResult::NotHandled,
                };
                self.commit_page_change(next);
                EventResult::Handled
            }
            SystemEvent::Wheel { pos, delta } => {
                if !self.body_contains(*pos) {
                    return EventResult::NotHandled;
                }
                let viewport_h = self.body_viewport_height();
                let old_y = self.body_scroll.scroll_offset();
                let max = (self.body_content_height() - viewport_h).max(0.0);
                let next_y = (old_y + delta.y * self.visual.geometry.wheel_step).clamp(0.0, max);
                self.body_scroll.set_scroll_offset(next_y);
                let dy = next_y - old_y;

                let old_x = self.horizontal_scroll.get();
                let next_x = (old_x + delta.x * self.visual.geometry.wheel_step)
                    .clamp(0.0, self.horizontal_max_scroll());
                self.horizontal_scroll.set(next_x);
                let dx = next_x - old_x;
                if dx.abs() > 0.01 {
                    self.horizontal_scroll_requires_paint.set(true);
                }
                if dy.abs() > 0.01 {
                    self.push_scroll_delta(0.0, dy);
                }
                if dx.abs() > 0.01 || dy.abs() > 0.01 {
                    if dx.abs() > 0.01 && !self.view_columns.is_empty() {
                        self.layout_requested.set(true);
                    }
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerDown {
                pos,
                button: crate::ui::MouseButton::Left,
                ..
            } => {
                if let Some(column) = self.resize_handle_at_point(*pos) {
                    let start_width = self.columns[column].width;
                    self.resize_drag.set(Some(TableResizeDrag {
                        column,
                        start_x: pos.x,
                        start_width,
                    }));
                    self.hover_resize_column.set(Some(column));
                    self.pressed_action.set(None);
                    return EventResult::Handled;
                }
                if let Some(action) = self.action_at_point(*pos) {
                    self.pressed_action.set(Some(action));
                    self.hover_row.set(Self::action_row(action));
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerUp {
                pos,
                button: crate::ui::MouseButton::Left,
                ..
            } => {
                if let Some(drag) = self.resize_drag.replace(None) {
                    let valid_release = self.local_frame().contains(*pos)
                        && self.leaf_header_contains(pos.y, Some(drag.column));
                    if !valid_release {
                        self.restore_column_width(drag);
                    } else {
                        self.clamp_horizontal_scroll();
                    }
                    self.hover_resize_column
                        .set(valid_release.then_some(drag.column));
                    return EventResult::Handled;
                }
                let Some(pressed) = self.pressed_action.replace(None) else {
                    return EventResult::NotHandled;
                };
                let released = self.action_at_point(*pos);
                self.hover_row.set(released.and_then(Self::action_row));
                if released == Some(pressed) {
                    self.commit_pointer_action(pressed);
                }
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if let Some(drag) = self.resize_drag.get() {
                    if !self.local_frame().contains(*pos)
                        || !self.leaf_header_contains(pos.y, Some(drag.column))
                    {
                        self.cancel_column_resize();
                        self.hover_resize_column.set(None);
                    } else {
                        self.resize_column_to_pointer(drag, pos.x);
                        self.hover_resize_column.set(Some(drag.column));
                    }
                    return EventResult::Handled;
                }
                let resize_column = self.resize_handle_at_point(*pos);
                let resize_changed = self.hover_resize_column.replace(resize_column) != resize_column;
                let next = resize_column
                    .is_none()
                    .then(|| self.action_at_point(*pos).and_then(Self::action_row))
                    .flatten();
                if resize_changed | (self.hover_row.replace(next) != next) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave => {
                let changed = self.hover_row.replace(None).is_some()
                    | self.pressed_action.replace(None).is_some()
                    | self.hover_resize_column.replace(None).is_some()
                    | self.cancel_column_resize();
                if changed {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|value| SemanticEvent::change(id, value))
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    wants_continuous_pointer_move => (&self) -> bool { self.resize_drag.get().is_some() }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let frame = Self::normalized_frame(frame);
        self.last_frame.set(Some(frame));
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let resolved = self.visual.resolve(ctx.tokens());
        let frame_visual = self.visual.frame;
        let radius = resolved
            .radius
            .min(frame.w.min(frame.h) * frame_visual.radius_frame_ratio);
        let r = Some(Radius::uniform(radius));
        let mut y = frame.y;
        let sel = self.selected_row.get();
        let hover = self.hover_row.get();
        let expanded = self.expanded_row.get();
        let column_geometry = self.column_geometry(frame.x, frame.w);
        ctx.push_clip(frame);

        // 空状态
        if self.rows.is_empty() && !self.loading {
            let loc = crate::ui::widget_runtime::locale::use_locale();
            let empty = if self.empty_text.is_empty() { loc.empty_data } else { &self.empty_text };
            let horizontal_inset = frame_visual
                .empty_inset
                .min(frame.w * frame_visual.empty_inset_ratio);
            Self::paint_single_line(
                ctx,
                empty,
                Rect::new(
                    frame.x + horizontal_inset,
                    frame.y,
                    (frame.w - horizontal_inset * 2.0).max(0.0),
                    (frame.h - self.pagination_height()).max(0.0),
                ),
                resolved.text_secondary,
                frame_visual.empty_font_size,
            );
            if self.bordered {
                ctx.stroke_rect(frame, resolved.border, frame_visual.border_width, r);
            }
            self.paint_pagination(frame, ctx, resolved);
            ctx.pop_clip();
            return;
        }

        header::paint(
            self,
            Rect::new(frame.x, y, frame.w, frame.h),
            ctx,
            &column_geometry,
            resolved,
        );
        y += self.total_header_height();

        // 分隔线
        ctx.fill_rect(
            Rect::new(
                frame.x,
                y,
                frame.w,
                self.visual.geometry.body_separator,
            ),
            resolved.border,
            None,
        );
        y += self.visual.geometry.body_separator;

        // 数据行（虚拟滚动：仅绘制 viewport ± overscan）
        let body_top = y;
        let body_viewport_h = self.body_viewport_height();
        let (start, end) = self.visible_row_range(body_viewport_h);
        let body_clip = Rect::new(frame.x, body_top, frame.w, body_viewport_h);
        ctx.push_clip(body_clip);

        for actual_ri in start..end {
            let Some(row) = self.rows.get(actual_ri) else {
                continue;
            };
            let is_selected = sel == Some(actual_ri);
            let is_hovered = hover == Some(actual_ri);
            let is_checked = self.checked_rows.contains(&actual_ri);
            let is_expanded = expanded == Some(actual_ri);
            let is_pressed = self
                .pressed_action
                .get()
                .and_then(Self::action_row)
                == Some(actual_ri)
                && is_hovered;

            let expanded_offset = if expanded.is_some_and(|expanded_row| actual_ri > expanded_row)
            {
                self.expand_height
            } else {
                0.0
            };
            let row_y = body_top + actual_ri as f32 * self.row_h + expanded_offset
                - self.body_scroll.scroll_offset();
            if row_y + self.row_h < body_top || row_y > body_top + body_viewport_h {
                continue;
            }

            let row_bg = if is_pressed {
                resolved.pressed_background
            } else if is_selected {
                resolved.selected_background
            } else if is_hovered {
                resolved.hover_background
            } else if actual_ri.is_multiple_of(2) {
                resolved.background
            } else {
                resolved.alternate_background
            };
            let row_rect = Rect::new(frame.x, row_y, frame.w, self.row_h);
            ctx.fill_rect(row_rect, row_bg, None);

            if self.selection {
                crate::ui::widgets::Icon::paint_in_frame(
                    ctx,
                    if is_checked {
                        frame_visual.checked_icon
                    } else {
                        frame_visual.unchecked_icon
                    },
                    Rect::new(
                        frame.x + frame_visual.selection_icon_x,
                        row_rect.y,
                        frame_visual.selection_icon_width,
                        row_rect.h,
                    ),
                    resolved.primary,
                    frame_visual.selection_icon_size,
                );
                if self.bordered {
                    ctx.fill_rect(
                        Rect::new(
                            frame.x + self.selection_width()
                                - frame_visual.selection_divider_width,
                            row_rect.y,
                            frame_visual.selection_divider_width,
                            row_rect.h,
                        ),
                        resolved.border,
                        None,
                    );
                }
            }

            // 表体复用表头的共享列区层级。
            for zone in COLUMN_PAINT_ORDER {
                let Some(clip) = column_geometry.clip_for(zone, row_rect.y, row_rect.h) else {
                    continue;
                };
                ctx.push_clip(clip);
                for laid_out in column_geometry
                    .columns
                    .iter()
                    .filter(|column| column.zone == zone)
                {
                    if self.is_cell_covered(actual_ri, laid_out.index) {
                        continue;
                    }
                    let row_span = self.row_span(actual_ri, laid_out.index);
                    let col_span = self.col_span(actual_ri, laid_out.index);
                    let cell_height = self.span_height(actual_ri, row_span);
                    // 用共享列几何解析合并单元格矩形，避免绘制与子项布局各自推导。
                    let Some(cell_frame) = column_geometry.span_bounds(
                        // 以当前未覆盖列作为合并锚点。
                        laid_out.index,
                        // 传入当前锚点声明的逻辑列跨度。
                        col_span,
                        // 普通行绘制从当前物理行顶部开始。
                        row_rect.y,
                        // 保留行合并计算后的完整高度。
                        cell_height,
                    )
                    else {
                        // 缺少锚点几何时跳过当前单元格。
                        continue;
                    };
                    if !self.view_columns.contains(&laid_out.index) {
                        let cell = row
                            .get(laid_out.index)
                            .map(String::as_str)
                            .unwrap_or("");
                        let tc = if is_selected {
                            resolved.primary
                        } else {
                            resolved.text
                        };
                        if let Some(cell_frame) = cell_frame.intersect(&clip) {
                            let horizontal_inset = frame_visual
                                .cell_inset
                                .min(cell_frame.w * frame_visual.cell_inset_ratio);
                            Self::paint_single_line(
                                ctx,
                                cell,
                                Rect::new(
                                    cell_frame.x + horizontal_inset,
                                    cell_frame.y,
                                    (cell_frame.w - horizontal_inset * 2.0).max(0.0),
                                    cell_frame.h,
                                ),
                                tc,
                                frame_visual.cell_font_size,
                            );
                        }
                    }
                    if self.bordered {
                        ctx.fill_rect(
                            Rect::new(
                                // 纵向边界跟随合并矩形的真实物理右边缘。
                                cell_frame.x + cell_frame.w - frame_visual.border_width,
                                row_rect.y,
                                frame_visual.border_width,
                                cell_height,
                            ),
                            resolved.border,
                            None,
                        );
                    }
                }
                ctx.pop_clip();
            }

            if actual_ri + 1 < end || is_expanded {
                ctx.fill_rect(
                    Rect::new(
                        frame.x,
                        row_y + self.row_h,
                        frame.w,
                        frame_visual.border_width,
                    ),
                    resolved.border,
                    None,
                );
            }

            // 扩展行内容
            if is_expanded {
                let expand_rect = Rect::new(
                    frame.x,
                    row_y + self.row_h,
                    frame.w,
                    self.expand_height,
                );
                ctx.fill_rect(expand_rect, resolved.alternate_background, None);
            }
        }

        // Row backgrounds and separators are painted one physical row at a time. Repaint merged
        // anchors after those rows so a row-spanning cell keeps one background, border and text
        // rectangle instead of being split by later rows.
        if self.has_spans() {
            let mut merged_anchors = HashSet::new();
            for visible_row in start..end {
                for column in 0..self.columns.len() {
                    if let Some(anchor) = self.cell_anchor(visible_row, column) {
                        let row_span = self.row_span(anchor.0, anchor.1);
                        let col_span = self.col_span(anchor.0, anchor.1);
                        if row_span > 1 || col_span > 1 {
                            merged_anchors.insert(anchor);
                        }
                    }
                }
            }
            let mut merged_anchors = merged_anchors.into_iter().collect::<Vec<_>>();
            merged_anchors.sort_unstable();
            for (row_index, column_index) in merged_anchors {
                let Some(row) = self.rows.get(row_index) else {
                    continue;
                };
                let row_span = self.row_span(row_index, column_index);
                let col_span = self.col_span(row_index, column_index);
                let cell_height = self.span_height(row_index, row_span);
                let expanded_offset = if expanded.is_some_and(|expanded_row| row_index > expanded_row)
                {
                    self.expand_height
                } else {
                    0.0
                };
                let row_y = body_top + row_index as f32 * self.row_h + expanded_offset
                    - self.body_scroll.scroll_offset();
                // 用共享列几何解析最终重绘与命中一致的合并矩形。
                let Some(cell_frame) = column_geometry.span_bounds(
                    // 传入当前合并锚点列。
                    column_index,
                    // 传入当前锚点声明的逻辑列跨度。
                    col_span,
                    // 传入滚动与展开偏移后的纵坐标。
                    row_y,
                    // 传入完整跨行高度。
                    cell_height,
                )
                else {
                    // 缺少锚点几何时跳过当前合并单元格。
                    continue;
                };
                // 完整合并矩形与表体视口无交集时无需访问任何列区片段。
                if cell_frame.intersect(&body_clip).is_none() {
                    continue;
                }
                let is_selected = sel == Some(row_index);
                let is_hovered = hover == Some(row_index);
                let is_pressed = self
                    .pressed_action
                    .get()
                    .and_then(Self::action_row)
                    == Some(row_index)
                    && is_hovered;
                let cell_bg = if is_pressed {
                    resolved.pressed_background
                } else if is_selected {
                    resolved.selected_background
                } else if is_hovered {
                    resolved.hover_background
                } else if row_index.is_multiple_of(2) {
                    resolved.background
                } else {
                    resolved.alternate_background
                };

                // 按共享列区层级重绘跨度在每个固定区中的真实可见片段。
                for zone in COLUMN_PAINT_ORDER {
                    // 解析当前列区中属于该逻辑跨度且最终可见的裁剪。
                    let Some(zone_clip) = column_geometry.merged_span_repaint_clip_for(
                        // 传入当前合并锚点列。
                        column_index,
                        // 传入当前锚点声明的逻辑列跨度。
                        col_span,
                        // 传入当前共享绘制列区。
                        zone,
                        // 继承合并单元格纵坐标。
                        cell_frame.y,
                        // 继承合并单元格完整跨行高度。
                        cell_frame.h,
                    )
                    else {
                        // 当前跨度未覆盖该列区时直接进入下一层。
                        continue;
                    };
                    // 将背景、内容与边框限制在当前列区的跨度片段内。
                    ctx.push_clip(zone_clip);
                    // 为当前可见片段补绘统一的合并单元格背景。
                    ctx.fill_rect(cell_frame, cell_bg, None);
                    // 普通文本列由表格自身绘制，自定义 View 列留给子树。
                    if !self.view_columns.contains(&column_index) {
                        // 从锚点列读取合并单元格唯一文本。
                        let cell = row.get(column_index).map(String::as_str).unwrap_or("");
                        // 水平留白按完整合并矩形收敛，跨区片段只负责裁剪。
                        let horizontal_inset = frame_visual
                            .cell_inset
                            .min(cell_frame.w * frame_visual.cell_inset_ratio);
                        // 在统一矩形中绘制一次逻辑内容的当前可见切片。
                        Self::paint_single_line(
                            ctx,
                            cell,
                            Rect::new(
                                cell_frame.x + horizontal_inset,
                                cell_frame.y,
                                (cell_frame.w - horizontal_inset * 2.0).max(0.0),
                                cell_frame.h,
                            ),
                            if is_selected {
                                resolved.primary
                            } else {
                                resolved.text
                            },
                            frame_visual.cell_font_size,
                        );
                    }
                    // 带边框表格沿完整逻辑矩形绘制外框并由片段裁剪。
                    if self.bordered {
                        // 提交完整合并矩形描边。
                        ctx.stroke_rect(
                            cell_frame,
                            resolved.border,
                            frame_visual.border_width,
                            None,
                        );
                    } else {
                        // 无边框表格只补绘合并单元格底部分隔线。
                        ctx.fill_rect(
                            Rect::new(
                                cell_frame.x,
                                cell_frame.y + cell_frame.h,
                                cell_frame.w,
                                frame_visual.border_width,
                            ),
                            resolved.border,
                            None,
                        );
                    }
                    // 恢复进入当前列区片段前的裁剪状态。
                    ctx.pop_clip();
                }
            }
        }

        // Expansion affordances belong to physical rows. Paint them after merged cells so a
        // colspan reaching the trailing edge cannot obscure the icon.
        if self.expandable {
            for actual_ri in start..end {
                let expanded_offset = if expanded.is_some_and(|expanded_row| actual_ri > expanded_row)
                {
                    self.expand_height
                } else {
                    0.0
                };
                let row_y = body_top + actual_ri as f32 * self.row_h + expanded_offset
                    - self.body_scroll.scroll_offset();
                let icon_size = frame_visual
                    .expand_icon_size
                    .min(self.row_h)
                    .min(frame.w);
                crate::ui::widgets::Icon::paint_in_frame(
                    ctx,
                    if expanded == Some(actual_ri) {
                        frame_visual.expanded_icon
                    } else {
                        frame_visual.collapsed_icon
                    },
                    Rect::new(
                        frame.x
                            + (frame.w - icon_size - frame_visual.expand_icon_right).max(0.0),
                        row_y + (self.row_h - icon_size) * frame_visual.center_ratio,
                        icon_size,
                        icon_size,
                    ),
                    resolved.text_secondary,
                    frame_visual.expand_icon_font_size,
                );
            }
        }

        ctx.pop_clip();
        self.paint_pagination(frame, ctx, resolved);
        if self.loading {
            self.paint_loading_overlay(frame, ctx, resolved);
        }
        if self.bordered {
            ctx.stroke_rect(frame, resolved.border, frame_visual.border_width, r);
        }
        if self.focused && tree.keyboard_focus_visible() {
            let inset = frame_visual
                .focus_inset
                .min(frame.w * frame_visual.center_ratio)
                .min(frame.h * frame_visual.center_ratio);
            ctx.stroke_rect(
                Rect::new(
                    frame.x + inset,
                    frame.y + inset,
                    (frame.w - inset * 2.0).max(0.0),
                    (frame.h - inset * 2.0).max(0.0),
                ),
                resolved.primary,
                frame_visual.focus_stroke,
                Some(Radius::uniform(
                    radius.min(
                        (frame.w - inset * 2.0).min(frame.h - inset * 2.0)
                            * frame_visual.center_ratio,
                    ),
                )),
            );
        }
        ctx.pop_clip();
    }

    update_animation => (&mut self, dt: f64) -> bool {
        self.loading_dirty = false;
        if !self.loading {
            return false;
        }
        let before = self.loading_phase;
        self.loading_phase = (self.loading_phase
            + dt.max(0.0) as f32 * std::f32::consts::TAU
                / self.visual.loading.duration_seconds)
            .rem_euclid(std::f32::consts::TAU);
        self.loading_dirty = (self.loading_phase - before).abs() > f32::EPSILON;
        true
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.loading_dirty {
            self.loading_spinner_bounds(Self::normalized_frame(frame))
        } else {
            Rect::zero()
        }
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        let header_height = self.total_header_height();
        Some(Rect::new(
            frame.x,
            frame.y + header_height + self.visual.geometry.body_separator,
            frame.w,
            (frame.h
                - header_height
                - self.visual.geometry.body_separator
                - self.pagination_height())
            .max(0.0),
        ))
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        // 由独立辅助统一生成完整 View frame 与父级片段裁剪。
        self.layout_table_children(frame, children, tree)
    }
}

// 把表格数据、交互状态与 UIX 静态视觉融合为单一根节点。
fn build_table_view(mut kernel: Table, declared_visual: TableVisual) -> crate::ui::view::ViewNode {
    let visual = UIX_TABLE_VISUAL.get_or_init(|| declared_visual);
    if !kernel.row_h_authored {
        kernel.row_h = visual.geometry.row_height;
    }
    kernel.header_h = visual.geometry.header_height;
    kernel.visual = visual;
    crate::ui::view::ViewNode::leaf(kernel)
}

// 让 Table 与泛型 DataTable 的最终内核统一进入同一 UIX 根。
pub(crate) fn build_table_uix_root(kernel: Table) -> crate::ui::view::ViewNode {
    crate::uix!("src/ui/widgets/display/table/table.uix")
}

impl TableChange {
    fn payload(&self) -> String {
        let sort_column = self
            .sort_column
            .map(|idx| idx.to_string())
            .unwrap_or_default();
        let sort_direction = match self.sort_direction {
            SortDirection::None => "none",
            SortDirection::Asc => "asc",
            SortDirection::Desc => "desc",
        };
        format!(
            "sort_column={};sort_direction={};page={};page_size={}",
            sort_column, sort_direction, self.page, self.page_size
        )
    }
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}
