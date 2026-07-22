//! Breadcrumb widget — 面包屑导航路径。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Radius;
use crate::ui::core::widget::WidgetTree;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent,
};
use std::cell::Cell;
use std::collections::BTreeSet;

const BREADCRUMB_HEIGHT: f32 = 22.0;
const TITLE_FONT_SIZE: f32 = 13.0;
const TITLE_GLYPH_WIDTH: f32 = 7.5;
const SEPARATOR_FONT_SIZE: f32 = 12.0;
const SEPARATOR_GLYPH_WIDTH: f32 = 8.0;
const ICON_SIZE: f32 = 14.0;
const ICON_SLOT_WIDTH: f32 = 16.0;
const ICON_TEXT_GAP: f32 = 4.0;
const OVERFLOW_ROW_HEIGHT: f32 = 28.0;
const OVERFLOW_MIN_WIDTH: f32 = 112.0;
const OVERFLOW_HORIZONTAL_PADDING: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BreadcrumbSlot {
    Item(usize),
    Overflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BreadcrumbHit {
    Item(usize),
    OverflowTrigger,
    OverflowItem(usize),
}

/// 面包屑的一项。
#[derive(Debug, Clone, PartialEq)]
pub struct BreadcrumbItem {
    pub title: String,
    pub active: bool,
    pub icon: String,
}

impl BreadcrumbItem {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            active: false,
            icon: String::new(),
        }
    }
    pub fn active(mut self) -> Self {
        self.active = true;
        self
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }
}

