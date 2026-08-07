//! 数据表格组件：静态列 / 数据驱动 / 虚拟滚动 / 分页与排序。
//!
//! 子模块划分（P2 行数治理）：[`types`] 公开类型、[`builder`] 构建器、
//! [`table_a`] / [`table_b`] 组件实现、[`tests`] 测试。

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::Radius;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::render_handler::RenderHandlerRegistration;
use crate::ui::virtualization::virtual_scroll::VirtualListScroll;
use crate::ui::{
    ComponentId, EventResult, KeyCode, LayoutChild, SemanticEvent, SnapshotFields,
    SnapshotTableColumn, SnapshotTableColumnGroup, SystemEvent, WidgetTree,
};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::rc::Rc;

mod builder;
mod config;
pub(crate) mod geometry;
mod header;
mod table_a;
mod table_b;
#[cfg(test)]
mod tests;
pub(crate) mod types;

use config::{flatten_column_groups, merge_table_columns};
// 引入共享列区绘制层级与列几何快照。
use geometry::{TableColumnGeometry, COLUMN_PAINT_ORDER};
use types::{
    finite_nonnegative, ColumnGroupRange, TablePaginationCallback, TablePointerAction,
    TableResizeDrag, TableRowClickCallback,
};
pub use builder::TableBuilder;
pub use types::{
    DataTable, ExpandRenderer, SortDirection, Fixed, TableChange, TableColumn, TableColumnGroup,
    TableDataColumn, TableDataError, TableEmptyRenderer, TablePagination, TableRow,
};
pub(crate) use types::TableCellRenderer;

