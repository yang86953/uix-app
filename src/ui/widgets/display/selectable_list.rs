//! Selectable list widget.

use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::api::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::foundation::virtual_scroll::VirtualListScroll;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent,
    WidgetTree,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectableListAction {
    Header,
    Row(usize),
}

const DEFAULT_WIDTH: f32 = 220.0;
const DEFAULT_HEIGHT: f32 = 500.0;
const HEADER_HEIGHT: f32 = 48.0;
const HEADER_INSET: f32 = 8.0;
const HEADER_BUTTON_HEIGHT: f32 = 32.0;
const FOOTER_HEIGHT: f32 = 28.0;
const ROW_HORIZONTAL_INSET: f32 = 8.0;

#[derive(Debug, Clone, Copy)]
struct SelectableListGeometry {
    frame: Rect,
    header_button: Option<Rect>,
    body: Rect,
    footer: Option<Rect>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectableItem {
    pub id: String,
    pub text: String,
    pub icon: Option<String>,
}

impl Default for SelectableList {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectableList {
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::SelectableList {
            items: self.items.clone(),
            active_index: self.active_index,
            header_button_text: self.header_button_text.clone(),
            footer_text: self.footer_text.clone(),
            item_height: self.item_height,
        }
    }

    pub(crate) fn item_stride(&self) -> f32 {
        self.item_height + 2.0
    }

    pub(crate) fn list_body_viewport_height(&self) -> f32 {
        let frame = self
            .last_frame
            .get()
            .unwrap_or_else(|| Rect::new(0.0, 0.0, DEFAULT_WIDTH, DEFAULT_HEIGHT));
        self.geometry(frame).body.h
    }

    #[cfg(test)]
    pub(crate) fn row_index_at_y(&self, pos_y: f32) -> Option<usize> {
        let frame = self
            .last_frame
            .get()
            .unwrap_or_else(|| Rect::new(0.0, 0.0, DEFAULT_WIDTH, DEFAULT_HEIGHT));
        let geometry = self.local_geometry(frame);
        self.row_index_in_geometry(Point::new(geometry.body.x, pos_y), geometry)
    }

    fn row_index_in_geometry(
        &self,
        point: Point,
        geometry: SelectableListGeometry,
    ) -> Option<usize> {
        if !geometry.body.contains(point) {
            return None;
        }
        let local_y = point.y - geometry.body.y + self.body_scroll.scroll_offset();
        if local_y < 0.0 {
            return None;
        }
        let stride = self.item_stride();
        let row = (local_y / stride) as usize;
        let y_in_row = local_y - row as f32 * stride;
        if row < self.items.len() && y_in_row < self.item_height {
            Some(row)
        } else {
            None
        }
    }

    fn push_scroll_delta(&self, dx: f32, dy: f32) {
        if dx.abs() <= 0.01 && dy.abs() <= 0.01 {
            return;
        }
        let current = self.scroll_delta_strip.get();
        self.scroll_delta_strip
            .set((current.0 + dx, current.1 + dy));
    }

    fn action_at_point(&self, point: Point) -> Option<SelectableListAction> {
        let frame = self
            .last_frame
            .get()
            .unwrap_or_else(|| Rect::new(0.0, 0.0, DEFAULT_WIDTH, DEFAULT_HEIGHT));
        let geometry = self.local_geometry(frame);
        if geometry
            .header_button
            .is_some_and(|button| button.contains(point))
        {
            Some(SelectableListAction::Header)
        } else {
            self.row_index_in_geometry(point, geometry)
                .map(SelectableListAction::Row)
        }
    }

    fn select(&mut self, index: usize, activate_unchanged: bool) {
        if self.items.is_empty() {
            return;
        }
        let index = index.min(self.items.len() - 1);
        let changed = index != self.active_index;
        self.active_index = index;
        if changed || activate_unchanged {
            self.pending_action
                .set(Some(SelectableListAction::Row(index)));
        }
        if changed {
            self.ensure_active_visible();
        }
    }

    fn move_active(&mut self, forward: bool) {
        if self.items.is_empty() {
            return;
        }
        let next = if forward {
            (self.active_index + 1).min(self.items.len() - 1)
        } else {
            self.active_index.saturating_sub(1)
        };
        self.select(next, false);
    }

