use crate::core::{Point, Rect, Size};
use crate::define_widget;
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::{
    EventResult, SemanticEvent, SnapshotFields, SnapshotTransferItem, SystemEvent, WidgetId,
    WidgetTree,
};
use std::cell::RefCell;

// ════════════════════════════════════════════════════════════════════════════
// QRCode
// ════════════════════════════════════════════════════════════════════════════

define_widget! {
    pub struct QRCode {
        value: String,
        size: f32,
        error_level: u8,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
        Size::new(self.size, self.size)
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let loc = crate::ui::locale::use_locale();
        let bg = Color::white();
        let fg = Color::black();
        let r = Some(Radius::uniform(ctx.tokens().border_radius()));
        ctx.fill_rect(frame, bg, r);

        // 根据 value 生成确定性伪随机 QR 矩阵（非真实编码）
        let cells = if self.error_level > 0 { 25 } else { 21 };
        let cell_s = frame.w / cells as f32;
        let seed: u64 = self.value.bytes().fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));

        for y in 0..cells {
            for x in 0..cells {
                let is_finder = (x >= cells - 7 || x < 7) && y < 7 || (x < 7 && y >= cells - 7);
                let is_filled = if is_finder {
                    let in_pattern = (x <= 1 || x >= 5) && (y <= 1 || y >= 5);
                    let is_center = (2..=4).contains(&x) && (2..=4).contains(&y);
                    (in_pattern && !is_center) || (is_center && !in_pattern)
                } else if (x >= cells - 8 || x <= 7) && y == 6 {
                    // 时序模式
                    x % 2 == 0
                } else if x == 6 && (y >= cells - 8 || y <= 7) {
                    y % 2 == 0
                } else {
                    // 数据区域：基于 seed 的确定性随机
                    let idx = (y * cells + x) as u64;
                    let hash = seed.wrapping_mul(idx + 1).wrapping_add(idx.wrapping_mul(idx + 3));
                    !hash.is_multiple_of(3)
                };
                if is_filled {
                    ctx.fill_rect(Rect::new(frame.x + x as f32 * cell_s, frame.y + y as f32 * cell_s, cell_s, cell_s), fg, None);
                }
            }
        }
        // 中心 UIX 标记
        ctx.fill_rect(Rect::new(frame.x + frame.w * 0.38, frame.y + frame.h * 0.38, frame.w * 0.24, frame.h * 0.24), bg, Some(Radius::uniform(4.0)));
        ctx.draw_text(loc.qrcode_logo, Point::new(frame.x + frame.w * 0.42, frame.y + frame.h * 0.44), fg, 11.0);
    }
}
impl QRCode {
    pub fn new(value: &str) -> Self {
        Self {
            value: value.to_string(),
            size: 160.0,
            error_level: 1,
        }
    }
    pub fn size(mut self, s: f32) -> Self {
        self.size = s;
        self
    }
    pub fn error_level(mut self, lv: u8) -> Self {
        self.error_level = lv;
        self
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::QRCode {
            value: self.value.clone(),
            size: self.size,
            error_level: self.error_level,
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Transfer
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct TransferItem {
    pub key: String,
    pub title: String,
    pub selected: bool,
}

define_widget! {
    pub struct Transfer {
        source: Vec<TransferItem>,
        target: Vec<TransferItem>,
        initial_source: Vec<TransferItem>,
        initial_target: Vec<TransferItem>,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
        Size::new(500.0, 200.0)
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if let SystemEvent::PointerDown { pos, .. } = event {
            let half = 220.0;
            let item_h = 28.0;
            if pos.x < half {
                let idx = (pos.y / item_h) as usize;
                if idx < self.source.len() { self.source[idx].selected = !self.source[idx].selected; }
            } else if pos.x > half + 60.0 {
                let idx = (pos.y / item_h) as usize;
                if idx < self.target.len() { self.target[idx].selected = !self.target[idx].selected; }
            } else {
                if pos.y >= 80.0 && pos.y < 100.0 {
                    let mut i = 0;
                    while i < self.source.len() {
                        if self.source[i].selected {
                            let mut item = self.source.remove(i);
                            item.selected = false;
                            self.target.push(item);
                        } else { i += 1; }
                    }
                    return EventResult::Handled;
                }
                if pos.y >= 100.0 && pos.y < 120.0 {
                    let mut i = 0;
                    while i < self.target.len() {
                        if self.target[i].selected {
                            let mut item = self.target.remove(i);
                            item.selected = false;
                            self.source.push(item);
                        } else { i += 1; }
                    }
                    return EventResult::Handled;
                }
            }
        }
        EventResult::NotHandled
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_quaternary();
        let primary = ctx.tokens().color_primary();
        let fill = ctx.tokens().color_fill_tertiary();
        let half = 220.0;
        let item_h = 28.0;
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        let left_rect = Rect::new(frame.x, frame.y, half, frame.h);
        ctx.fill_rect(left_rect, bg, r);
        ctx.stroke_rect(left_rect, border, 1.0, r);
        let loc = crate::ui::locale::use_locale();
        ctx.draw_text(&format!("{} ({}项)", loc.transfer_source, self.source.len()), Point::new(frame.x + 8.0, frame.y + 6.0), text_sec, 12.0);
        for (i, item) in self.source.iter().enumerate() {
            let y = frame.y + 24.0 + i as f32 * item_h;
            let row_rect = Rect::new(frame.x, y, half, item_h);
            let row_y = ctx.visual_center_y(row_rect, 13.0);
            if item.selected { ctx.fill_rect(row_rect, fill, None); }
            ctx.draw_text(if item.selected { "☑" } else { "☐" }, Point::new(frame.x + 8.0, row_y), text, 12.0);
            ctx.draw_text(&item.title, Point::new(frame.x + 26.0, row_y), text, 13.0);
        }
        let btn_y = frame.y + frame.h * 0.5 - 20.0;
        let rbtn_rect = Rect::new(frame.x + half + 8.0, btn_y, 44.0, 20.0);
        let lbtn_rect = Rect::new(frame.x + half + 8.0, btn_y + 24.0, 44.0, 20.0);
        ctx.fill_rect(rbtn_rect, primary, Some(Radius::uniform(3.0)));
        ctx.text_center("→", rbtn_rect, Color::white(), 14.0);
        ctx.fill_rect(lbtn_rect, border, Some(Radius::uniform(3.0)));
        ctx.text_center("←", lbtn_rect, text, 14.0);
        let right_x = frame.x + half + 60.0;
        let right_rect = Rect::new(right_x, frame.y, half, frame.h);
        ctx.fill_rect(right_rect, bg, r);
        ctx.stroke_rect(right_rect, border, 1.0, r);
        ctx.draw_text(&format!("{} ({}项)", loc.transfer_target, self.target.len()), Point::new(right_x + 8.0, frame.y + 6.0), text_sec, 12.0);
        for (i, item) in self.target.iter().enumerate() {
            let y = frame.y + 24.0 + i as f32 * item_h;
            let row_rect = Rect::new(right_x, y, half, item_h);
            let row_y = ctx.visual_center_y(row_rect, 13.0);
            if item.selected { ctx.fill_rect(row_rect, fill, None); }
            ctx.draw_text(if item.selected { "☑" } else { "☐" }, Point::new(right_x + 8.0, row_y), text, 12.0);
            ctx.draw_text(&item.title, Point::new(right_x + 26.0, row_y), text, 13.0);
        }
    }
}
impl Transfer {
    pub fn new() -> Self {
        Self {
            source: Vec::new(),
            target: Vec::new(),
            initial_source: Vec::new(),
            initial_target: Vec::new(),
        }
    }
    pub fn source(mut self, items: Vec<TransferItem>) -> Self {
        self.initial_source = items.clone();
        self.source = items;
        self
    }
    pub fn target(mut self, items: Vec<TransferItem>) -> Self {
        self.initial_target = items.clone();
        self.target = items;
        self
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Transfer {
            source: self
                .initial_source
                .iter()
                .map(SnapshotTransferItem::from_transfer_item)
                .collect(),
            target: self
                .initial_target
                .iter()
                .map(SnapshotTransferItem::from_transfer_item)
                .collect(),
        }
    }
}
impl Default for Transfer {
    fn default() -> Self {
        Self::new()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Upload
// ════════════════════════════════════════════════════════════════════════════

/// 上传文件项。
#[derive(Debug, Clone)]
pub struct UploadFile {
    pub name: String,
    pub size: u64,
    pub progress: f32,
    pub status: UploadStatus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UploadStatus {
    Pending,
    Uploading,
    Done,
    Error,
}
define_widget! {
    pub struct Upload {
        accept: String,
        multiple: bool,
        file_list: Vec<UploadFile>,
        drag: bool,
        drag_hover: bool,
        max_count: usize,
        pending_change: RefCell<Option<String>>,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
        let list_h = self.file_list.len() as f32 * 32.0;
        Size::new(300.0, 100.0 + list_h)
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown { .. } => {
                let file_name = format!("upload_{}.txt", self.file_list.len() + 1);
                self.add_file(&file_name);
                self.pending_change
                    .replace(Some(format!("{}:pending", file_name)));
                EventResult::Handled
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

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border();
        let text_sec = ctx.tokens().color_text_quaternary();
        let text = ctx.tokens().color_text();
        let primary = ctx.tokens().color_primary();
        let error = ctx.tokens().color_error();
        let success = ctx.tokens().color_success();
        let r = Some(Radius::uniform(ctx.tokens().border_radius()));
        let upload_rect = Rect::new(frame.x, frame.y, frame.w, 100.0);
        ctx.fill_rect(upload_rect, bg, r);

        let drag_border = if self.drag_hover { primary } else { border };
        ctx.stroke_rect(upload_rect, drag_border, if self.drag && self.drag_hover { 2.0 } else { 1.0 }, r);
        if self.drag && self.drag_hover {
            ctx.stroke_rect(Rect::new(frame.x + 4.0, frame.y + 4.0, frame.w - 8.0, 92.0), primary, 1.0, Some(Radius::uniform(ctx.tokens().border_radius_sm())));
        }
        ctx.draw_text("📁", Point::new(frame.x + frame.w * 0.5 - 12.0, frame.y + 24.0), text_sec, 24.0);
        let loc = crate::ui::locale::use_locale();
        ctx.draw_text(loc.upload_drag, Point::new(frame.x + frame.w * 0.5 - 48.0, frame.y + 60.0), text_sec, 13.0);
        if !self.accept.is_empty() && self.accept != "*" {
            let suffix = format!("{}: {}", loc.filter_title, self.accept);
            ctx.draw_text(&suffix, Point::new(frame.x + frame.w * 0.5 - 36.0, frame.y + 78.0), text_sec, 10.0);
        }

        for (i, f) in self.file_list.iter().enumerate() {
            let y = frame.y + 104.0 + i as f32 * 32.0;
            let status_color = match f.status {
                UploadStatus::Error => error,
                UploadStatus::Done => success,
                UploadStatus::Uploading => primary,
                UploadStatus::Pending => text_sec,
            };
            ctx.draw_text("📄", Point::new(frame.x + 8.0, y + 4.0), text_sec, 14.0);
            ctx.draw_text(&f.name, Point::new(frame.x + 28.0, y + 5.0), text, 12.0);
            if f.status == UploadStatus::Uploading {
                let bar_w = frame.w - 40.0;
                let bar_rect = Rect::new(frame.x + 10.0, y + 20.0, bar_w * f.progress, 4.0);
                ctx.fill_rect(bar_rect, primary, None);
            }
            let status_str: &str = match f.status {
                UploadStatus::Done => "✓",
                UploadStatus::Error => "✗",
                UploadStatus::Pending => "⏳",
                UploadStatus::Uploading => "↻",
            };
            ctx.draw_text(status_str, Point::new(frame.x + frame.w - 20.0, y + 5.0), status_color, 12.0);
        }
    }
}
impl Upload {
    pub fn new() -> Self {
        Self {
            accept: "*".into(),
            multiple: false,
            file_list: Vec::new(),
            drag: true,
            drag_hover: false,
            max_count: 10,
            pending_change: RefCell::new(None),
        }
    }
    pub fn accept(mut self, a: &str) -> Self {
        self.accept = a.to_string();
        self
    }
    pub fn multiple(mut self, v: bool) -> Self {
        self.multiple = v;
        self
    }
    pub fn drag(mut self, v: bool) -> Self {
        self.drag = v;
        self
    }
    pub fn max_count(mut self, n: usize) -> Self {
        self.max_count = n;
        self
    }
    pub fn add_file(&mut self, name: &str) {
        if self.file_list.len() >= self.max_count {
            return;
        }
        self.file_list.push(UploadFile {
            name: name.to_string(),
            size: 0,
            progress: 0.0,
            status: UploadStatus::Pending,
        });
    }
    pub fn update_progress(&mut self, idx: usize, progress: f32) {
        if idx < self.file_list.len() {
            self.file_list[idx].progress = progress;
            self.file_list[idx].status = UploadStatus::Uploading;
        }
    }
    pub fn complete_file(&mut self, idx: usize, success: bool) {
        if idx < self.file_list.len() {
            self.file_list[idx].status = if success {
                UploadStatus::Done
            } else {
                UploadStatus::Error
            };
        }
    }
    pub fn file_count(&self) -> usize {
        self.file_list.len()
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Upload {
            accept: self.accept.clone(),
            multiple: self.multiple,
            drag: self.drag,
            max_count: self.max_count,
        }
    }
}
impl Default for Upload {
    fn default() -> Self {
        Self::new()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Watermark
// ════════════════════════════════════════════════════════════════════════════

define_widget! {
    pub struct Watermark {
        text: String,
        color: Color,
        font_size: f32,
        opacity: f32,
        rotate: f32,
        gap_x: f32,
        gap_y: f32,
        x_offset: f32,
        y_offset: f32,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
        Size::zero()
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if self.text.is_empty() { return; }
        let mut c = self.color;
        c.a = (self.opacity * 255.0) as u8;
        let step_x = self.gap_x;
        let step_y = self.gap_y;
        let angle_rad = self.rotate * std::f32::consts::PI / 180.0;
        let cos_a = angle_rad.cos();
        let sin_a = angle_rad.sin();

        let fw = frame.w as i32;
        let fh = frame.h as i32;
        let sx = step_x as i32;
        let sy = step_y as i32;

        for gy in 0..(fh / sy.max(1) + 2) {
            for gx in 0..(fw / sx.max(1) + 2) {
                let base_x = gx as f32 * step_x + self.x_offset;
                let base_y = gy as f32 * step_y + self.y_offset;
                // 旋转偏移
                let half = self.text.len() as f32 * self.font_size * 0.3;
                let rx = (base_x - half) * cos_a - (base_y - half) * sin_a + half;
                let ry = (base_x - half) * sin_a + (base_y - half) * cos_a + half;
                ctx.draw_text(&self.text, Point::new(rx, ry), c, self.font_size);
            }
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        // Watermark 通过 render 在全帧范围绘制水印，
        // dirty_rect 返回实际 frame 区域（通常是全屏），
        // 禁止返回硬编码巨型区域（原 -10000~20000）避免脏区域爆炸。
        frame
    }
}
impl Watermark {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            color: Color::from_rgba(0, 0, 0, 255),
            font_size: 14.0,
            opacity: 0.15,
            rotate: -22.0,
            gap_x: 200.0,
            gap_y: 160.0,
            x_offset: 0.0,
            y_offset: 0.0,
        }
    }
    pub fn color(mut self, c: Color) -> Self {
        self.color = c;
        self
    }
    pub fn font_size(mut self, s: f32) -> Self {
        self.font_size = s;
        self
    }
    pub fn opacity(mut self, o: f32) -> Self {
        self.opacity = o;
        self
    }
    pub fn rotate(mut self, r: f32) -> Self {
        self.rotate = r;
        self
    }
    pub fn gap(mut self, x: f32, y: f32) -> Self {
        self.gap_x = x;
        self.gap_y = y;
        self
    }
    pub fn offset(mut self, x: f32, y: f32) -> Self {
        self.x_offset = x;
        self.y_offset = y;
        self
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Watermark {
            text: self.text.clone(),
            color: self.color,
            font_size: self.font_size,
            opacity: self.opacity,
            rotate: self.rotate,
            gap_x: self.gap_x,
            gap_y: self.gap_y,
            x_offset: self.x_offset,
            y_offset: self.y_offset,
        }
    }
}