component! {
    pub struct Table {
        columns: Vec<TableColumn>,
        column_groups: Vec<ColumnGroupRange>,
        pub(crate) rows: Vec<TableRow>,
        row_keys: Vec<String>,
        view_columns: Vec<usize>,
        materialized_cell_range: Cell<Option<(usize, usize)>>,
        pub(crate) row_h: f32,
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
            1.0
        } else {
            0.0
        };
        let h = self.total_header_height()
            + body_separator
            + body_h
            + self.pagination_height();
        constraints.clamp(Size::new(
            self.fixed_width.unwrap_or(w),
            self.fixed_height.unwrap_or(h.max(60.0)),
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
        let header_height = self.total_header_height() + 1.0;
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
                let next_y = (old_y + delta.y * 40.0).clamp(0.0, max);
                self.body_scroll.set_scroll_offset(next_y);
                let dy = next_y - old_y;

                let old_x = self.horizontal_scroll.get();
                let next_x = (old_x + delta.x * 40.0)
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

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
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
        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let primary = ctx.tokens().color_primary();
        let hover_bg = ctx.tokens().color_fill_quaternary();
        let sel_bg = ctx.tokens().color_primary_bg();
        let radius = ctx
            .tokens()
            .border_radius_sm()
            .min(frame.w.min(frame.h) * 0.5);
        let r = Some(Radius::uniform(radius));
        let mut y = frame.y;
        let sel = self.selected_row.get();
        let hover = self.hover_row.get();
        let expanded = self.expanded_row.get();
        let column_geometry = self.column_geometry(frame.x, frame.w);
        ctx.push_clip(frame);

        // 空状态
        if self.rows.is_empty() && !self.loading {
            let loc = crate::ui::component::locale::use_locale();
            let empty = if self.empty_text.is_empty() { loc.empty_data } else { &self.empty_text };
            let horizontal_inset = 16.0_f32.min(frame.w * 0.25);
            Self::paint_single_line(
                ctx,
                empty,
                Rect::new(
                    frame.x + horizontal_inset,
                    frame.y,
                    (frame.w - horizontal_inset * 2.0).max(0.0),
                    (frame.h - self.pagination_height()).max(0.0),
                ),
                text_sec,
                14.0,
            );
            if self.bordered {
                ctx.stroke_rect(frame, border, 1.0, r);
            }
            self.paint_pagination(frame, ctx);
            ctx.pop_clip();
            return;
        }

        header::paint(self, Rect::new(frame.x, y, frame.w, frame.h), ctx, &column_geometry);
        y += self.total_header_height();

        // 分隔线
        ctx.fill_rect(Rect::new(frame.x, y, frame.w, 1.0), border, None);
        y += 1.0;

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
                ctx.tokens().color_fill_secondary()
            } else if is_selected {
                sel_bg
            } else if is_hovered {
                hover_bg
            } else if actual_ri.is_multiple_of(2) {
                bg
            } else {
                ctx.tokens().color_bg_container()
            };
            let row_rect = Rect::new(frame.x, row_y, frame.w, self.row_h);
            ctx.fill_rect(row_rect, row_bg, None);

            if self.selection {
                crate::ui::widgets::Icon::paint_in_frame(
                    ctx,
                    if is_checked { "check-square" } else { "square" },
                    Rect::new(frame.x + 6.0, row_rect.y, 18.0, row_rect.h),
                    primary,
                    14.0,
                );
                if self.bordered {
                    ctx.fill_rect(
                        Rect::new(
                            frame.x + self.selection_width() - 1.0,
                            row_rect.y,
                            1.0,
                            row_rect.h,
                        ),
                        border,
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
                    let cell_width = self.span_width(&column_geometry, laid_out.index, col_span);
                    let cell_height = self.span_height(actual_ri, row_span);
                    if !self.view_columns.contains(&laid_out.index) {
                        let cell = row
                            .get(laid_out.index)
                            .map(String::as_str)
                            .unwrap_or("");
                        let tc = if is_selected { primary } else { text_color };
                        let cell_frame = Rect::new(
                            laid_out.x,
                            row_rect.y,
                            cell_width,
                            cell_height,
                        );
                        if let Some(cell_frame) = cell_frame.intersect(&clip) {
                            let horizontal_inset = 8.0_f32.min(cell_frame.w * 0.25);
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
                                12.0,
                            );
                        }
                    }
                    if self.bordered {
                        ctx.fill_rect(
                            Rect::new(
                                laid_out.x + cell_width - 1.0,
                                row_rect.y,
                                1.0,
                                cell_height,
                            ),
                            border,
                            None,
                        );
                    }
                }
                ctx.pop_clip();
            }

            if actual_ri + 1 < end || is_expanded {
                ctx.fill_rect(Rect::new(frame.x, row_y + self.row_h, frame.w, 1.0), border, None);
            }

            // 扩展行内容
            if is_expanded {
                let expand_rect = Rect::new(
                    frame.x,
                    row_y + self.row_h,
                    frame.w,
                    self.expand_height,
                );
                ctx.fill_rect(expand_rect, ctx.tokens().color_bg_container(), None);
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
                let Some(laid_out) = column_geometry
                    .columns
                    .iter()
                    .find(|column| column.index == column_index)
                else {
                    continue;
                };
                let row_span = self.row_span(row_index, column_index);
                let col_span = self.col_span(row_index, column_index);
                let cell_width = self.span_width(&column_geometry, column_index, col_span);
                let cell_height = self.span_height(row_index, row_span);
                let expanded_offset = if expanded.is_some_and(|expanded_row| row_index > expanded_row)
                {
                    self.expand_height
                } else {
                    0.0
                };
                let row_y = body_top + row_index as f32 * self.row_h + expanded_offset
                    - self.body_scroll.scroll_offset();
                let cell_frame = Rect::new(laid_out.x, row_y, cell_width, cell_height);
                if cell_frame.intersect(&body_clip).is_none() {
                    continue;
                }
                let Some(zone_clip) =
                    column_geometry.clip_for(laid_out.zone, cell_frame.y, cell_frame.h)
                else {
                    continue;
                };
                let is_selected = sel == Some(row_index);
                let is_hovered = hover == Some(row_index);
                let is_pressed = self
                    .pressed_action
                    .get()
                    .and_then(Self::action_row)
                    == Some(row_index)
                    && is_hovered;
                let cell_bg = if is_pressed {
                    ctx.tokens().color_fill_secondary()
                } else if is_selected {
                    sel_bg
                } else if is_hovered {
                    hover_bg
                } else if row_index.is_multiple_of(2) {
                    bg
                } else {
                    ctx.tokens().color_bg_container()
                };

                ctx.push_clip(zone_clip);
                ctx.fill_rect(cell_frame, cell_bg, None);
                if !self.view_columns.contains(&column_index) {
                    let cell = row.get(column_index).map(String::as_str).unwrap_or("");
                    let horizontal_inset = 8.0_f32.min(cell_frame.w * 0.25);
                    Self::paint_single_line(
                        ctx,
                        cell,
                        Rect::new(
                            cell_frame.x + horizontal_inset,
                            cell_frame.y,
                            (cell_frame.w - horizontal_inset * 2.0).max(0.0),
                            cell_frame.h,
                        ),
                        if is_selected { primary } else { text_color },
                        12.0,
                    );
                }
                if self.bordered {
                    ctx.stroke_rect(cell_frame, border, 1.0, None);
                } else {
                    ctx.fill_rect(
                        Rect::new(
                            cell_frame.x,
                            cell_frame.y + cell_frame.h,
                            cell_frame.w,
                            1.0,
                        ),
                        border,
                        None,
                    );
                }
                ctx.pop_clip();
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
                let icon_size = 18.0_f32.min(self.row_h).min(frame.w);
                crate::ui::widgets::Icon::paint_in_frame(
                    ctx,
                    if expanded == Some(actual_ri) {
                        "chevron-up"
                    } else {
                        "chevron-down"
                    },
                    Rect::new(
                        frame.x + (frame.w - icon_size - 6.0).max(0.0),
                        row_y + (self.row_h - icon_size) * 0.5,
                        icon_size,
                        icon_size,
                    ),
                    text_sec,
                    10.0,
                );
            }
        }

        ctx.pop_clip();
        self.paint_pagination(frame, ctx);
        if self.loading {
            self.paint_loading_overlay(frame, ctx);
        }
        if self.bordered {
            ctx.stroke_rect(frame, border, 1.0, r);
        }
        if self.focused && tree.keyboard_focus_visible() {
            let inset = 1.0_f32.min(frame.w * 0.5).min(frame.h * 0.5);
            ctx.stroke_rect(
                Rect::new(
                    frame.x + inset,
                    frame.y + inset,
                    (frame.w - inset * 2.0).max(0.0),
                    (frame.h - inset * 2.0).max(0.0),
                ),
                primary,
                2.0,
                Some(Radius::uniform(
                    radius.min((frame.w - inset * 2.0).min(frame.h - inset * 2.0) * 0.5),
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
            + dt.max(0.0) as f32 * std::f32::consts::TAU / 0.8)
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
            frame.y + header_height + 1.0,
            frame.w,
            (frame.h - header_height - 1.0 - self.pagination_height()).max(0.0),
        ))
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        self.last_frame.set(Some(frame));
        if !self.view_columns.is_empty() {
            let (start, end) = self
                .materialized_cell_range
                .get()
                .unwrap_or_else(|| self.visible_row_range(self.body_viewport_height()));
            let column_geometry = self.column_geometry(frame.x, frame.w);
            let body_top = frame.y + self.total_header_height() + 1.0;
            let mut positions = Vec::with_capacity(children.len());
            for (local_index, child) in children.iter().enumerate() {
                let column_count = self.view_columns.len();
                let row = start + local_index / column_count;
                if row >= end {
                    break;
                }
                let column_index = self.view_columns[local_index % column_count];
                let Some(column) = column_geometry
                    .columns
                    .iter()
                    .find(|column| column.index == column_index)
                else {
                    continue;
                };
                let expanded_offset = if self.expanded_row.get().is_some_and(|expanded| row > expanded)
                {
                    self.expand_height
                } else {
                    0.0
                };
                // View children stay in content coordinates. The compositor and
                // hit-test path apply the viewport scroll offset, so unchanged
                // cells do not need new frames for every vertical wheel event.
                let row_y = body_top + row as f32 * self.row_h + expanded_offset;
                if self.cell_anchor(row, column_index) != Some((row, column_index)) {
                    positions.push((child.id, Rect::new(column.x, row_y, 0.0, 0.0)));
                    continue;
                }
                let row_span = self.row_span(row, column_index);
                let col_span = self.col_span(row, column_index);
                let cell_width = self.span_width(&column_geometry, column_index, col_span);
                let cell_height = self.span_height(row, row_span);
                let cell = Rect::new(column.x, row_y, cell_width, cell_height);
                positions.push((
                    child.id,
                    column_geometry
                        .clip_for(column.zone, row_y, cell_height)
                        .and_then(|zone_clip| cell.intersect(&zone_clip))
                        .unwrap_or_else(|| Rect::new(cell.x, cell.y, 0.0, 0.0)),
                ));
            }
            return positions;
        }
        let Some(expanded_row) = self.expanded_row.get() else {
            return Vec::new();
        };
        let Some(child) = children.first() else {
            return Vec::new();
        };
        let y = frame.y
            + self.total_header_height()
            + 1.0
            + (expanded_row + 1) as f32 * self.row_h;
        vec![(
            child.id,
            Rect::new(frame.x, y, frame.w, self.expand_height),
        )]
    }
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
