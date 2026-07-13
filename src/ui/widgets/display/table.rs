use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Radius;
use crate::ui::core::paint_scope::current_paint_widget;
use crate::ui::foundation::virtual_scroll::VirtualListScroll;
use crate::ui::render_handler::RenderHandlerRegistration;
use crate::ui::{
    ComponentId, EventResult, SemanticEvent, SnapshotFields, SnapshotTableColumn, SystemEvent,
    WidgetTree,
};
use std::cell::{Cell, RefCell};

/// 排序方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    None,
    Asc,
    Desc,
}

/// 表格列定义。
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumn {
    pub title: String,
    pub width: f32,
    pub sortable: bool,
    pub sort_direction: SortDirection,
    pub filterable: bool,
    pub filters: Vec<(String, bool)>, // (label, active)
}

impl TableColumn {
    pub fn new(title: impl Into<String>, width: f32) -> Self {
        Self {
            title: title.into(),
            width,
            sortable: false,
            sort_direction: SortDirection::None,
            filterable: false,
            filters: Vec::new(),
        }
    }
    pub fn sortable(mut self, v: bool) -> Self {
        self.sortable = v;
        self
    }
    pub fn filterable(mut self, v: bool) -> Self {
        self.filterable = v;
        self
    }
}

/// 表格行数据。
pub type TableRow = Vec<String>;

/// 扩展行渲染器。
pub type ExpandRenderer = Box<dyn Fn(usize, &mut PaintContext, Rect)>;

/// 变更事件。
#[derive(Debug, Clone)]
pub struct TableChange {
    pub sort_column: Option<usize>,
    pub sort_direction: SortDirection,
    pub page: usize,
    pub page_size: usize,
}

/// Declarative table builder that carries paint handlers outside the component.
pub struct TableBuilder {
    table: Table,
    expand_renderer: ExpandRenderer,
}

