use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::{Color, Radius};
use crate::ui::component::paint_context::PaintContext;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields,
    SnapshotTransferItem, SystemEvent, WidgetTree,
};
use qrcode::{types::Color as QrModuleColor, EcLevel, QrCode};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

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

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
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

impl TransferItem {
    pub fn new(key: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            selected: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDirection {
    LeftToRight,
    RightToLeft,
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
        source_title: String,
        target_title: String,
        searchable: bool,
        search_query: String,
        item_renderer: Option<Rc<dyn Fn(&TransferItem) -> crate::ui::view::ViewNode>>,
        change_callback: Option<Rc<dyn Fn(&[TransferItem], &[TransferItem], MoveDirection)>>,
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

    accepts_text_input => (&self) -> bool { self.searchable }

    text_input_cursor_rect => (&self) -> Rect {
        let width = (self.search_query.chars().count() as f32 * 8.0 + 8.0).clamp(8.0, 280.0);
        Rect::new(8.0 + width, 4.0, 1.0, 20.0)
    }

    build_view_children => (&self) -> Vec<crate::ui::view::ViewNode> {
        let Some(factory) = self.item_renderer.as_ref() else {
            return Vec::new();
        };
        let mut children = Vec::with_capacity(self.source.len() + self.target.len());
        for item in &self.source {
            children.push(factory(item).key(format!("transfer:source:{}", item.key)));
        }
        for item in &self.target {
            children.push(factory(item).key(format!("transfer:target:{}", item.key)));
        }
        children
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        let half = ((frame.w - 60.0) * 0.5).max(40.0);
        let row_h = 28.0;
        let header_h = 24.0 + if self.searchable { 24.0 } else { 0.0 };
        let mut layouts = Vec::with_capacity(children.len());
        for (index, child) in children.iter().enumerate() {
            let (pane, raw_index) = if index < self.source.len() {
                (TransferPane::Source, index)
            } else {
                (TransferPane::Target, index - self.source.len())
            };
            let visible = self.visible_indices(pane).into_iter().position(|item| item == raw_index);
            let rect = if let Some(visible_index) = visible {
                let x = if pane == TransferPane::Source { frame.x } else { frame.x + half + 60.0 };
                Rect::new(x, frame.y + header_h + visible_index as f32 * row_h, half, row_h)
            } else {
                Rect::zero()
            };
            layouts.push((child.id, rect));
        }
        layouts
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
            SystemEvent::KeyDown { key: KeyCode::Backspace, .. }
                if self.searchable && !self.search_query.is_empty() =>
            {
                self.search_query.pop();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key: KeyCode::Escape, .. }
                if self.searchable && !self.search_query.is_empty() =>
            {
                self.search_query.clear();
                EventResult::Handled
            }
            SystemEvent::TextInput { text } | SystemEvent::Paste { text }
                if self.searchable && !text.is_empty() && !text.chars().any(char::is_control) =>
            {
                self.search_query.push_str(text);
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

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_quaternary();
        let primary = ctx.tokens().color_primary();
        let fill = ctx.tokens().color_fill_tertiary();
        const LIST_HEADER_H: f32 = 24.0;
        let search_h = if self.searchable { 24.0 } else { 0.0 };
        let content_header_h = LIST_HEADER_H + search_h;
        const BTN_COL_W: f32 = 60.0;
        let half = ((frame.w - BTN_COL_W) * 0.5).max(40.0);
        let item_h = 28.0;
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        let left_rect = Rect::new(frame.x, frame.y, half, frame.h);
        ctx.fill_rect(left_rect, bg, r);
        let left_border = if self.focused
            && tree.keyboard_focus_visible()
            && self.active_pane == TransferPane::Source
        {
            primary
        } else {
            border
        };
        ctx.stroke_rect(left_rect, left_border, if left_border == primary { 1.5 } else { 1.0 }, r);
        let loc = crate::ui::component::locale::use_locale();
        let source_title = if self.source_title.is_empty() { loc.transfer_source } else { &self.source_title };
        if self.searchable {
            ctx.stroke_rect(
                Rect::new(frame.x + 4.0, frame.y + 2.0, (half - 8.0).max(0.0), 20.0),
                border,
                1.0,
                None,
            );
            ctx.draw_text(
                &format!("搜索: {}", self.search_query),
                Point::new(frame.x + 8.0, frame.y + 6.0),
                text_sec,
                11.0,
            );
        }
        ctx.draw_text(
            &format!("{} ({}项)", source_title, self.source.len()),
            Point::new(frame.x + 8.0, frame.y + search_h + 6.0),
            text_sec,
            12.0,
        );
        for (i, raw_index) in self.visible_indices(TransferPane::Source).into_iter().enumerate() {
            let item = &self.source[raw_index];
            let y = frame.y + content_header_h + i as f32 * item_h;
            let row_rect = Rect::new(frame.x, y, half, item_h);
            let row_y = ctx.visual_center_y(row_rect, 13.0);
            if item.selected { ctx.fill_rect(row_rect, fill, None); }
            if self.focused
                && tree.keyboard_focus_visible()
                && self.active_pane == TransferPane::Source
                && self.active_index == raw_index
            {
                ctx.stroke_rect(row_rect, primary, 1.0, None);
            }
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                if item.selected { "check-square" } else { "square" },
                Rect::new(frame.x + 4.0, y, 20.0, item_h),
                text,
                12.0,
            );
            if self.item_renderer.is_none() {
                ctx.draw_text(&item.title, Point::new(frame.x + 26.0, row_y), text, 13.0);
            }
        }
        let btn_y = frame.y + frame.h * 0.5 - 20.0;
        let rbtn_rect = Rect::new(frame.x + half + 8.0, btn_y, 44.0, 20.0);
        let lbtn_rect = Rect::new(frame.x + half + 8.0, btn_y + 24.0, 44.0, 20.0);
        ctx.fill_rect(rbtn_rect, primary, Some(Radius::uniform(3.0)));
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            "arrow-right",
            rbtn_rect,
            Color::white(),
            14.0,
        );
        ctx.fill_rect(lbtn_rect, border, Some(Radius::uniform(3.0)));
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            "arrow-left",
            lbtn_rect,
            text,
            14.0,
        );
        let right_x = frame.x + half + BTN_COL_W;
        let right_rect = Rect::new(right_x, frame.y, half, frame.h);
        ctx.fill_rect(right_rect, bg, r);
        let right_border = if self.focused
            && tree.keyboard_focus_visible()
            && self.active_pane == TransferPane::Target
        {
            primary
        } else {
            border
        };
        ctx.stroke_rect(right_rect, right_border, if right_border == primary { 1.5 } else { 1.0 }, r);
        let target_title = if self.target_title.is_empty() { loc.transfer_target } else { &self.target_title };
        if self.searchable {
            ctx.stroke_rect(
                Rect::new(right_x + 4.0, frame.y + 2.0, (half - 8.0).max(0.0), 20.0),
                border,
                1.0,
                None,
            );
            ctx.draw_text(
                &format!("搜索: {}", self.search_query),
                Point::new(right_x + 8.0, frame.y + 6.0),
                text_sec,
                11.0,
            );
        }
        ctx.draw_text(
            &format!("{} ({}项)", target_title, self.target.len()),
            Point::new(right_x + 8.0, frame.y + search_h + 6.0),
            text_sec,
            12.0,
        );
        for (i, raw_index) in self.visible_indices(TransferPane::Target).into_iter().enumerate() {
            let item = &self.target[raw_index];
            let y = frame.y + content_header_h + i as f32 * item_h;
            let row_rect = Rect::new(right_x, y, half, item_h);
            let row_y = ctx.visual_center_y(row_rect, 13.0);
            if item.selected { ctx.fill_rect(row_rect, fill, None); }
            if self.focused
                && tree.keyboard_focus_visible()
                && self.active_pane == TransferPane::Target
                && self.active_index == raw_index
            {
                ctx.stroke_rect(row_rect, primary, 1.0, None);
            }
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                if item.selected { "check-square" } else { "square" },
                Rect::new(right_x + 4.0, y, 20.0, item_h),
                text,
                12.0,
            );
            if self.item_renderer.is_none() {
                ctx.draw_text(&item.title, Point::new(right_x + 26.0, row_y), text, 13.0);
            }
        }
    }
}
impl Transfer {
    pub fn new() -> Self {
        Self {
            source: Vec::new(),
            target: Vec::new(),
            source_title: String::new(),
            target_title: String::new(),
            searchable: false,
            search_query: String::new(),
            item_renderer: None,
            change_callback: None,
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

    pub fn left_data(self, items: Vec<TransferItem>) -> Self {
        self.source(items)
    }

    pub fn right_data(self, items: Vec<TransferItem>) -> Self {
        self.target(items)
    }

    pub fn titles(mut self, source: impl Into<String>, target: impl Into<String>) -> Self {
        self.source_title = source.into();
        self.target_title = target.into();
        self
    }

    pub fn searchable(mut self, value: bool) -> Self {
        self.searchable = value;
        if !value {
            self.search_query.clear();
        }
        self
    }

    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    /// 自定义条目视图工厂；默认绘制仍使用稳定文本快照。
    pub fn render_item<F, V>(mut self, factory: F) -> Self
    where
        F: Fn(&TransferItem) -> V + 'static,
        V: crate::ui::view::View,
    {
        self.item_renderer = Some(Rc::new(move |item| {
            crate::ui::view::View::build(factory(item))
        }));
        self
    }

    pub fn on_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(&[TransferItem], &[TransferItem], MoveDirection) + 'static,
    {
        self.change_callback = Some(Rc::new(callback));
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

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.source_title = next.source_title;
        self.target_title = next.target_title;
        self.searchable = next.searchable;
        self.search_query = if next.searchable {
            next.search_query
        } else {
            String::new()
        };
        self.item_renderer = next.item_renderer;
        self.change_callback = next.change_callback;
    }

    #[cfg(test)]
    pub(crate) fn set_frame_for_test(&self, frame: Rect) {
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
    }

    fn visible_indices(&self, pane: TransferPane) -> Vec<usize> {
        let query = self.search_query.trim().to_lowercase();
        let items = match pane {
            TransferPane::Source => &self.source,
            TransferPane::Target => &self.target,
        };
        items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                query.is_empty()
                    || item.title.to_lowercase().contains(&query)
                    || item.key.to_lowercase().contains(&query)
            })
            .map(|(index, _)| index)
            .collect()
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
        let header = HEADER + if self.searchable { 24.0 } else { 0.0 };
        const BUTTONS: f32 = 60.0;
        const ROW: f32 = 28.0;
        let Some(frame) = self.last_frame.get().filter(|frame| frame.contains(pos)) else {
            return EventResult::NotHandled;
        };
        let x = pos.x - frame.x;
        let y = pos.y - frame.y;
        let half = ((frame.w - BUTTONS) * 0.5).max(40.0);
        let row = (y >= header).then(|| ((y - header) / ROW) as usize);
        if x < half {
            return self.toggle_visible_row(TransferPane::Source, row);
        }
        if x > half + BUTTONS {
            return self.toggle_visible_row(TransferPane::Target, row);
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

    fn toggle_visible_row(&mut self, pane: TransferPane, row: Option<usize>) -> EventResult {
        let Some(visible_row) = row else {
            return EventResult::NotHandled;
        };
        let Some(raw_row) = self.visible_indices(pane).get(visible_row).copied() else {
            return EventResult::NotHandled;
        };
        self.toggle_row(pane, Some(raw_row))
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
        if let Some(callback) = self.change_callback.as_ref() {
            let direction = match from {
                TransferPane::Source => MoveDirection::LeftToRight,
                TransferPane::Target => MoveDirection::RightToLeft,
            };
            callback(&self.source, &self.target, direction);
        }
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

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if self.text.is_empty() { return; }
        let color = self.effective_color();
        let columns = (frame.w.max(0.0) / self.gap_x).ceil() as usize + 2;
        let rows = (frame.h.max(0.0) / self.gap_y).ceil() as usize + 2;

        ctx.push_clip(frame);
        for gy in 0..rows {
            for gx in 0..columns {
                self.paint_rotated_text(ctx, self.tile_position(frame, gx, gy), color);
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
        Point::new(
            frame.x + gx as f32 * self.gap_x + self.x_offset,
            frame.y + gy as f32 * self.gap_y + self.y_offset,
        )
    }

    fn paint_rotated_text(&self, ctx: &mut PaintContext, origin: Point, color: Color) {
        let angle = self.rotate.to_radians();
        let (sin_a, cos_a) = angle.sin_cos();
        let line_height = self.font_size * 1.4;
        for (line_index, line) in self.text.split('\n').enumerate() {
            let normal_offset = line_index as f32 * line_height;
            let line_origin = Point::new(
                origin.x - sin_a * normal_offset,
                origin.y + cos_a * normal_offset,
            );
            let mut advance = 0.0;
            for character in line.chars() {
                let glyph = character.to_string();
                let position = Self::rotated_advance(line_origin, advance, sin_a, cos_a);
                ctx.draw_text(&glyph, position, color, self.font_size);
                advance += ctx.measure_text(&glyph, self.font_size).w;
            }
        }
    }

    fn rotated_advance(origin: Point, advance: f32, sin_a: f32, cos_a: f32) -> Point {
        Point::new(origin.x + cos_a * advance, origin.y + sin_a * advance)
    }

    fn effective_color(&self) -> Color {
        let alpha = (self.color.a as f32 * self.opacity)
            .round()
            .clamp(0.0, 255.0) as u8;
        self.color.with_alpha(alpha)
    }

    fn positive_or(value: f32, fallback: f32) -> f32 {
        if value.is_finite() && value >= 1.0 {
            value
        } else {
            fallback
        }
    }

    #[cfg(test)]
    pub(crate) fn effective_color_for_test(&self) -> Color {
        self.effective_color()
    }

    #[cfg(test)]
    pub(crate) fn rotated_advance_for_test(&self, origin: Point, advance: f32) -> Point {
        let (sin_a, cos_a) = self.rotate.to_radians().sin_cos();
        Self::rotated_advance(origin, advance, sin_a, cos_a)
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