    fn ensure_active_visible(&mut self) {
        let stride = self.item_stride();
        let viewport_height = self.list_body_viewport_height();
        let old_offset = self.body_scroll.scroll_offset();
        let row_top = self.active_index as f32 * stride;
        let row_bottom = row_top + stride;
        let new_offset = if row_top < old_offset {
            row_top
        } else if row_bottom > old_offset + viewport_height {
            row_bottom - viewport_height
        } else {
            old_offset
        };
        self.body_scroll.set_scroll_offset(new_offset);
        self.body_scroll
            .clamp_to_content(self.items.len(), stride, viewport_height);
        self.push_scroll_delta(0.0, self.body_scroll.scroll_offset() - old_offset);
    }

    pub fn items(mut self, items: Vec<SelectableItem>) -> Self {
        self.items = items;
        self.active_index = self.active_index.min(self.items.len().saturating_sub(1));
        self
    }

    pub fn active(mut self, index: usize) -> Self {
        self.active_index = index.min(self.items.len().saturating_sub(1));
        self
    }

    pub fn header_button(mut self, text: impl Into<String>) -> Self {
        self.header_button_text = text.into();
        self
    }

    pub fn footer(mut self, text: impl Into<String>) -> Self {
        self.footer_text = text.into();
        self
    }

    pub fn row_height(mut self, height: f32) -> Self {
        if height.is_finite() {
            self.item_height = height.max(20.0);
        }
        self
    }

    pub fn selected_id(&self) -> Option<&str> {
        self.items
            .get(self.active_index)
            .map(|item| item.id.as_str())
    }

    pub fn selected_text(&self) -> Option<&str> {
        self.items
            .get(self.active_index)
            .map(|item| item.text.as_str())
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let selected_id = self.selected_id().map(str::to_owned);
        self.items = next.items;
        self.header_button_text = next.header_button_text;
        self.footer_text = next.footer_text;
        self.item_height = next.item_height;
        self.active_index = selected_id
            .as_deref()
            .and_then(|id| self.items.iter().position(|item| item.id == id))
            .unwrap_or_else(|| next.active_index.min(self.items.len().saturating_sub(1)));
        self.body_scroll.clamp_to_content(
            self.items.len(),
            self.item_stride(),
            self.list_body_viewport_height(),
        );
        self.hovered_index.set(
            self.hovered_index
                .get()
                .filter(|idx| *idx < self.items.len()),
        );
        self.pressed_action.set(None);
    }

    fn geometry(&self, frame: Rect) -> SelectableListGeometry {
        let frame = Self::normalized_frame(frame);
        let header_height = if self.header_button_text.is_empty() {
            0.0
        } else {
            HEADER_HEIGHT.min(frame.h)
        };
        let remaining = (frame.h - header_height).max(0.0);
        let footer_height = if self.footer_text.is_empty() {
            0.0
        } else {
            FOOTER_HEIGHT.min(remaining)
        };
        let body_height = (remaining - footer_height).max(0.0);
        let header_button = (!self.header_button_text.is_empty()).then(|| {
            let horizontal_inset = HEADER_INSET.min(frame.w * 0.5);
            let vertical_inset = HEADER_INSET.min(header_height * 0.5);
            Rect::new(
                frame.x + horizontal_inset,
                frame.y + vertical_inset,
                (frame.w - horizontal_inset * 2.0).max(0.0),
                HEADER_BUTTON_HEIGHT.min((header_height - vertical_inset * 2.0).max(0.0)),
            )
        });
        let body = Rect::new(frame.x, frame.y + header_height, frame.w, body_height);
        let footer = (!self.footer_text.is_empty() && footer_height > 0.0)
            .then(|| Rect::new(frame.x, body.y + body.h, frame.w, footer_height));
        SelectableListGeometry {
            frame,
            header_button,
            body,
            footer,
        }
    }

    fn local_geometry(&self, frame: Rect) -> SelectableListGeometry {
        self.geometry(Rect::new(0.0, 0.0, frame.w, frame.h))
    }

    fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(
            frame.x,
            frame.y,
            if frame.w.is_finite() {
                frame.w.max(0.0)
            } else {
                0.0
            },
            if frame.h.is_finite() {
                frame.h.max(0.0)
            } else {
                0.0
            },
        )
    }

    fn paint_single_line(
        ctx: &mut PaintContext<'_>,
        value: &str,
        frame: Rect,
        color: Color,
        font_size: f32,
    ) {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let Some(value) = Self::elide_single_line(ctx, value, font_size, frame.w) else {
            return;
        };
        ctx.push_clip(frame);
        ctx.draw_text_in_frame(&value, frame, color, font_size);
        ctx.pop_clip();
    }

