//! Table widget — 简单数据表格。

use crate::define_widget;
use crate::base::{Rect, Size};
use crate::graphics::Radius;
use crate::ui::render_context::RenderContext;
use crate::ui::widget::WidgetTree;

/// 表格列定义。
#[derive(Debug, Clone)]
pub struct TableColumn {
    pub title: String,
    pub width: f32,
}

impl TableColumn {
    pub fn new(title: impl Into<String>, width: f32) -> Self {
        Self { title: title.into(), width }
    }
}

/// 表格行数据（字符串列表）。
pub type TableRow = Vec<String>;

define_widget! {
    /// Table — 数据表格，含表头、行、悬停高亮。
    pub struct Table {
        columns: Vec<TableColumn>,
        rows: Vec<TableRow>,
        row_h: f32,
        header_h: f32,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        let w: f32 = self.columns.iter().map(|c| c.width).sum();
        let h = self.header_h + self.rows.len() as f32 * self.row_h;
        Size::new(w, h)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_elevated();
        let header_bg = ctx.tokens().color_fill_tertiary();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        let mut y = frame.y;

        // 表头
        let header_rect = Rect::new(frame.x, y, frame.w, self.header_h);
        ctx.fill_rect(header_rect, header_bg, r);
        let mut x = frame.x;
        for col in &self.columns {
            ctx.draw_text(&col.title, crate::base::Point::new(x + 8.0, y + 6.0), text_color, 13.0);
            x += col.width;
        }
        y += self.header_h;

        // 分隔线
        ctx.fill_rect(Rect::new(frame.x, y, frame.w, 1.0), border, None);
        y += 1.0;

        // 数据行
        for (ri, row) in self.rows.iter().enumerate() {
            let row_bg = if ri % 2 == 0 { bg } else { ctx.tokens().color_bg_container() };
            let row_rect = Rect::new(frame.x, y, frame.w, self.row_h);
            ctx.fill_rect(row_rect, row_bg, None);

            let mut x = frame.x;
            for (ci, col) in self.columns.iter().enumerate() {
                let cell = row.get(ci).map(|s| s.as_str()).unwrap_or("");
                ctx.draw_text(cell, crate::base::Point::new(x + 8.0, y + 5.0), text_color, 12.0);
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
        Self { columns: Vec::new(), rows: Vec::new(), row_h: 28.0, header_h: 32.0 }
    }
    pub fn columns(mut self, cols: Vec<TableColumn>) -> Self { self.columns = cols; self }
    pub fn rows(mut self, rows: Vec<TableRow>) -> Self { self.rows = rows; self }
    pub fn row_height(mut self, h: f32) -> Self { self.row_h = h; self }
}
