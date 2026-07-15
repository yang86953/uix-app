use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::{
    ComponentId, EventResult, SemanticEvent, SnapshotFields, SnapshotTransferItem, SystemEvent,
    WidgetTree,
};
use qrcode::{types::Color as QrModuleColor, EcLevel, QrCode};
use std::cell::{Cell, RefCell};

// ════════════════════════════════════════════════════════════════════════════
// QRCode
// ════════════════════════════════════════════════════════════════════════════

component! {
    pub struct QRCode {
        value: String,
        size: f32,
        error_level: u8,
        modules: Vec<bool>,
        module_count: usize,
        encoding_error: Option<String>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(self.size, self.size))
    }

    picture_policy => (&self) -> crate::draw::compositor::PicturePolicy {
        crate::draw::compositor::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let bg = Color::white();
        let fg = Color::black();
        ctx.fill_rect(frame, bg, None);

        if self.encoding_error.is_some() || self.module_count == 0 {
            ctx.text_center("QR !", frame, ctx.tokens().color_error(), 14.0);
            return;
        }

        // ISO/IEC 18004 要求四模块 quiet zone；使用正方形模块并居中，
        // 不叠加 logo 或圆角，以免破坏编码矩阵的可扫描性。
        const QUIET_ZONE: usize = 4;
        let symbol_modules = self.module_count + QUIET_ZONE * 2;
        let module_size = frame.w.min(frame.h) / symbol_modules as f32;
        if !module_size.is_finite() || module_size <= 0.0 {
            return;
        }
        let symbol_size = module_size * symbol_modules as f32;
        let origin = Point::new(
            frame.x + (frame.w - symbol_size) * 0.5 + QUIET_ZONE as f32 * module_size,
            frame.y + (frame.h - symbol_size) * 0.5 + QUIET_ZONE as f32 * module_size,
        );
        for y in 0..self.module_count {
            for x in 0..self.module_count {
                if self.modules[y * self.module_count + x] {
                    ctx.fill_rect(
                        Rect::new(
                            origin.x + x as f32 * module_size,
                            origin.y + y as f32 * module_size,
                            module_size,
                            module_size,
                        ),
                        fg,
                        None,
                    );
                }
            }
        }
    }
}
impl QRCode {
    pub fn new(value: &str) -> Self {
        let mut qr = Self {
            value: value.to_string(),
            size: 160.0,
            error_level: 1,
            modules: Vec::new(),
            module_count: 0,
            encoding_error: None,
        };
        qr.rebuild_encoding();
        qr
    }
    pub fn size(mut self, s: f32) -> Self {
        if s.is_finite() {
            self.size = s.max(1.0);
        }
        self
    }
    pub fn error_level(mut self, lv: u8) -> Self {
        self.error_level = lv.min(3);
        self.rebuild_encoding();
        self
    }

    /// 返回标准 QR 矩阵的边长（不含四模块 quiet zone）。
    pub fn module_count(&self) -> usize {
        self.module_count
    }

    /// 编码失败时返回具体原因；成功时为 `None`。
    pub fn encoding_error(&self) -> Option<&str> {
        self.encoding_error.as_deref()
    }

    pub fn is_valid(&self) -> bool {
        self.encoding_error.is_none() && self.module_count > 0
    }

    #[cfg(test)]
    pub(crate) fn module(&self, x: usize, y: usize) -> Option<bool> {
        (x < self.module_count && y < self.module_count)
            .then(|| self.modules[y * self.module_count + x])
    }