    fn elide_single_line(
        ctx: &mut PaintContext<'_>,
        value: &str,
        font_size: f32,
        max_width: f32,
    ) -> Option<String> {
        if !max_width.is_finite() || max_width <= 0.0 {
            return None;
        }
        let value = value.replace(['\r', '\n'], " ");
        if Self::text_width(ctx, &value, font_size) <= max_width {
            return Some(value);
        }
        const ELLIPSIS: &str = "…";
        if Self::text_width(ctx, ELLIPSIS, font_size) > max_width {
            return None;
        }
        let mut visible = String::new();
        for ch in value.chars() {
            visible.push(ch);
            visible.push_str(ELLIPSIS);
            let fits = Self::text_width(ctx, &visible, font_size) <= max_width;
            visible.pop();
            if !fits {
                visible.pop();
                break;
            }
        }
        visible.push_str(ELLIPSIS);
        Some(visible)
    }

    fn text_width(ctx: &mut PaintContext<'_>, value: &str, font_size: f32) -> f32 {
        ctx.measure_text(value, font_size).w.max(
            crate::draw::resources::font::text_backend::estimate_text_metrics(
                value,
                f32::INFINITY,
                font_size,
            )
            .max_line_width,
        )
    }
}

impl SelectableItem {
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            icon: None,
        }
    }

    pub fn icon(mut self, icon: &str) -> Self {
        self.icon = Some(icon.to_string());
        self
    }
}

