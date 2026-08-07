use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::{Color, Radius};
use crate::ui::component::paint_context::PaintContext;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields,
    SnapshotTransferItem, SystemEvent, WidgetTree,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

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

    // 测试目标保留传输组件 frame 注入入口，供交互几何测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
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
