use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields,
    SnapshotTransferItem, SystemEvent, WidgetTree,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferItem {
    pub key: String,
    pub title: String,
    pub selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransferPane {
    Source,
    Target,
}

component! {
    pub struct Transfer {
        source: Vec<TransferItem>,
        target: Vec<TransferItem>,
        last_frame: Cell<Option<Rect>>,
        focused: bool,
        active_pane: TransferPane,
        active_index: usize,
        pending_change: RefCell<Option<String>>,
    }

    tab_index => (&self) -> i32 { i32::from(!self.source.is_empty() || !self.target.is_empty()) }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(500.0, 200.0))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => self.pointer_down(*pos),
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => self.key_down(*key),
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|keys| SemanticEvent::change(id, keys))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
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
        let left_border = if self.focused && self.active_pane == TransferPane::Source { primary } else { border };
        ctx.stroke_rect(left_rect, left_border, if left_border == primary { 1.5 } else { 1.0 }, r);
        let loc = crate::ui::locale::use_locale();
        ctx.draw_text(&format!("{} ({}项)", loc.transfer_source, self.source.len()), Point::new(frame.x + 8.0, frame.y + 6.0), text_sec, 12.0);
        for (i, item) in self.source.iter().enumerate() {
            let y = frame.y + LIST_HEADER_H + i as f32 * item_h;
            let row_rect = Rect::new(frame.x, y, half, item_h);
            let row_y = ctx.visual_center_y(row_rect, 13.0);
            if item.selected { ctx.fill_rect(row_rect, fill, None); }
            if self.focused && self.active_pane == TransferPane::Source && self.active_index == i {
                ctx.stroke_rect(row_rect, primary, 1.0, None);
            }
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
        let right_border = if self.focused && self.active_pane == TransferPane::Target { primary } else { border };
        ctx.stroke_rect(right_rect, right_border, if right_border == primary { 1.5 } else { 1.0 }, r);
        ctx.draw_text(&format!("{} ({}项)", loc.transfer_target, self.target.len()), Point::new(right_x + 8.0, frame.y + 6.0), text_sec, 12.0);
        for (i, item) in self.target.iter().enumerate() {
            let y = frame.y + LIST_HEADER_H + i as f32 * item_h;
            let row_rect = Rect::new(right_x, y, half, item_h);
            let row_y = ctx.visual_center_y(row_rect, 13.0);
            if item.selected { ctx.fill_rect(row_rect, fill, None); }
            if self.focused && self.active_pane == TransferPane::Target && self.active_index == i {
                ctx.stroke_rect(row_rect, primary, 1.0, None);
            }
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
            last_frame: Cell::new(None),
            focused: false,
            active_pane: TransferPane::Source,
            active_index: 0,
            pending_change: RefCell::new(None),
        }
    }
    pub fn source(mut self, items: Vec<TransferItem>) -> Self {
        self.source = items;
        self
    }
    pub fn target(mut self, items: Vec<TransferItem>) -> Self {
        self.target = items;
        self
    }

    pub fn source_items(&self) -> &[TransferItem] {
        &self.source
    }

    pub fn target_items(&self) -> &[TransferItem] {
        &self.target
    }

    pub fn active_index(&self) -> usize {
        self.active_index
    }

    pub fn target_is_active(&self) -> bool {
        self.active_pane == TransferPane::Target
    }

    pub(crate) fn sync_from(&mut self, _next: Self) {}

    #[cfg(test)]
    pub(crate) fn set_frame_for_test(&self, frame: Rect) {
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Transfer {
            source: self
                .source
                .iter()
                .map(SnapshotTransferItem::from_transfer_item)
                .collect(),
            target: self
                .target
                .iter()
                .map(SnapshotTransferItem::from_transfer_item)
                .collect(),
        }
    }

    fn pointer_down(&mut self, pos: Point) -> EventResult {
        const HEADER: f32 = 24.0;
        const BUTTONS: f32 = 60.0;
        const ROW: f32 = 28.0;
        let Some(frame) = self.last_frame.get().filter(|frame| frame.contains(pos)) else {
            return EventResult::NotHandled;
        };
        let x = pos.x - frame.x;
        let y = pos.y - frame.y;
        let half = ((frame.w - BUTTONS) * 0.5).max(40.0);
        let row = (y >= HEADER).then(|| ((y - HEADER) / ROW) as usize);
        if x < half {
            return self.toggle_row(TransferPane::Source, row);
        }
        if x > half + BUTTONS {
            return self.toggle_row(TransferPane::Target, row);
        }
        let button_y = frame.h * 0.5 - 20.0;
        if y >= button_y && y < button_y + 20.0 {
            self.move_selected(TransferPane::Source);
            return EventResult::Handled;
        }
        if y >= button_y + 24.0 && y < button_y + 44.0 {
            self.move_selected(TransferPane::Target);
            return EventResult::Handled;
        }
        EventResult::NotHandled
    }

    fn key_down(&mut self, key: KeyCode) -> EventResult {
        if self.source.is_empty() && self.target.is_empty() {
            return EventResult::NotHandled;
        }
        match key {
            KeyCode::Left => self.activate_pane(TransferPane::Source),
            KeyCode::Right => self.activate_pane(TransferPane::Target),
            KeyCode::Up => self.active_index = self.active_index.saturating_sub(1),
            KeyCode::Down => {
                self.active_index =
                    (self.active_index + 1).min(self.active_len().saturating_sub(1));
            }
            KeyCode::Home => self.active_index = 0,
            KeyCode::End => self.active_index = self.active_len().saturating_sub(1),
            KeyCode::Space => {
                self.toggle_active();
            }
            KeyCode::Enter => {
                if !self.active_items().iter().any(|item| item.selected) {
                    self.toggle_active();
                }
                self.move_selected(self.active_pane);
            }
            _ => return EventResult::NotHandled,
        }
        EventResult::Handled
    }

    fn toggle_row(&mut self, pane: TransferPane, row: Option<usize>) -> EventResult {
        let Some(index) = row else {
            return EventResult::NotHandled;
        };
        let items = match pane {
            TransferPane::Source => &mut self.source,
            TransferPane::Target => &mut self.target,
        };
        let Some(item) = items.get_mut(index) else {
            return EventResult::NotHandled;
        };
        item.selected = !item.selected;
        self.active_pane = pane;
        self.active_index = index;
        self.focused = true;
        EventResult::Handled
    }

    fn toggle_active(&mut self) {
        let index = self.active_index;
        let items = match self.active_pane {
            TransferPane::Source => &mut self.source,
            TransferPane::Target => &mut self.target,
        };
        if let Some(item) = items.get_mut(index) {
            item.selected = !item.selected;
        }
    }

    fn move_selected(&mut self, from: TransferPane) -> bool {
        let (source, target) = match from {
            TransferPane::Source => (&mut self.source, &mut self.target),
            TransferPane::Target => (&mut self.target, &mut self.source),
        };
        let mut kept = Vec::with_capacity(source.len());
        let mut moved = Vec::new();
        for mut item in source.drain(..) {
            if item.selected {
                item.selected = false;
                moved.push(item);
            } else {
                kept.push(item);
            }
        }
        *source = kept;
        if moved.is_empty() {
            return false;
        }
        target.extend(moved);
        self.active_index = self.active_index.min(self.active_len().saturating_sub(1));
        self.pending_change
            .replace(Some(self.target_keys_payload()));
        true
    }

    fn activate_pane(&mut self, pane: TransferPane) {
        self.active_pane = pane;
        self.active_index = self.active_index.min(self.active_len().saturating_sub(1));
    }

    fn active_items(&self) -> &[TransferItem] {
        match self.active_pane {
            TransferPane::Source => &self.source,
            TransferPane::Target => &self.target,
        }
    }

    fn active_len(&self) -> usize {
        self.active_items().len()
    }

    fn target_keys_payload(&self) -> String {
        self.target
            .iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>()
            .join(",")
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
#[derive(Debug, Clone, PartialEq)]
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

    measure => (&self, constraints: Constraints) -> Size {
        let list_h = self.file_list.len() as f32 * 32.0;
        constraints.clamp(Size::new(300.0, 100.0 + list_h))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::FileDrop { files, .. } if self.drag => self.queue_dropped_files(files),
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
        crate::ui::widgets::icon::paint_icon_in_frame(
            ctx,
            "upload",
            Rect::new(frame.x + frame.w * 0.5 - 24.0, frame.y + 12.0, 48.0, 40.0),
            text_sec,
            24.0,
        );
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
            crate::ui::widgets::icon::paint_icon_in_frame(
                ctx,
                "file",
                Rect::new(frame.x + 6.0, y, 18.0, 24.0),
                text_sec,
                14.0,
            );
            ctx.draw_text(&f.name, Point::new(frame.x + 28.0, y + 5.0), text, 12.0);
            if f.status == UploadStatus::Uploading {
                let bar_w = (frame.w - 40.0).max(0.0);
                let bar_rect = Rect::new(frame.x + 10.0, y + 20.0, bar_w * f.progress, 4.0);
                ctx.fill_rect(bar_rect, primary, None);
            }
            let status_icon = match f.status {
                UploadStatus::Done => "check",
                UploadStatus::Error => "x",
                UploadStatus::Pending => "clock",
                UploadStatus::Uploading => "refresh-cw",
            };
            crate::ui::widgets::icon::paint_icon_in_frame(
                ctx,
                status_icon,
                Rect::new(frame.x + frame.w - 24.0, y, 20.0, 24.0),
                status_color,
                12.0,
            );
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
        let _ = self.try_add_file(name);
    }
    pub fn try_add_file(&mut self, path: &str) -> bool {
        if !self.accepts_file(path) {
            return false;
        }
        self.push_file(Self::display_name(path))
    }
    pub fn remove_file(&mut self, index: usize) -> Option<UploadFile> {
        (index < self.file_list.len()).then(|| self.file_list.remove(index))
    }
    pub fn clear_files(&mut self) {
        self.file_list.clear();
    }
    pub fn update_progress(&mut self, idx: usize, progress: f32) {
        if idx < self.file_list.len() {
            self.file_list[idx].progress = if progress.is_finite() {
                progress.clamp(0.0, 1.0)
            } else {
                0.0
            };
            self.file_list[idx].status = UploadStatus::Uploading;
        }
    }
    pub fn complete_file(&mut self, idx: usize, success: bool) {
        if idx < self.file_list.len() {
            if success {
                self.file_list[idx].progress = 1.0;
            }
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
    pub fn files(&self) -> &[UploadFile] {
        &self.file_list
    }

    fn queue_dropped_files(&mut self, files: &[String]) -> EventResult {
        let limit = if self.multiple { usize::MAX } else { 1 };
        let mut accepted = Vec::new();
        for path in files {
            if accepted.len() >= limit {
                continue;
            }
            if self.try_add_file(path) {
                accepted.push(Self::display_name(path).to_string());
            }
        }

        if accepted.is_empty() {
            return EventResult::NotHandled;
        }
        self.pending_change.replace(Some(
            accepted
                .iter()
                .map(|name| format!("{name}:pending"))
                .collect::<Vec<_>>()
                .join(","),
        ));
        EventResult::Handled
    }

    fn push_file(&mut self, name: &str) -> bool {
        if self.file_list.len() >= self.max_count {
            return false;
        }
        self.file_list.push(UploadFile {
            name: name.to_string(),
            size: 0,
            progress: 0.0,
            status: UploadStatus::Pending,
        });
        true
    }

    fn accepts_file(&self, path: &str) -> bool {
        let accept = self.accept.trim();
        if accept.is_empty() || matches!(accept, "*" | "*/*") {
            return true;
        }
        let name = Self::display_name(path);
        let extension = name.rsplit_once('.').map(|(_, extension)| extension);
        accept.split(',').map(str::trim).any(|pattern| {
            if matches!(pattern, "*" | "*/*") {
                return true;
            }
            if pattern.contains('/') {
                return false;
            }
            let expected = pattern
                .strip_prefix("*.")
                .or_else(|| pattern.strip_prefix('.'))
                .unwrap_or(pattern);
            !expected.is_empty()
                && extension.is_some_and(|actual| actual.eq_ignore_ascii_case(expected))
        })
    }

    fn display_name(path: &str) -> &str {
        path.rsplit(['/', '\\'])
            .find(|segment| !segment.is_empty())
            .unwrap_or(path)
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
            files: self.file_list.clone(),
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
        c.a = (self.opacity * 255.0).round() as u8;
        let columns = (frame.w.max(0.0) / self.gap_x).ceil() as usize + 2;
        let rows = (frame.h.max(0.0) / self.gap_y).ceil() as usize + 2;

        ctx.push_clip(frame);
        for gy in 0..rows {
            for gx in 0..columns {
                ctx.draw_text(
                    &self.text,
                    self.tile_position(frame, gx, gy),
                    c,
                    self.font_size,
                );
            }
        }
        ctx.pop_clip();
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        // Watermark 通过 render 在全帧范围绘制水印，
        // dirty_rect 返回实际 frame 区域（通常是全屏），
        // 禁止返回硬编码巨型区域（原 -10000~20000）避免脏区域爆炸。
        frame
    }
}
impl Watermark {
    const DEFAULT_FONT_SIZE: f32 = 14.0;
    const DEFAULT_OPACITY: f32 = 0.15;
    const DEFAULT_ROTATE: f32 = -22.0;
    const DEFAULT_GAP_X: f32 = 200.0;
    const DEFAULT_GAP_Y: f32 = 160.0;

    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            color: Color::from_rgba(0, 0, 0, 255),
            font_size: Self::DEFAULT_FONT_SIZE,
            opacity: Self::DEFAULT_OPACITY,
            rotate: Self::DEFAULT_ROTATE,
            gap_x: Self::DEFAULT_GAP_X,
            gap_y: Self::DEFAULT_GAP_Y,
            x_offset: 0.0,
            y_offset: 0.0,
        }
    }
    pub fn color(mut self, c: Color) -> Self {
        self.color = c;
        self
    }
    pub fn font_size(mut self, s: f32) -> Self {
        self.font_size = Self::positive_or(s, Self::DEFAULT_FONT_SIZE);
        self
    }
    pub fn opacity(mut self, o: f32) -> Self {
        self.opacity = if o.is_finite() {
            o.clamp(0.0, 1.0)
        } else {
            Self::DEFAULT_OPACITY
        };
        self
    }
    pub fn rotate(mut self, r: f32) -> Self {
        self.rotate = if r.is_finite() {
            r
        } else {
            Self::DEFAULT_ROTATE
        };
        self
    }
    pub fn gap(mut self, x: f32, y: f32) -> Self {
        self.gap_x = Self::positive_or(x, Self::DEFAULT_GAP_X);
        self.gap_y = Self::positive_or(y, Self::DEFAULT_GAP_Y);
        self
    }
    pub fn offset(mut self, x: f32, y: f32) -> Self {
        self.x_offset = if x.is_finite() { x } else { 0.0 };
        self.y_offset = if y.is_finite() { y } else { 0.0 };
        self
    }

    pub(crate) fn tile_position(&self, frame: Rect, gx: usize, gy: usize) -> Point {
        let base_x = gx as f32 * self.gap_x + self.x_offset;
        let base_y = gy as f32 * self.gap_y + self.y_offset;
        let angle_rad = self.rotate.to_radians();
        let (sin_a, cos_a) = angle_rad.sin_cos();
        let half = self.text.len() as f32 * self.font_size * 0.3;
        let rotated_x = (base_x - half) * cos_a - (base_y - half) * sin_a + half;
        let rotated_y = (base_x - half) * sin_a + (base_y - half) * cos_a + half;
        Point::new(frame.x + rotated_x, frame.y + rotated_y)
    }

    fn positive_or(value: f32, fallback: f32) -> f32 {
        if value.is_finite() && value >= 1.0 {
            value
        } else {
            fallback
        }
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