component! {
    /// A vertical list with selectable rows.
    pub struct SelectableList {
        pub items: Vec<SelectableItem>,
        pub active_index: usize,
        pub header_button_text: String,
        pub footer_text: String,
        pub item_height: f32,

        hovered_index: Cell<Option<usize>>,
        hovered_header: Cell<bool>,
        pressed_action: Cell<Option<SelectableListAction>>,
        focused: bool,
        pub(crate) body_scroll: VirtualListScroll,
        scroll_delta_strip: Cell<(f32, f32)>,
        pub(crate) last_frame: Cell<Option<Rect>>,
        pending_action: Cell<Option<SelectableListAction>>,
    }

    @new -> Self {
        Self {
            items: Vec::new(),
            active_index: 0,
            header_button_text: String::new(),
            footer_text: String::new(),
            item_height: 36.0,
            hovered_index: Cell::new(None),
            hovered_header: Cell::new(false),
            pressed_action: Cell::new(None),
            focused: false,
            body_scroll: VirtualListScroll::new(),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
            last_frame: Cell::new(None),
            pending_action: Cell::new(None),
        }
    }

    tab_index => (&self) -> i32 {
        i32::from(!self.items.is_empty() || !self.header_button_text.is_empty())
    }

    measure => (&self, constraints: Constraints) -> Size {
        let mut h = 0.0;
        if !self.header_button_text.is_empty() {
            h += 48.0;
        }
        h += self.items.len() as f32 * self.item_stride();
        if !self.footer_text.is_empty() {
            h += 28.0;
        }
        constraints.clamp(Size::new(220.0, h.max(100.0)))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(action) = self.action_at_point(*pos) {
                    self.hovered_header
                        .set(action == SelectableListAction::Header);
                    self.hovered_index.set(match action {
                        SelectableListAction::Row(index) => Some(index),
                        SelectableListAction::Header => None,
                    });
                    self.pressed_action.set(Some(action));
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }

            SystemEvent::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let pressed = self.pressed_action.replace(None);
                if let Some(action) = pressed {
                    let released = self.action_at_point(*pos);
                    self.hovered_header
                        .set(released == Some(SelectableListAction::Header));
                    self.hovered_index.set(match released {
                        Some(SelectableListAction::Row(index)) => Some(index),
                        _ => None,
                    });
                    if released == Some(action) {
                        match action {
                            SelectableListAction::Header => {
                                self.pending_action.set(Some(SelectableListAction::Header));
                            }
                            SelectableListAction::Row(index) => self.select(index, true),
                        }
                    }
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }

            SystemEvent::PointerMove { pos, .. } => {
                let old_hover = self.hovered_index.get();
                let old_btn = self.hovered_header.get();

                let action = self.action_at_point(*pos);
                let new_btn = action == Some(SelectableListAction::Header);
                let new_hover = match action {
                    Some(SelectableListAction::Row(index)) => Some(index),
                    _ => None,
                };

                self.hovered_index.set(new_hover);
                self.hovered_header.set(new_btn);
                if old_hover != new_hover || old_btn != new_btn {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }

            SystemEvent::Wheel { pos, delta } => {
                let frame = self
                    .last_frame
                    .get()
                    .unwrap_or_else(|| Rect::new(0.0, 0.0, DEFAULT_WIDTH, DEFAULT_HEIGHT));
                if !self.local_geometry(frame).body.contains(*pos) {
                    return EventResult::NotHandled;
                }
                let viewport_h = self.list_body_viewport_height();
                let dy = self.body_scroll.scroll_by_wheel(
                    delta.y,
                    self.items.len(),
                    self.item_stride(),
                    viewport_h,
                );
                if dy.abs() > 0.01 {
                    self.push_scroll_delta(0.0, dy);
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }

            SystemEvent::PointerLeave => {
                let changed = self.hovered_index.replace(None).is_some()
                    | self.hovered_header.replace(false)
                    | self.pressed_action.replace(None).is_some();
                if changed {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }

            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }

            SystemEvent::FocusOut => {
                self.focused = false;
                self.pressed_action.set(None);
                EventResult::Handled
            }

            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Up if !self.items.is_empty() => {
                    self.move_active(false);
                    EventResult::Handled
                }
                KeyCode::Down if !self.items.is_empty() => {
                    self.move_active(true);
                    EventResult::Handled
                }
                KeyCode::Home if !self.items.is_empty() => {
                    self.select(0, false);
                    EventResult::Handled
                }
                KeyCode::End if !self.items.is_empty() => {
                    self.select(self.items.len() - 1, false);
                    EventResult::Handled
                }
                KeyCode::Enter | KeyCode::Space if !self.items.is_empty() => {
                    self.select(self.active_index, true);
                    EventResult::Handled
                }
                KeyCode::Enter | KeyCode::Space if !self.header_button_text.is_empty() => {
                    self.pending_action.set(Some(SelectableListAction::Header));
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },

            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        match self.pending_action.take()? {
            SelectableListAction::Header => {
                Some(SemanticEvent::submit(id, self.header_button_text.clone()))
            }
            SelectableListAction::Row(index) => self
                .items
                .get(index)
                .map(|item| SemanticEvent::change(id, item.id.clone())),
        }
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

    scroll_composite_viewport => (&self, frame: Rect) -> Option<Rect> {
        let body = self.geometry(frame).body;
        (body.w > 0.0 && body.h > 0.0).then_some(body)
    }

    viewport_scroll_offset => (&self) -> Option<(f32, f32)> {
        Some((0.0, self.body_scroll.scroll_offset()))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let geometry = self.geometry(frame);
        let frame = geometry.frame;
        self.last_frame.set(Some(frame));
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border_secondary();
        let fill = ctx.tokens().color_fill_secondary();
        let fill_hover = ctx.tokens().color_fill();
        let t_sec = ctx.tokens().color_text_secondary();
        let t_ter = ctx.tokens().color_text_tertiary();
        let t_pri = ctx.tokens().color_primary();

        ctx.push_clip(frame);
        ctx.fill_rect(frame, bg, None);
        if frame.w >= 1.0 {
            ctx.fill_rect(
                Rect::new(frame.x + frame.w - 1.0, frame.y, 1.0, frame.h),
                border,
                None,
            );
        }

        if let Some(btn_frame) = geometry.header_button {
            let pressed = self.pressed_action.get() == Some(SelectableListAction::Header)
                && self.hovered_header.get();
            let btn_bg = if pressed {
                Color::from_rgba(55, 110, 255, 38)
            } else if self.hovered_header.get() {
                fill_hover
            } else {
                fill
            };
            ctx.fill_rect(btn_frame, btn_bg, Some(Radius::uniform(6.0)));
            let icon_size = 18.0_f32.min(btn_frame.h);
            let icon_frame = Rect::new(
                btn_frame.x + 8.0_f32.min(btn_frame.w * 0.25),
                btn_frame.y + (btn_frame.h - icon_size) * 0.5,
                icon_size,
                icon_size,
            );
            crate::ui::widgets::general::icon::Icon::paint_in_frame(
                ctx, "plus", icon_frame, t_sec, 14.0,
            );
            let text_frame = Rect::new(
                icon_frame.x + icon_frame.w + 4.0,
                btn_frame.y,
                (btn_frame.x + btn_frame.w - icon_frame.x - icon_frame.w - 10.0).max(0.0),
                btn_frame.h,
            );
            Self::paint_single_line(ctx, &self.header_button_text, text_frame, t_sec, 13.0);
            let separator_width = (frame.w - HEADER_INSET * 2.0).max(0.0);
            if separator_width > 0.0 && geometry.body.y > frame.y {
                ctx.fill_rect(
                    Rect::new(
                        frame.x + HEADER_INSET.min(frame.w * 0.5),
                        (geometry.body.y - 1.0).max(frame.y),
                        separator_width,
                        1.0,
                    ),
                    border,
                    None,
                );
            }
        }

        let list_clip = geometry.body;
        ctx.push_clip(list_clip);

        let stride = self.item_stride();
        let scroll_offset = self.body_scroll.scroll_offset();
        let (start, end) = self
            .body_scroll
            .scroll_range(self.items.len(), stride, list_clip.h);

        for i in start..end {
            let iy = list_clip.y + i as f32 * stride - scroll_offset;
            if iy + self.item_height < list_clip.y || iy > list_clip.y + list_clip.h {
                continue;
            }

            let is_active = i == self.active_index;
            let is_hover = self.hovered_index.get() == Some(i);
            let horizontal_inset = ROW_HORIZONTAL_INSET.min(frame.w * 0.5);
            let item_frame = Rect::new(
                frame.x + horizontal_inset,
                iy,
                (frame.w - horizontal_inset * 2.0).max(0.0),
                self.item_height.min(list_clip.h.max(0.0)),
            );
            let is_pressed = self.pressed_action.get() == Some(SelectableListAction::Row(i))
                && is_hover;

            if is_pressed {
                ctx.fill_rect(
                    item_frame,
                    Color::from_rgba(55, 110, 255, 45),
                    Some(Radius::uniform(6.0)),
                );
            } else if is_active {
                ctx.fill_rect(item_frame, Color::from_rgba(55, 110, 255, 25), Some(Radius::uniform(6.0)));
                ctx.fill_rect(
                    Rect::new(
                        item_frame.x,
                        item_frame.y + 6.0_f32.min(item_frame.h * 0.5),
                        3.0_f32.min(item_frame.w),
                        (item_frame.h - 12.0).max(0.0),
                    ),
                    t_pri,
                    Some(Radius::uniform(1.5)),
                );
            } else if is_hover {
                ctx.fill_rect(item_frame, fill_hover, Some(Radius::uniform(6.0)));
            }

            let icon = self.items[i].icon.as_deref().unwrap_or("");
            let icon_slot = if icon.is_empty() { 0.0 } else { 26.0_f32.min(item_frame.w) };
            let text_x = if icon.is_empty() {
                item_frame.x + 14.0_f32.min(item_frame.w * 0.25)
            } else {
                item_frame.x + icon_slot + 6.0_f32.min(item_frame.w * 0.1)
            };
            if !icon.is_empty() {
                let icon_size = 18.0_f32.min(item_frame.h);
                crate::ui::widgets::general::icon::Icon::paint_in_frame(
                    ctx,
                    icon,
                    Rect::new(
                        item_frame.x + 8.0_f32.min(item_frame.w * 0.2),
                        item_frame.y + (item_frame.h - icon_size) * 0.5,
                        icon_size,
                        icon_size,
                    ),
                    t_sec,
                    14.0,
                );
            }

            let color = if is_active { t_pri } else { t_sec };
            let text_frame = Rect::new(
                text_x,
                item_frame.y,
                (item_frame.x + item_frame.w - text_x - 10.0).max(0.0),
                item_frame.h,
            );
            Self::paint_single_line(ctx, &self.items[i].text, text_frame, color, 13.0);
        }

        ctx.pop_clip();

        if let Some(footer) = geometry.footer {
            let horizontal_inset = 12.0_f32.min(footer.w * 0.25);
            Self::paint_single_line(
                ctx,
                &self.footer_text,
                Rect::new(
                    footer.x + horizontal_inset,
                    footer.y,
                    (footer.w - horizontal_inset * 2.0).max(0.0),
                    footer.h,
                ),
                t_ter,
                11.0,
            );
        }
        if self.focused && tree.keyboard_focus_visible() {
            let inset = 1.0_f32.min(frame.w * 0.5).min(frame.h * 0.5);
            let focus_frame = Rect::new(
                frame.x + inset,
                frame.y + inset,
                (frame.w - inset * 2.0).max(0.0),
                (frame.h - inset * 2.0).max(0.0),
            );
            ctx.stroke_rect(
                focus_frame,
                t_pri,
                1.5,
                Some(Radius::uniform(ctx.tokens().border_radius_sm())),
            );
        }
        ctx.pop_clip();
    }
}
