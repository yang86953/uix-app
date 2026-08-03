use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Radius;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::{
    ComponentId, EventResult, MouseButton, SemanticEvent, SnapshotFields, SystemEvent, WidgetTree,
};
use std::cell::{Cell, RefCell};

// ════════════════════════════════════════════════════════════════════════════
// Upload
// ════════════════════════════════════════════════════════════════════════════

/// 上传文件项。
#[derive(Debug, Clone, PartialEq)]
pub struct UploadFile {
    pub name: String,
    /// Original path for a real local file; synthetic queue entries keep `None`.
    pub source_path: Option<String>,
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
        max_size: Option<u64>,
        show_upload_list: bool,
        preview_image: bool,
        manual: bool,
        last_width: Cell<f32>,
        layout_requested: Cell<bool>,
        pending_change: RefCell<Option<String>>,
        focused: bool,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        let list_h = if self.show_upload_list {
            self.file_list.len() as f32 * 32.0
        } else {
            0.0
        };
        constraints.clamp(Size::new(300.0, 100.0 + list_h))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => self.remove_file_at(*pos),
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

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        self.last_width.set(frame.w.max(0.0));
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
        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(upload_rect, primary, 2.0, r);
        }
        if self.drag && self.drag_hover {
            ctx.stroke_rect(Rect::new(frame.x + 4.0, frame.y + 4.0, frame.w - 8.0, 92.0), primary, 1.0, Some(Radius::uniform(ctx.tokens().border_radius_sm())));
        }
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            "upload",
            Rect::new(frame.x + frame.w * 0.5 - 24.0, frame.y + 12.0, 48.0, 40.0),
            text_sec,
            24.0,
        );
        let loc = crate::ui::component::locale::use_locale();
        ctx.draw_text(loc.upload_drag, Point::new(frame.x + frame.w * 0.5 - 48.0, frame.y + 60.0), text_sec, 13.0);
        if !self.accept.is_empty() && self.accept != "*" {
            let suffix = format!("{}: {}", loc.filter_title, self.accept);
            ctx.draw_text(&suffix, Point::new(frame.x + frame.w * 0.5 - 36.0, frame.y + 78.0), text_sec, 10.0);
        }

        if !self.show_upload_list {
            return;
        }

        for (i, f) in self.file_list.iter().enumerate() {
            let y = frame.y + 104.0 + i as f32 * 32.0;
            let status_color = match f.status {
                UploadStatus::Error => error,
                UploadStatus::Done => success,
                UploadStatus::Uploading => primary,
                UploadStatus::Pending => text_sec,
            };
            let thumbnail = Rect::new(frame.x + 4.0, y + 4.0, 24.0, 24.0);
            let drew_preview = self.preview_image
                && f.source_path.as_deref().is_some_and(|path| {
                    let Some(handle) = ctx.image_service().ensure_loaded(path) else {
                        return false;
                    };
                    let device_scale = ctx.device_pixel_ratio().max(f32::EPSILON);
                    let target_side = (24.0 * device_scale).ceil().clamp(1.0, 4096.0) as u32;
                    let drawable = ctx
                        .image_service()
                        .rounded_rect_sized(
                            handle,
                            target_side,
                            target_side,
                            2.0 * device_scale,
                            true,
                        )
                        .unwrap_or(handle);
                    ctx.draw_image_fill(drawable, thumbnail);
                    true
                });
            if !drew_preview {
                crate::ui::widgets::icon::Icon::paint_in_frame(
                    ctx,
                    "file",
                    Rect::new(frame.x + 6.0, y, 18.0, 24.0),
                    text_sec,
                    14.0,
                );
            }
            let text_x = frame.x + if drew_preview { 34.0 } else { 28.0 };
            let file_text_clip = Rect::new(text_x, y, (frame.x + frame.w - 56.0 - text_x).max(0.0), 32.0);
            ctx.push_clip(file_text_clip);
            ctx.draw_text(&f.name, Point::new(text_x, y + 2.0), text, 12.0);
            ctx.draw_text(
                &Self::format_file_size(f.size),
                Point::new(text_x, y + 17.0),
                text_sec,
                10.0,
            );
            if f.status == UploadStatus::Uploading {
                let bar_w = (frame.x + frame.w - 56.0 - text_x).max(0.0);
                let bar_rect = Rect::new(text_x, y + 28.0, bar_w * f.progress, 3.0);
                ctx.fill_rect(bar_rect, primary, None);
            }
            ctx.pop_clip();
            let status_icon = match f.status {
                UploadStatus::Done => "check",
                UploadStatus::Error => "x",
                UploadStatus::Pending => "clock",
                UploadStatus::Uploading => "refresh-cw",
            };
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                status_icon,
                Rect::new(frame.x + frame.w - 48.0, y + 4.0, 20.0, 24.0),
                status_color,
                12.0,
            );
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                "x",
                Rect::new(frame.x + frame.w - 24.0, y + 4.0, 20.0, 24.0),
                text_sec,
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
            max_size: None,
            show_upload_list: true,
            preview_image: false,
            manual: false,
            last_width: Cell::new(0.0),
            layout_requested: Cell::new(false),
            pending_change: RefCell::new(None),
            focused: false,
        }
    }
    pub fn dragger() -> Self {
        Self::new().drag(true)
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
    /// Limit newly queued real files to at most `bytes` bytes.
    pub fn max_size(mut self, bytes: u64) -> Self {
        self.max_size = Some(bytes);
        self
    }
    pub fn show_upload_list(mut self, show: bool) -> Self {
        self.show_upload_list = show;
        self
    }
    /// Render decodable real local image files as list thumbnails.
    pub fn preview_image(mut self, preview: bool) -> Self {
        self.preview_image = preview;
        self
    }
    /// Require an explicit [`Upload::upload`] call to produce an application-side upload batch.
    pub fn manual(mut self, manual: bool) -> Self {
        self.manual = manual;
        self
    }
    pub fn add_file(&mut self, name: &str) {
        let _ = self.try_add_file(name);
    }
    pub fn try_add_file(&mut self, path: &str) -> bool {
        if !self.accepts_file(path) {
            return false;
        }
        let size = match std::fs::metadata(path) {
            Ok(metadata) if metadata.is_file() => Some(metadata.len()),
            Ok(_) => return false,
            Err(_) => None,
        };
        if self
            .max_size
            .is_some_and(|max_size| size.is_some_and(|size| size > max_size))
        {
            return false;
        }
        self.push_file(
            Self::display_name(path),
            size.unwrap_or(0),
            size.map(|_| path.to_string()),
        )
    }
    pub fn remove_file(&mut self, index: usize) -> Option<UploadFile> {
        if index >= self.file_list.len() {
            return None;
        }
        let removed = self.file_list.remove(index);
        if self.show_upload_list {
            self.layout_requested.set(true);
        }
        Some(removed)
    }
    pub fn clear_files(&mut self) {
        if self.file_list.is_empty() {
            return;
        }
        self.file_list.clear();
        if self.show_upload_list {
            self.layout_requested.set(true);
        }
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

    /// Mark pending manual entries as uploading and return this call's application-side batch.
    pub fn upload(&mut self) -> Vec<UploadFile> {
        if !self.manual {
            return Vec::new();
        }
        let mut batch = Vec::new();
        for file in &mut self.file_list {
            if file.status != UploadStatus::Pending {
                continue;
            }
            file.status = UploadStatus::Uploading;
            file.progress = 0.0;
            batch.push(file.clone());
        }
        batch
    }

    fn remove_file_at(&mut self, pos: Point) -> EventResult {
        if !self.show_upload_list || self.file_list.is_empty() {
            return EventResult::NotHandled;
        }
        let width = self.last_width.get();
        if width <= 0.0 || pos.x < (width - 28.0).max(0.0) || pos.x > width || pos.y < 104.0 {
            return EventResult::NotHandled;
        }
        let index = ((pos.y - 104.0) / 32.0).floor() as usize;
        let Some(removed) = self.remove_file(index) else {
            return EventResult::NotHandled;
        };
        self.pending_change
            .replace(Some(format!("{}:removed", removed.name)));
        EventResult::Handled
    }

    fn format_file_size(bytes: u64) -> String {
        const KIB: f64 = 1024.0;
        const MIB: f64 = KIB * 1024.0;
        if bytes >= MIB as u64 {
            format!("{:.1} MiB", bytes as f64 / MIB)
        } else if bytes >= KIB as u64 {
            format!("{:.1} KiB", bytes as f64 / KIB)
        } else {
            format!("{bytes} B")
        }
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

    fn push_file(&mut self, name: &str, size: u64, source_path: Option<String>) -> bool {
        if self.file_list.len() >= self.max_count {
            return false;
        }
        self.file_list.push(UploadFile {
            name: name.to_string(),
            source_path,
            size,
            progress: 0.0,
            status: UploadStatus::Pending,
        });
        if self.show_upload_list {
            self.layout_requested.set(true);
        }
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
        let list_visibility_changed = self.show_upload_list != next.show_upload_list;
        self.accept = next.accept;
        self.multiple = next.multiple;
        self.drag = next.drag;
        self.max_count = next.max_count;
        self.max_size = next.max_size;
        self.show_upload_list = next.show_upload_list;
        self.preview_image = next.preview_image;
        self.manual = next.manual;
        let old_len = self.file_list.len();
        if self.file_list.len() > self.max_count {
            self.file_list.truncate(self.max_count);
        }
        if list_visibility_changed || old_len != self.file_list.len() {
            self.layout_requested.set(true);
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Upload {
            accept: self.accept.clone(),
            multiple: self.multiple,
            drag: self.drag,
            max_count: self.max_count,
            max_size: self.max_size,
            show_upload_list: self.show_upload_list,
            preview_image: self.preview_image,
            manual: self.manual,
            files: self.file_list.clone(),
        }
    }
}
impl Default for Upload {
    fn default() -> Self {
        Self::new()
    }
}
