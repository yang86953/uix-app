//! Table widget — 数据表格，支持表头、行选中、悬停高亮、列排序。

use crate::define_widget;
use uix_core::{Point, Rect, Size};
use uix_graphics::{Color, Radius};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};
use std::cell::Cell;

/// 排序方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    None,
    Asc,
    Desc,
}

/// 表格列定义。
#[derive(Debug, Clone)]
pub struct TableColumn {
    pub title: String,
    pub width: f32,
    pub sortable: bool,
    pub sort_direction: SortDirection,
}

impl TableColumn {
    pub fn new(title: impl Into<String>, width: f32) -> Self {
        Self { title: title.into(), width, sortable: false, sort_direction: SortDirection::None }
    }
    pub fn sortable(mut self, v: bool) -> Self { self.sortable = v; self }
}

/// 表格行数据（字符串列表）。
pub type TableRow = Vec<String>;

define_widget! {
    /// Table — 数据表格，含表头、行选中、悬停高亮、列排序。
    pub struct Table {
        columns: Vec<TableColumn>,
        rows: Vec<TableRow>,
        row_h: f32,
        header_h: f32,
        selected_row: Cell<Option<usize>>,
        hover_row: Cell<Option<usize>>,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        let w: f32 = self.columns.iter().map(|c| c.width).sum();
        let h = self.header_h + self.rows.len() as f32 * self.row_h;
        Size::new(w, h)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                if pos.y >= self.header_h {
                    let row = ((pos.y - self.header_h) / self.row_h) as usize;
                    if row < self.rows.len() {
                        self.selected_row.set(Some(row));
                        return EventResult::Handled;
                    }
                }
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

        // 表头
        let header_rect = Rect::new(frame.x, y, frame.w, self.header_h);
        ctx.fill_rect(header_rect, header_bg, r);
        let mut x = frame.x;
        let hdr_y = ctx.visual_center_y(header_rect, 13.0);
        for col in &self.columns {
            ctx.draw_text(&col.title, Point::new(x + 8.0, hdr_y), text_color, 13.0);
            // 排序指示器
            if col.sortable {
                let indicator = match col.sort_direction {
                    SortDirection::Asc => " ▲",
                    SortDirection::Desc => " ▼",
                    SortDirection::None => "",
                };
                if !indicator.is_empty() {
                    ctx.draw_text(indicator, Point::new(x + 8.0 + col.width - 24.0, hdr_y), primary, 11.0);
                }
            }
            x += col.width;
        }
        y += self.header_h;

        // 分隔线
        ctx.fill_rect(Rect::new(frame.x, y, frame.w, 1.0), border, None);
        y += 1.0;

        // 数据行
        for (ri, row) in self.rows.iter().enumerate() {
            let is_selected = sel == Some(ri);
            let is_hovered = hover == Some(ri);
            let row_bg = if is_selected { sel_bg } else if is_hovered { hover_bg } else if ri % 2 == 0 { bg } else { ctx.tokens().color_bg_container() };
            let row_rect = Rect::new(frame.x, y, frame.w, self.row_h);
            ctx.fill_rect(row_rect, row_bg, None);

            let mut x = frame.x;
            let cell_y = ctx.visual_center_y(row_rect, 12.0);
            for (ci, col) in self.columns.iter().enumerate() {
                let cell = row.get(ci).map(|s| s.as_str()).unwrap_or("");
                let tc = if is_selected { primary } else { text_color };
                ctx.draw_text(cell, Point::new(x + 8.0, cell_y), tc, 12.0);
                x += col.width;
            }
            // 行分隔线
            if ri < self.rows.len() - 1 {
                ctx.fill_rect(Rect::new(frame.x, y + self.row_h, frame.w, 1.0), border, None);
            }
            y += self.row_h;
        }
    }
}

impl Default for Table { fn default() -> Self { Self::new() } }

impl Table {
    pub fn new() -> Self {
        Self {
            columns: Vec::new(), rows: Vec::new(), row_h: 28.0, header_h: 32.0,
            selected_row: Cell::new(None), hover_row: Cell::new(None),
        }
    }
    pub fn columns(mut self, cols: Vec<TableColumn>) -> Self { self.columns = cols; self }
    pub fn rows(mut self, rows: Vec<TableRow>) -> Self { self.rows = rows; self }
    pub fn row_height(mut self, h: f32) -> Self { self.row_h = h; self }
    pub fn selected_row(&self) -> Option<usize> { self.selected_row.get() }
    pub fn set_selected_row(&self, row: Option<usize>) { self.selected_row.set(row); }
}