    fn rebuild_encoding(&mut self) {
        let level = match self.error_level {
            0 => EcLevel::L,
            1 => EcLevel::M,
            2 => EcLevel::Q,
            _ => EcLevel::H,
        };
        match QrCode::with_error_correction_level(self.value.as_bytes(), level) {
            Ok(code) => {
                self.module_count = code.width();
                self.modules = code
                    .into_colors()
                    .into_iter()
                    .map(|module| module == QrModuleColor::Dark)
                    .collect();
                self.encoding_error = None;
            }
            Err(error) => {
                self.modules.clear();
                self.module_count = 0;
                self.encoding_error = Some(error.to_string());
            }
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.value = next.value;
        self.size = next.size;
        self.error_level = next.error_level;
        self.modules = next.modules;
        self.module_count = next.module_count;
        self.encoding_error = next.encoding_error;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::QRCode {
            value: self.value.clone(),
            size: self.size,
            error_level: self.error_level,
            module_count: self.module_count,
            encoding_error: self.encoding_error.clone(),
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

component! {
    pub struct Transfer {
        source: Vec<TransferItem>,
        target: Vec<TransferItem>,
        initial_source: Vec<TransferItem>,
        initial_target: Vec<TransferItem>,
        last_frame_w: Cell<f32>,
        last_frame_h: Cell<f32>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(500.0, 200.0))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if let SystemEvent::PointerDown { pos, .. } = event {
            const LIST_HEADER_H: f32 = 24.0;
            const BTN_COL_W: f32 = 60.0;
            let item_h = 28.0;
            let half = {
                let w = self.last_frame_w.get();
                let w = if w > 0.0 { w } else { 500.0 };
                ((w - BTN_COL_W) * 0.5).max(40.0)
            };
            let frame_h = {
                let h = self.last_frame_h.get();
                if h > 0.0 { h } else { 200.0 }
            };
            let list_idx = |y: f32| -> Option<usize> {
                if y < LIST_HEADER_H {
                    return None;
                }
                Some(((y - LIST_HEADER_H) / item_h) as usize)
            };
            if pos.x < half {
                if let Some(idx) = list_idx(pos.y) {
                    if idx < self.source.len() {
                        self.source[idx].selected = !self.source[idx].selected;
                        return EventResult::Handled;
                    }
                }
            } else if pos.x > half + BTN_COL_W {
                if let Some(idx) = list_idx(pos.y) {
                    if idx < self.target.len() {
                        self.target[idx].selected = !self.target[idx].selected;
                        return EventResult::Handled;
                    }
                }
            } else {
                let btn_y = frame_h * 0.5 - 20.0;
                if pos.y >= btn_y && pos.y < btn_y + 20.0 {
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
                if pos.y >= btn_y + 24.0 && pos.y < btn_y + 44.0 {
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
        self.last_frame_w.set(frame.w);
        self.last_frame_h.set(frame.h);
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_quaternary();
        let primary = ctx.tokens().color_primary();
        let fill = ctx.tokens().color_fill_tertiary();
        const LIST_HEADER_H: f32 = 24.0;
        const BTN_COL_W: f32 = 60.0;
        let half = ((frame.w - BTN_COL_W) * 0.5).max(40.0);
        let item_h = 28.0;
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        let left_rect = Rect::new(frame.x, frame.y, half, frame.h);
        ctx.fill_rect(left_rect, bg, r);
        ctx.stroke_rect(left_rect, border, 1.0, r);
        let loc = crate::ui::locale::use_locale();
        ctx.draw_text(&format!("{} ({}项)", loc.transfer_source, self.source.len()), Point::new(frame.x + 8.0, frame.y + 6.0), text_sec, 12.0);
        for (i, item) in self.source.iter().enumerate() {
            let y = frame.y + LIST_HEADER_H + i as f32 * item_h;
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
        let right_x = frame.x + half + BTN_COL_W;
        let right_rect = Rect::new(right_x, frame.y, half, frame.h);
        ctx.fill_rect(right_rect, bg, r);
        ctx.stroke_rect(right_rect, border, 1.0, r);
        ctx.draw_text(&format!("{} ({}项)", loc.transfer_target, self.target.len()), Point::new(right_x + 8.0, frame.y + 6.0), text_sec, 12.0);
        for (i, item) in self.target.iter().enumerate() {
            let y = frame.y + LIST_HEADER_H + i as f32 * item_h;
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
            last_frame_w: Cell::new(500.0),
            last_frame_h: Cell::new(200.0),
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

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.initial_source = next.initial_source;
        self.initial_target = next.initial_target;
    }

    #[cfg(test)]
    pub(crate) fn source_count(&self) -> usize {
        self.source.len()
    }

    #[cfg(test)]
    pub(crate) fn target_count(&self) -> usize {
        self.target.len()
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
component! {
    pub struct Upload {
        accept: String,
        multiple: bool,
        file_list: Vec<UploadFile>,
        drag: bool,
        drag_hover: bool,
        max_count: usize,
        pending_change: RefCell<Option<String>>,
    }

    measure => (&self, _constraints: Constraints) -> Size {
        let list_h = self.file_list.len() as f32 * 32.0;
        Size::new(300.0, 100.0 + list_h)
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown { .. } => {
                let file_name = Self::simulated_file_name(&self.accept, self.file_list.len() + 1);
                self.add_file(&file_name);
                self.pending_change
                    .replace(Some(format!("{}:pending", file_name)));
                EventResult::Handled
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
    fn simulated_file_name(accept: &str, index: usize) -> String {
        let ext = accept
            .split(',')
            .map(|s| s.trim().trim_start_matches('.'))
            .find(|s| !s.is_empty() && *s != "*")
            .unwrap_or("bin");
        format!("upload_{index}.{ext}")
    }

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

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.accept = next.accept;
        self.multiple = next.multiple;
        self.drag = next.drag;
        self.max_count = next.max_count;
        if self.file_list.len() > self.max_count {
            self.file_list.truncate(self.max_count);
        }
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

component! {
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

    measure => (&self, _constraints: Constraints) -> Size {
        Size::zero()
    }

    picture_policy => (&self) -> crate::draw::compositor::PicturePolicy {
        crate::draw::compositor::PicturePolicy::Eligible
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

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.text = next.text;
        self.color = next.color;
        self.font_size = next.font_size;
        self.opacity = next.opacity;
        self.rotate = next.rotate;
        self.gap_x = next.gap_x;
        self.gap_y = next.gap_y;
        self.x_offset = next.x_offset;
        self.y_offset = next.y_offset;
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