component! {
    /// Breadcrumb — 导航路径指示器。
    pub struct Breadcrumb {
        items: Vec<BreadcrumbItem>,
        separator: String,
        max_items: usize,
        focused: bool,
        overflow_open: bool,
        overflow_highlighted: Option<usize>,
        layout_requested: Cell<bool>,
        pending_change: Cell<Option<usize>>,
    }

    tab_index => (&self) -> i32 { i32::from(!self.items.is_empty()) }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::compositor::PicturePolicy {
        crate::draw::compositor::PicturePolicy::Eligible
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                match self.hit_test(*pos) {
                    Some(BreadcrumbHit::Item(index)) => {
                        self.close_overflow();
                        self.select(index, true);
                        EventResult::Handled
                    }
                    Some(BreadcrumbHit::OverflowTrigger) => {
                        if self.overflow_open {
                            self.close_overflow();
                        } else {
                            self.open_overflow(true);
                        }
                        EventResult::Handled
                    }
                    Some(BreadcrumbHit::OverflowItem(index)) => {
                        self.close_overflow();
                        self.select(index, true);
                        EventResult::Handled
                    }
                    None if self.overflow_open => {
                        self.close_overflow();
                        EventResult::Handled
                    }
                    None => EventResult::NotHandled,
                }
            }
            SystemEvent::PointerMove { pos, .. } if self.overflow_open => {
                self.overflow_highlighted = self.overflow_item_at(*pos);
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                let was_open = self.overflow_open;
                self.close_overflow();
                if was_open {
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
                self.close_overflow();
                EventResult::Handled
            }
            SystemEvent::WindowBlur => {
                let was_focused = self.focused;
                self.focused = false;
                let was_open = self.overflow_open;
                self.close_overflow();
                if was_focused || was_open {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::KeyDown { key, .. } if !self.items.is_empty() => {
                self.handle_key(*key)
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        let index = self.pending_change.take()?;
        self.items
            .get(index)
            .map(|item| SemanticEvent::change(id, item.title.clone()))
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    wants_continuous_pointer_move => (&self) -> bool { self.overflow_open }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let text_secondary = ctx.tokens().color_text_secondary();
        let text_color = ctx.tokens().color_text();
        let primary = ctx.tokens().color_primary();
        let slots = self.line_layout();
        for (position, (slot, local_rect)) in slots.iter().enumerate() {
            let rect = Self::absolute_rect(*local_rect, frame);
            match slot {
                BreadcrumbSlot::Item(index) => {
                    let item = &self.items[*index];
                    let color = if item.active { text_color } else { text_secondary };
                    self.paint_item(ctx, item, rect, color, 0.0);
                }
                BreadcrumbSlot::Overflow => {
                    if self.overflow_open {
                        ctx.fill_rect(
                            rect,
                            ctx.tokens().color_fill_tertiary(),
                            Some(Radius::uniform(ctx.tokens().border_radius_sm())),
                        );
                    }
                    ctx.text_center(
                        "...",
                        rect,
                        if self.overflow_open { primary } else { text_secondary },
                        TITLE_FONT_SIZE,
                    );
                }
            }
            if position + 1 < slots.len() {
                let separator_rect = Rect::new(
                    rect.x + rect.w,
                    frame.y,
                    Self::separator_width(&self.separator),
                    BREADCRUMB_HEIGHT,
                );
                ctx.text_center(
                    &self.separator,
                    separator_rect,
                    text_secondary,
                    SEPARATOR_FONT_SIZE,
                );
            }
        }

        if self.overflow_open {
            self.paint_overflow(frame, ctx);
        }

        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                Rect::new(frame.x, frame.y, self.line_width(), BREADCRUMB_HEIGHT),
                primary,
                1.5,
                Some(Radius::uniform(ctx.tokens().border_radius_sm())),
            );
        }
    }
}

impl Default for Breadcrumb {
    fn default() -> Self {
        Self::new()
    }
}

impl Breadcrumb {
    fn intrinsic_size(&self) -> Size {
        if self.items.is_empty() {
            return Size::zero();
        }
        let line_width = self.line_width();
        if self.overflow_open {
            if let Some((menu, _)) = self.overflow_layout() {
                return Size::new(line_width.max(menu.x + menu.w), menu.y + menu.h);
            }
        }
        Size::new(line_width, BREADCRUMB_HEIGHT)
    }

    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            separator: "/".to_string(),
            max_items: 0,
            focused: false,
            overflow_open: false,
            overflow_highlighted: None,
            layout_requested: Cell::new(false),
            pending_change: Cell::new(None),
        }
    }
    pub fn item(mut self, item: BreadcrumbItem) -> Self {
        self.items.push(item);
        self.normalize_active();
        self
    }
    pub fn items(mut self, items: Vec<BreadcrumbItem>) -> Self {
        self.items = items;
        self.normalize_active();
        self
    }
    pub fn separator(mut self, s: impl Into<String>) -> Self {
        self.separator = s.into();
        self
    }

    pub fn max_items(mut self, maximum: usize) -> Self {
        self.max_items = maximum;
        self
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let active_title = self.active_title().map(str::to_owned);
        self.items = next.items;
        self.normalize_active();
        if let Some(active_title) = active_title {
            if let Some(index) = self
                .items
                .iter()
                .position(|item| item.title == active_title)
            {
                self.set_active(index);
            }
        }
        self.separator = next.separator;
        self.max_items = next.max_items;
        self.close_overflow();
        self.pending_change.set(None);
    }

    pub fn active_index(&self) -> usize {
        self.items.iter().position(|item| item.active).unwrap_or(0)
    }

    pub fn active_title(&self) -> Option<&str> {
        self.items
            .get(self.active_index())
            .map(|item| item.title.as_str())
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Breadcrumb {
            items: self.items.clone(),
            separator: self.separator.clone(),
        }
    }

    fn normalize_active(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let active = self.items.iter().rposition(|item| item.active).unwrap_or(0);
        self.set_active(active);
    }

    fn set_active(&mut self, index: usize) {
        for (item_index, item) in self.items.iter_mut().enumerate() {
            item.active = item_index == index;
        }
    }

    fn select(&mut self, index: usize, activate_unchanged: bool) {
        if self.items.is_empty() {
            return;
        }
        let index = index.min(self.items.len() - 1);
        let changed = index != self.active_index();
        self.set_active(index);
        if changed || activate_unchanged {
            self.pending_change.set(Some(index));
        }
    }

    fn move_active(&mut self, forward: bool) {
        let current = self.active_index();
        let next = if forward {
            (current + 1).min(self.items.len() - 1)
        } else {
            current.saturating_sub(1)
        };
        self.select(next, false);
    }

    fn handle_key(&mut self, key: KeyCode) -> EventResult {
        if self.overflow_open {
            return match key {
                KeyCode::Down => {
                    self.move_overflow_highlight(true);
                    EventResult::Handled
                }
                KeyCode::Up => {
                    self.move_overflow_highlight(false);
                    EventResult::Handled
                }
                KeyCode::Home => {
                    self.overflow_highlighted = self.hidden_item_indices().first().copied();
                    EventResult::Handled
                }
                KeyCode::End => {
                    self.overflow_highlighted = self.hidden_item_indices().last().copied();
                    EventResult::Handled
                }
                KeyCode::Enter | KeyCode::Space => {
                    if let Some(index) = self.overflow_highlighted {
                        self.close_overflow();
                        self.select(index, true);
                    }
                    EventResult::Handled
                }
                KeyCode::Escape => {
                    self.close_overflow();
                    EventResult::Handled
                }
                KeyCode::Left => {
                    self.close_overflow();
                    self.move_active(false);
                    EventResult::Handled
                }
                KeyCode::Right => {
                    self.close_overflow();
                    self.move_active(true);
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            };
        }

        match key {
            KeyCode::Down => {
                if self.open_overflow(true) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            KeyCode::Up => {
                if self.open_overflow(false) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            KeyCode::Left => {
                self.move_active(false);
                EventResult::Handled
            }
            KeyCode::Right => {
                self.move_active(true);
                EventResult::Handled
            }
            KeyCode::Home => {
                self.select(0, false);
                EventResult::Handled
            }
            KeyCode::End => {
                self.select(self.items.len() - 1, false);
                EventResult::Handled
            }
            KeyCode::Enter | KeyCode::Space => {
                self.pending_change.set(Some(self.active_index()));
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    fn open_overflow(&mut self, forward: bool) -> bool {
        let hidden = self.hidden_item_indices();
        if hidden.is_empty() {
            self.close_overflow();
            return false;
        }
        if !self.overflow_open {
            self.layout_requested.set(true);
        }
        self.overflow_open = true;
        self.overflow_highlighted = if forward {
            hidden.first().copied()
        } else {
            hidden.last().copied()
        };
        true
    }

    fn close_overflow(&mut self) {
        if self.overflow_open {
            self.layout_requested.set(true);
        }
        self.overflow_open = false;
        self.overflow_highlighted = None;
    }

    fn move_overflow_highlight(&mut self, forward: bool) {
        let hidden = self.hidden_item_indices();
        if hidden.is_empty() {
            self.close_overflow();
            return;
        }
        let position = self
            .overflow_highlighted
            .and_then(|current| hidden.iter().position(|index| *index == current));
        let next = match (position, forward) {
            (Some(position), true) => (position + 1) % hidden.len(),
            (Some(position), false) => (position + hidden.len() - 1) % hidden.len(),
            (None, true) => 0,
            (None, false) => hidden.len() - 1,
        };
        self.overflow_highlighted = Some(hidden[next]);
    }

    fn hit_test(&self, pos: Point) -> Option<BreadcrumbHit> {
        if self.overflow_open {
            if let Some(index) = self.overflow_item_at(pos) {
                return Some(BreadcrumbHit::OverflowItem(index));
            }
        }
        if pos.y < 0.0 || pos.y >= BREADCRUMB_HEIGHT || pos.x < 0.0 {
            return None;
        }
        self.line_layout().into_iter().find_map(|(slot, rect)| {
            rect.contains(pos).then_some(match slot {
                BreadcrumbSlot::Item(index) => BreadcrumbHit::Item(index),
                BreadcrumbSlot::Overflow => BreadcrumbHit::OverflowTrigger,
            })
        })
    }

    fn overflow_item_at(&self, pos: Point) -> Option<usize> {
        let (_, rows) = self.overflow_layout()?;
        rows.into_iter()
            .find_map(|(index, rect)| rect.contains(pos).then_some(index))
    }

    fn line_layout(&self) -> Vec<(BreadcrumbSlot, Rect)> {
        let slots = self.visible_slots();
        let separator_width = Self::separator_width(&self.separator);
        let mut x = 0.0;
        let mut layout = Vec::with_capacity(slots.len());
        for (position, slot) in slots.iter().copied().enumerate() {
            let width = match slot {
                BreadcrumbSlot::Item(index) => Self::item_width(&self.items[index]),
                BreadcrumbSlot::Overflow => Self::text_width("...", TITLE_GLYPH_WIDTH),
            };
            layout.push((slot, Rect::new(x, 0.0, width, BREADCRUMB_HEIGHT)));
            x += width;
            if position + 1 < slots.len() {
                x += separator_width;
            }
        }
        layout
    }

    fn line_width(&self) -> f32 {
        self.line_layout()
            .last()
            .map(|(_, rect)| rect.x + rect.w)
            .unwrap_or(0.0)
    }

    fn overflow_layout(&self) -> Option<(Rect, Vec<(usize, Rect)>)> {
        let trigger = self
            .line_layout()
            .into_iter()
            .find_map(|(slot, rect)| (slot == BreadcrumbSlot::Overflow).then_some(rect))?;
        let hidden = self.hidden_item_indices();
        if hidden.is_empty() {
            return None;
        }
        let width = hidden
            .iter()
            .map(|index| Self::item_width(&self.items[*index]) + OVERFLOW_HORIZONTAL_PADDING * 2.0)
            .fold(OVERFLOW_MIN_WIDTH, f32::max);
        let menu = Rect::new(
            trigger.x,
            BREADCRUMB_HEIGHT,
            width,
            hidden.len() as f32 * OVERFLOW_ROW_HEIGHT,
        );
        let rows = hidden
            .into_iter()
            .enumerate()
            .map(|(row, index)| {
                (
                    index,
                    Rect::new(
                        menu.x,
                        menu.y + row as f32 * OVERFLOW_ROW_HEIGHT,
                        menu.w,
                        OVERFLOW_ROW_HEIGHT,
                    ),
                )
            })
            .collect();
        Some((menu, rows))
    }

    fn visible_slots(&self) -> Vec<BreadcrumbSlot> {
        let visible = self.visible_item_indices();
        if visible.len() == self.items.len() {
            return visible.into_iter().map(BreadcrumbSlot::Item).collect();
        }
        let mut inserted_overflow = false;
        let mut previous = None;
        let mut slots = Vec::with_capacity(visible.len() + 1);
        for index in visible {
            if !inserted_overflow && previous.is_some_and(|prior| prior + 1 < index) {
                slots.push(BreadcrumbSlot::Overflow);
                inserted_overflow = true;
            }
            slots.push(BreadcrumbSlot::Item(index));
            previous = Some(index);
        }
        slots
    }

    fn visible_item_indices(&self) -> Vec<usize> {
        if self.items.is_empty() || self.max_items == 0 {
            return (0..self.items.len()).collect();
        }

        let target = self.max_items.max(3).min(self.items.len());
        if self.items.len() <= target {
            return (0..self.items.len()).collect();
        }

        let active = self.active_index().min(self.items.len() - 1);
        let mut visible = BTreeSet::new();
        visible.insert(0);
        visible.insert(active);
        visible.insert(self.items.len() - 1);

        let mut distance = 1;
        while visible.len() < target {
            if let Some(index) = active.checked_sub(distance) {
                visible.insert(index);
                if visible.len() == target {
                    break;
                }
            }
            if let Some(index) = active
                .checked_add(distance)
                .filter(|index| *index < self.items.len())
            {
                visible.insert(index);
            }
            distance += 1;
        }
        visible.into_iter().collect()
    }

    fn hidden_item_indices(&self) -> Vec<usize> {
        let visible = self.visible_item_indices();
        (0..self.items.len())
            .filter(|index| visible.binary_search(index).is_err())
            .collect()
    }

    fn paint_item(
        &self,
        ctx: &mut PaintContext,
        item: &BreadcrumbItem,
        rect: Rect,
        color: crate::draw::Color,
        horizontal_padding: f32,
    ) {
        let mut title_x = rect.x + horizontal_padding;
        if !item.icon.is_empty() {
            let icon_rect = Rect::new(title_x, rect.y, ICON_SLOT_WIDTH, rect.h);
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx, &item.icon, icon_rect, color, ICON_SIZE,
            );
            title_x += ICON_SLOT_WIDTH + ICON_TEXT_GAP;
        }
        let title_rect = Rect::new(
            title_x,
            rect.y,
            Self::text_width(&item.title, TITLE_GLYPH_WIDTH),
            rect.h,
        );
        ctx.text_center(&item.title, title_rect, color, TITLE_FONT_SIZE);
    }

    fn paint_overflow(&self, frame: Rect, ctx: &mut PaintContext) {
        let Some((local_menu, rows)) = self.overflow_layout() else {
            return;
        };
        let menu = Self::absolute_rect(local_menu, frame);
        let radius = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        let shadow = ctx.tokens().box_shadow_secondary();
        ctx.draw_box_shadow(
            menu,
            shadow.layer_1.2,
            shadow.layer_1.0,
            shadow.layer_1.1,
            shadow.layer_1.3,
            radius,
        );
        ctx.fill_rect(menu, ctx.tokens().color_bg_elevated(), radius);
        ctx.stroke_rect(menu, ctx.tokens().color_border(), 1.0, radius);

        for (index, local_row) in rows {
            let row = Self::absolute_rect(local_row, frame);
            if self.overflow_highlighted == Some(index) {
                ctx.fill_rect(row, ctx.tokens().color_fill_tertiary(), radius);
            }
            self.paint_item(
                ctx,
                &self.items[index],
                row,
                ctx.tokens().color_text(),
                OVERFLOW_HORIZONTAL_PADDING,
            );
        }
    }

    fn item_width(item: &BreadcrumbItem) -> f32 {
        Self::text_width(&item.title, TITLE_GLYPH_WIDTH)
            + if item.icon.is_empty() {
                0.0
            } else {
                ICON_SLOT_WIDTH + ICON_TEXT_GAP
            }
    }

    fn separator_width(separator: &str) -> f32 {
        Self::text_width(separator, SEPARATOR_GLYPH_WIDTH)
    }

    fn absolute_rect(rect: Rect, frame: Rect) -> Rect {
        Rect::new(frame.x + rect.x, frame.y + rect.y, rect.w, rect.h)
    }

    fn text_width(text: &str, glyph_width: f32) -> f32 {
        text.chars().count() as f32 * glyph_width + 8.0
    }

    #[cfg(test)]
    pub(crate) fn visible_slots_for_test(&self) -> Vec<Option<usize>> {
        self.visible_slots()
            .into_iter()
            .map(|slot| match slot {
                BreadcrumbSlot::Item(index) => Some(index),
                BreadcrumbSlot::Overflow => None,
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn overflow_trigger_rect_for_test(&self) -> Option<Rect> {
        self.line_layout()
            .into_iter()
            .find_map(|(slot, rect)| (slot == BreadcrumbSlot::Overflow).then_some(rect))
    }

    #[cfg(test)]
    pub(crate) fn overflow_rows_for_test(&self) -> Vec<(usize, Rect)> {
        self.overflow_layout()
            .map(|(_, rows)| rows)
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn overflow_state_for_test(&self) -> (bool, Option<usize>) {
        (self.overflow_open, self.overflow_highlighted)
    }

    #[cfg(test)]
    pub(crate) fn item_content_rects_for_test(
        &self,
        index: usize,
    ) -> Option<(Rect, Option<Rect>, Rect)> {
        let item = self.items.get(index)?;
        let (_, item_rect) = self
            .line_layout()
            .into_iter()
            .find(|(slot, _)| *slot == BreadcrumbSlot::Item(index))?;
        let icon = (!item.icon.is_empty()).then_some(Rect::new(
            item_rect.x,
            item_rect.y,
            ICON_SLOT_WIDTH,
            item_rect.h,
        ));
        let title_x = item_rect.x
            + if icon.is_some() {
                ICON_SLOT_WIDTH + ICON_TEXT_GAP
            } else {
                0.0
            };
        let title = Rect::new(
            title_x,
            item_rect.y,
            Self::text_width(&item.title, TITLE_GLYPH_WIDTH),
            item_rect.h,
        );
        Some((item_rect, icon, title))
    }
}
