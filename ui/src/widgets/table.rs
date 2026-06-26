use crate::define_widget;
use uix_core::{Point, Rect, Size};
use uix_graphics::Radius;
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};
use std::cell::Cell;

/// 排序方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection { None, Asc, Desc }

/// 表格列定义。
#[derive(Debug, Clone)]
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
        Self { title: title.into(), width, sortable: false, sort_direction: SortDirection::None, filterable: false, filters: Vec::new() }
    }
    pub fn sortable(mut self, v: bool) -> Self { self.sortable = v; self }
    pub fn filterable(mut self, v: bool) -> Self { self.filterable = v; self }
}

/// 表格行数据。
pub type TableRow = Vec<String>;

/// 扩展行渲染器。
pub type ExpandRenderer = Box<dyn Fn(usize, &mut RenderContext, Rect)>;

/// 数据变更回调。
pub type TableChangeCallback = Box<dyn FnMut(TableChange)>;

/// 变更事件。
#[derive(Debug, Clone)]
pub struct TableChange {
    pub sort_column: Option<usize>,
    pub sort_direction: SortDirection,
    pub page: usize,
    pub page_size: usize,
}

define_widget! {
    pub struct Table {
        columns: Vec<TableColumn>,
        rows: Vec<TableRow>,
        row_h: f32,
        header_h: f32,
        selected_row: Cell<Option<usize>>,
        hover_row: Cell<Option<usize>>,
        /// 多选：选中行索引集合。
        checked_rows: Vec<usize>,
        /// 扩展行：当前展开的行索引。
        expanded_row: Cell<Option<usize>>,
        expand_height: f32,
        expand_renderer: Option<ExpandRenderer>,
        /// 空状态文案。
        empty_text: String,
        /// 当前分页。
        current_page: Cell<usize>,
        page_size: usize,
        on_change: Option<TableChangeCallback>,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        let w: f32 = self.columns.iter().map(|c| c.width).sum();
        let data_rows = self.rows.len().min(self.page_size);
        let extra = if self.expanded_row.get().is_some() { self.expand_height } else { 0.0 };
        let h = self.header_h + data_rows as f32 * self.row_h + extra;
        Size::new(w, h.max(60.0))
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                if pos.x < 32.0 && pos.y >= self.header_h {
                    let row = ((pos.y - self.header_h) / self.row_h) as usize;
                    if row < self.rows.len() {
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
                            if let Some(ref mut cb) = self.on_change {
                                cb(TableChange {
                                    sort_column: Some(ci),
                                    sort_direction: new_dir,
                                    page: self.current_page.get(),
                                    page_size: self.page_size,
                                });
                            }
                            return EventResult::Handled;
                        }
                        x += col.width;
                    }
                }
                if pos.y >= self.header_h {
                    let row = ((pos.y - self.header_h) / self.row_h) as usize;
                    if row < self.rows.len() {
                        self.selected_row.set(Some(row));
                        return EventResult::Handled;
                    }
                }
                self.selected_row.set(None);
                EventResult::NotHandled
            }
            WidgetEvent::MouseMove { pos } => {
                if pos.y >= self.header_h {
                    let row = ((pos.y - self.header_h) / self.row_h) as usize;
                    self.hover_row.set(if row < self.rows.len() { Some(row) } else { None });
                } else {
                    self.hover_row.set(None);
                }
                EventResult::NotHandled
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let loc = crate::locale::use_locale();
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
            let loc = crate::locale::use_locale();
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

        // 数据行（分页截取）
        let start = self.current_page.get() * self.page_size;
        let visible_rows: Vec<(usize, &TableRow)> = self.rows.iter().enumerate().skip(start).take(self.page_size).collect();

        for (vi, row) in visible_rows.iter().enumerate() {
            let actual_ri = start + vi;
            let is_selected = sel == Some(actual_ri);
            let is_hovered = hover == Some(actual_ri);
            let is_checked = self.checked_rows.contains(&actual_ri);
            let is_expanded = expanded == Some(actual_ri);

            let row_bg = if is_selected { sel_bg } else if is_hovered { hover_bg } else if actual_ri % 2 == 0 { bg } else { ctx.tokens().color_bg_container() };
            let row_rect = Rect::new(frame.x, y, frame.w, self.row_h);
            ctx.fill_rect(row_rect, row_bg, None);

            // 复选框
            let check_y = ctx.visual_center_y(row_rect, 14.0);
            ctx.draw_text(if is_checked { "☑" } else { "☐" }, Point::new(frame.x + 8.0, check_y), primary, 14.0);

            let mut x = frame.x + 32.0;
            let cell_y = ctx.visual_center_y(row_rect, 12.0);
            for (ci, col) in self.columns.iter().enumerate() {
                let cell = row.1.get(ci).map(|s| s.as_str()).unwrap_or("");
                let tc = if is_selected { primary } else { text_color };
                ctx.draw_text(cell, Point::new(x + 8.0, cell_y), tc, 12.0);
                x += col.width;
            }

            // 扩展行箭头
            if self.expand_renderer.is_some() {
                ctx.draw_text(if is_expanded { "▲" } else { "▼" }, Point::new(frame.x + frame.w - 20.0, cell_y), text_sec, 10.0);
            }

            if vi < visible_rows.len().saturating_sub(1) || is_expanded {
                ctx.fill_rect(Rect::new(frame.x, y + self.row_h, frame.w, 1.0), border, None);
            }
            y += self.row_h;

            // 扩展行内容
            if is_expanded {
                if let Some(ref renderer) = self.expand_renderer {
                    let expand_rect = Rect::new(frame.x, y, frame.w, self.expand_height);
                    ctx.fill_rect(expand_rect, ctx.tokens().color_bg_container(), None);
                    renderer(actual_ri, ctx, expand_rect);
                }
                y += self.expand_height;
            }
        }
    }
}

impl Default for Table { fn default() -> Self { Self::new() } }

impl Table {
    pub fn new() -> Self {
        Self {
            columns: Vec::new(), rows: Vec::new(), row_h: 28.0, header_h: 32.0,
            selected_row: Cell::new(None), hover_row: Cell::new(None),
            checked_rows: Vec::new(), expanded_row: Cell::new(None), expand_height: 60.0,
            expand_renderer: None, empty_text: String::new(),
            current_page: Cell::new(0), page_size: 20, on_change: None,
        }
    }
    pub fn columns(mut self, cols: Vec<TableColumn>) -> Self { self.columns = cols; self }
    pub fn rows(mut self, rows: Vec<TableRow>) -> Self { self.rows = rows; self }
    pub fn row_height(mut self, h: f32) -> Self { self.row_h = h; self }
    pub fn selected_row(&self) -> Option<usize> { self.selected_row.get() }
    pub fn set_selected_row(&self, row: Option<usize>) { self.selected_row.set(row); }
    pub fn checked_rows(&self) -> &[usize] { &self.checked_rows }
    pub fn empty_text(mut self, t: impl Into<String>) -> Self { self.empty_text = t.into(); self }
    pub fn expandable(mut self, height: f32, renderer: impl Fn(usize, &mut RenderContext, Rect) + 'static) -> Self {
        self.expand_height = height;
        self.expand_renderer = Some(Box::new(renderer));
        self
    }
    pub fn page_size(mut self, n: usize) -> Self { self.page_size = n; self }
    pub fn on_change<F: FnMut(TableChange) + 'static>(mut self, f: F) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }
}