component! {
    pub struct Table {
        columns: Vec<TableColumn>,
        pub(crate) rows: Vec<TableRow>,
        pub(crate) row_h: f32,
        header_h: f32,
        selected_row: Cell<Option<usize>>,
        hover_row: Cell<Option<usize>>,
        /// 多选：选中行索引集合。
        checked_rows: Vec<usize>,
        /// 扩展行：当前展开的行索引。
        expanded_row: Cell<Option<usize>>,
        expandable: bool,
        expand_height: f32,
        /// 空状态文案。
        empty_text: String,
        /// 当前分页。
        current_page: Cell<usize>,
        page_size: usize,
        pending_change: RefCell<Option<String>>,
        pub(crate) body_scroll: VirtualListScroll,
        scroll_delta_strip: Cell<(f32, f32)>,
        pub(crate) last_frame: Cell<Option<Rect>>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        let w: f32 = self.columns.iter().map(|c| c.width).sum();
        let extra = if self.expandable && self.expanded_row.get().is_some() { self.expand_height } else { 0.0 };
        let body_h = self.rows.len() as f32 * self.row_h + extra;
        let h = self.header_h + body_h;
        constraints.clamp(Size::new(w, h.max(60.0)))
    }

    scroll_delta_for_dirty => (&self) -> Option<(f32, f32)> {
        let delta = self.scroll_delta_strip.get();
        if delta.0.abs() > 0.01 || delta.1.abs() > 0.01 {
            self.scroll_delta_strip.set((0.0, 0.0));
            Some(delta)
        } else {
            None
        }
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::Wheel { delta, .. } => {
                let viewport_h = self.body_viewport_height();
                let old = self.body_scroll.scroll_offset();
                let max = (self.body_content_height() - viewport_h).max(0.0);
                let next = (old - delta.y * 40.0).clamp(0.0, max);
                self.body_scroll.set_scroll_offset(next);
                let dy = next - old;
                if dy.abs() > 0.01 {
                    self.push_scroll_delta(0.0, dy);
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerDown { pos, .. } => {
                if pos.x < 32.0 && pos.y >= self.header_h {
                    let row = self.row_index_at_y(pos.y);
                    if let Some(row) = row {
                        let idx = self.checked_rows.iter().position(|&r| r == row);
                        if let Some(i) = idx { self.checked_rows.remove(i); }
                        else { self.checked_rows.push(row); }
                        return EventResult::Handled;
                    }
                }
                if pos.y < self.header_h {
                    let mut x = 32.0;
                    for (ci, col) in self.columns.iter().enumerate() {
                        if pos.x >= x && pos.x < x + col.width && col.sortable {
                            let new_dir = match col.sort_direction {
                                SortDirection::None => SortDirection::Asc,
                                SortDirection::Asc => SortDirection::Desc,
                                SortDirection::Desc => SortDirection::None,
                            };
                            for c in &mut self.columns { c.sort_direction = SortDirection::None; }
                            self.columns[ci].sort_direction = new_dir;
                            let change = TableChange {
                                sort_column: Some(ci),
                                sort_direction: new_dir,
                                page: self.current_page.get(),
                                page_size: self.page_size,
                            };
                            self.pending_change.replace(Some(change.payload()));
                            return EventResult::Handled;
                        }
                        x += col.width;
                    }
                }
                if pos.y >= self.header_h {
                    if let Some(row) = self.row_index_at_y(pos.y) {
                        if self.expandable && self.expand_toggle_hit(pos.x) {
                            let next = (self.expanded_row.get() != Some(row)).then_some(row);
                            self.expanded_row.set(next);
                        }
                        self.selected_row.set(Some(row));
                        return EventResult::Handled;
                    }
                }
                self.selected_row.set(None);
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if pos.y >= self.header_h {
                    let row = self.row_index_at_y(pos.y);
                    self.hover_row.set(row.filter(|&r| r < self.rows.len()));
                } else {
                    self.hover_row.set(None);
                }
                EventResult::NotHandled
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

    wants_continuous_pointer_move => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        self.last_frame.set(Some(frame));
        let expand_renderer = if self.expandable {
            current_paint_widget().and_then(|id| tree.table_expand_renderer(id))
        } else {
            None
        };
        let loc = crate::ui::locale::use_locale();
        let bg = ctx.tokens().color_bg_elevated();
        let header_bg = ctx.tokens().color_fill_tertiary();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let primary = ctx.tokens().color_primary();
        let hover_bg = ctx.tokens().color_fill_quaternary();
        let sel_bg = ctx.tokens().color_primary_bg();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        let mut y = frame.y;
        let sel = self.selected_row.get();
        let hover = self.hover_row.get();
        let expanded = self.expanded_row.get();

        // 空状态
        if self.rows.is_empty() {
            let loc = crate::ui::locale::use_locale();
            let empty = if self.empty_text.is_empty() { loc.empty_data } else { &self.empty_text };
            let ey = ctx.visual_center_y(frame, 14.0);
            ctx.draw_text(empty, Point::new(frame.x + 16.0, ey), text_sec, 14.0);
            return;
        }

        // 表头
        let header_rect = Rect::new(frame.x, y, frame.w, self.header_h);
        ctx.fill_rect(header_rect, header_bg, r);
        let mut x = frame.x + 32.0;
        let hdr_y = ctx.visual_center_y(header_rect, 13.0);
        // 全选复选框
        let all_checked = !self.rows.is_empty() && self.checked_rows.len() == self.rows.len();
        ctx.draw_text(if all_checked { "☑" } else { "☐" }, Point::new(frame.x + 8.0, hdr_y), text_sec, 14.0);
        for col in &self.columns {
            ctx.draw_text(&col.title, Point::new(x + 8.0, hdr_y), text_color, 13.0);
            if col.sortable {
                let indicator = match col.sort_direction {
                    SortDirection::Asc => loc.table_sort_asc,
                    SortDirection::Desc => loc.table_sort_desc,
                    SortDirection::None => "",
                };
                if !indicator.is_empty() {
                    ctx.draw_text(indicator, Point::new(x + col.width - 24.0, hdr_y), primary, 11.0);
                } else {
                    ctx.draw_text(loc.table_sort_unsorted, Point::new(x + col.width - 24.0, hdr_y), text_sec, 10.0);
                }
            }
            x += col.width;
        }
        y += self.header_h;

        // 分隔线
        ctx.fill_rect(Rect::new(frame.x, y, frame.w, 1.0), border, None);
        y += 1.0;

        // 数据行（虚拟滚动：仅绘制 viewport ± overscan）
        let body_top = y;
        let body_viewport_h = self.body_viewport_height();
        let (start, end) = self.visible_row_range(body_viewport_h);
        let body_clip = Rect::new(frame.x, body_top, frame.w, body_viewport_h);
        ctx.canvas_2d().push_clip(body_clip);

        for actual_ri in start..end {
            let Some(row) = self.rows.get(actual_ri) else {
                continue;
            };
            let is_selected = sel == Some(actual_ri);
            let is_hovered = hover == Some(actual_ri);
            let is_checked = self.checked_rows.contains(&actual_ri);
            let is_expanded = expanded == Some(actual_ri);

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

            let row_bg = if is_selected { sel_bg } else if is_hovered { hover_bg } else if actual_ri.is_multiple_of(2) { bg } else { ctx.tokens().color_bg_container() };
            let row_rect = Rect::new(frame.x, row_y, frame.w, self.row_h);
            ctx.fill_rect(row_rect, row_bg, None);

            // 复选框
            let check_y = ctx.visual_center_y(row_rect, 14.0);
            ctx.draw_text(if is_checked { "☑" } else { "☐" }, Point::new(frame.x + 8.0, check_y), primary, 14.0);

            let mut x = frame.x + 32.0;
            let cell_y = ctx.visual_center_y(row_rect, 12.0);
            for (ci, col) in self.columns.iter().enumerate() {
                let cell = row.get(ci).map(|s| s.as_str()).unwrap_or("");
                let tc = if is_selected { primary } else { text_color };
                ctx.draw_text(cell, Point::new(x + 8.0, cell_y), tc, 12.0);
                x += col.width;
            }

            // 扩展行箭头
            if expand_renderer.is_some() {
                ctx.draw_text(if is_expanded { "▲" } else { "▼" }, Point::new(frame.x + frame.w - 20.0, cell_y), text_sec, 10.0);
            }

            if actual_ri + 1 < end || is_expanded {
                ctx.fill_rect(Rect::new(frame.x, row_y + self.row_h, frame.w, 1.0), border, None);
            }

            // 扩展行内容
            if is_expanded {
                if let Some(renderer) = expand_renderer {
                    let expand_rect = Rect::new(frame.x, row_y + self.row_h, frame.w, self.expand_height);
                    ctx.fill_rect(expand_rect, ctx.tokens().color_bg_container(), None);
                    renderer(actual_ri, ctx, expand_rect);
                }
            }
        }

        ctx.canvas_2d().pop_clip();
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

impl Table {
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            row_h: 28.0,
            header_h: 32.0,
            selected_row: Cell::new(None),
            hover_row: Cell::new(None),
            checked_rows: Vec::new(),
            expanded_row: Cell::new(None),
            expandable: false,
            expand_height: 60.0,
            empty_text: String::new(),
            current_page: Cell::new(0),
            page_size: 20,
            pending_change: RefCell::new(None),
            body_scroll: VirtualListScroll::new(),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
            last_frame: Cell::new(None),
        }
    }
    pub fn columns(mut self, cols: Vec<TableColumn>) -> Self {
        self.columns = cols;
        self
    }
    pub fn rows(mut self, rows: Vec<TableRow>) -> Self {
        self.rows = rows;
        self
    }
    pub fn row_height(mut self, h: f32) -> Self {
        self.row_h = h;
        self
    }
    pub fn selected_row(&self) -> Option<usize> {
        self.selected_row.get()
    }
    pub fn set_selected_row(&self, row: Option<usize>) {
        self.selected_row.set(row);
    }
    pub fn expanded_row(&self) -> Option<usize> {
        self.expanded_row.get()
    }
    pub fn checked_rows(&self) -> &[usize] {
        &self.checked_rows
    }
    pub fn empty_text(mut self, t: impl Into<String>) -> Self {
        self.empty_text = t.into();
        self
    }
    pub fn expandable(
        mut self,
        height: f32,
        renderer: impl Fn(usize, &mut PaintContext, Rect) + 'static,
    ) -> TableBuilder {
        self.expandable = true;
        self.expand_height = height.max(0.0);
        TableBuilder {
            table: self,
            expand_renderer: Box::new(renderer),
        }
    }
    pub fn page_size(mut self, n: usize) -> Self {
        self.page_size = n;
        self
    }

    pub(crate) fn body_viewport_height(&self) -> f32 {
        self.last_frame
            .get()
            .map(|f| (f.h - self.header_h - 1.0).max(self.row_h))
            .unwrap_or(300.0)
    }

    fn row_index_at_y(&self, pos_y: f32) -> Option<usize> {
        if pos_y < self.header_h {
            return None;
        }
        let mut local_y = pos_y - self.header_h + self.body_scroll.scroll_offset();
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

    fn expand_toggle_hit(&self, pos_x: f32) -> bool {
        let width = self
            .last_frame
            .get()
            .map(|frame| frame.w)
            .unwrap_or_else(|| self.columns.iter().map(|column| column.width).sum());
        pos_x >= (width - 32.0).max(0.0)
    }

    fn body_content_height(&self) -> f32 {
        self.rows.len() as f32 * self.row_h
            + if self.expandable && self.expanded_row.get().is_some() {
                self.expand_height
            } else {
                0.0
            }
    }

    fn visible_row_range(&self, viewport_height: f32) -> (usize, usize) {
        let mut logical_offset = self.body_scroll.scroll_offset();
        if self.expandable {
            if let Some(expanded) = self.expanded_row.get() {
                let expand_start = (expanded + 1) as f32 * self.row_h;
                if logical_offset > expand_start {
                    logical_offset = (logical_offset - self.expand_height).max(expand_start);
                }
            }
        }
        crate::ui::foundation::virtual_scroll::virtual_list_index_range(
            self.rows.len(),
            self.row_h,
            logical_offset,
            viewport_height,
            2,
        )
    }

    fn push_scroll_delta(&self, dx: f32, dy: f32) {
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
            rows: self.rows.clone(),
            row_h: self.row_h,
            header_h: self.header_h,
            expandable: self.expandable,
            expand_height: self.expand_height,
            empty_text: self.empty_text.clone(),
            page_size: self.page_size,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.columns = merge_table_columns(self.columns.as_slice(), next.columns);
        self.rows = next.rows;
        self.row_h = next.row_h;
        self.header_h = next.header_h;
        self.expandable = next.expandable;
        self.expand_height = next.expand_height;
        self.empty_text = next.empty_text;
        self.page_size = next.page_size;
        let max = (self.body_content_height() - self.body_viewport_height()).max(0.0);
        self.body_scroll
            .set_scroll_offset(self.body_scroll.scroll_offset().min(max));
    }
}

impl TableBuilder {
    fn into_parts(self) -> (Table, RenderHandlerRegistration) {
        (
            self.table,
            RenderHandlerRegistration::TableExpand(self.expand_renderer),
        )
    }

    pub fn columns(mut self, columns: Vec<TableColumn>) -> Self {
        self.table.columns = columns;
        self
    }

    pub fn rows(mut self, rows: Vec<TableRow>) -> Self {
        self.table.rows = rows;
        self
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.table.row_h = height;
        self
    }

    pub fn empty_text(mut self, text: impl Into<String>) -> Self {
        self.table.empty_text = text.into();
        self
    }

    pub fn page_size(mut self, size: usize) -> Self {
        self.table.page_size = size;
        self
    }
}

impl crate::ui::IntoWidgetNode for TableBuilder {
    fn into_node(self) -> crate::ui::core::widget::WidgetNode {
        let (table, handler) = self.into_parts();
        crate::ui::core::widget::WidgetNode::leaf(Box::new(table))
            .with_render_handlers(vec![handler])
    }
}

impl crate::ui::view::View for TableBuilder {
    fn build(self) -> crate::ui::view::ViewNode {
        let (table, handler) = self.into_parts();
        let mut node = crate::ui::view::ViewNode::leaf(table);
        node.render_handlers.push(handler);
        node
    }
}

impl From<TableBuilder> for crate::ui::view::ViewNode {
    fn from(builder: TableBuilder) -> Self {
        crate::ui::view::View::build(builder)
    }
}

fn merge_table_columns(current: &[TableColumn], next: Vec<TableColumn>) -> Vec<TableColumn> {
    next.into_iter()
        .enumerate()
        .map(|(idx, mut next_col)| {
            if let Some(current_col) = current.get(idx) {
                if next_col.sortable {
                    next_col.sort_direction = current_col.sort_direction;
                }
                if next_col.filterable {
                    for (label, active) in &mut next_col.filters {
                        if let Some((_, current_active)) = current_col
                            .filters
                            .iter()
                            .find(|(current_label, _)| current_label == label)
                        {
                            *active = *current_active;
                        }
                    }
                }
            }
            next_col
        })
        .collect()
}
