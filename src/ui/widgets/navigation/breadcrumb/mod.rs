//! Breadcrumb widget — 面包屑导航路径。

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Radius;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent, WidgetId,
};
use crate::widget;
use std::cell::Cell;
use std::collections::BTreeSet;

mod presentation;

use presentation::*;

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
    /// 向用户展示的路径标题。
    pub title: String,
    // 保存用于路由身份与 Change 载荷的稳定链接。
    /// 用作路由身份和选择事件载荷的稳定链接。
    pub link: String,
    /// 指示此条目是否代表当前路径位置。
    pub active: bool,
    /// 标题前显示的图标名称；空字符串表示不显示图标。
    pub icon: String,
}

impl BreadcrumbItem {
    /// 创建没有链接、图标且尚未激活的路径条目。
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            // 旧调用方缺省保持标题载荷兼容语义。
            link: String::new(),
            active: false,
            icon: String::new(),
        }
    }
    /// 将此条目标记为当前路径位置。
    pub fn active(mut self) -> Self {
        self.active = true;
        self
    }

    // 设置当前条目的稳定导航链接。
    /// 设置与展示标题分离的稳定导航链接。
    pub fn link(mut self, link: impl Into<String>) -> Self {
        // 保存拥有型链接，避免借用越过组件生命周期。
        self.link = link.into();
        // 返回配置完成的条目。
        self
    }

    /// 设置标题前显示的图标名称。
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }
}

widget! {
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
        // 同目录 UIX 生成的唯一静态视觉表。
        #[snapshot(skip)]
        visual: &'static BreadcrumbVisual,
    }

    tab_index => (&self) -> i32 { i32::from(!self.items.is_empty()) }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
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

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        let index = self.pending_change.take()?;
        self.items
            .get(index)
            // 选择事实优先发布稳定 link，旧条目继续回退标题。
            .map(|item| SemanticEvent::change(id, Self::selection_value(item)))
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    wants_continuous_pointer_move => (&self) -> bool { self.overflow_open }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        // 主行与溢出菜单共享一次主题解析。
        let visual = self.visual.resolve(ctx.tokens());
        let slots = self.line_layout();
        for (position, (slot, local_rect)) in slots.iter().enumerate() {
            let rect = Self::absolute_rect(*local_rect, frame);
            match slot {
                BreadcrumbSlot::Item(index) => {
                    let item = &self.items[*index];
                    let color = if item.active { visual.text } else { visual.text_secondary };
                    self.paint_item(ctx, item, rect, color, 0.0);
                }
                BreadcrumbSlot::Overflow => {
                    if self.overflow_open {
                        ctx.fill_rect(
                            rect,
                            visual.fill_tertiary,
                            Some(Radius::uniform(visual.radius)),
                        );
                    }
                    ctx.text_center(
                        "...",
                        rect,
                        if self.overflow_open { visual.primary } else { visual.text_secondary },
                        self.visual.typography.title,
                    );
                }
            }
            if position + 1 < slots.len() {
                let separator_rect = Rect::new(
                    rect.x + rect.w,
                    frame.y,
                    self.separator_width(&self.separator),
                    self.visual.layout.height,
                );
                ctx.text_center(
                    &self.separator,
                    separator_rect,
                    visual.text_secondary,
                    self.visual.typography.separator,
                );
            }
        }

        if self.overflow_open {
            self.paint_overflow(frame, ctx, &visual);
        }

        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                Rect::new(frame.x, frame.y, self.line_width(), self.visual.layout.height),
                visual.primary,
                self.visual.chrome.focus_width,
                Some(Radius::uniform(visual.radius)),
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
        Size::new(line_width, self.visual.layout.height)
    }

    /// 创建使用斜杠分隔且不折叠条目的空面包屑导航。
    pub fn new() -> Self {
        let visual = BREADCRUMB_VISUAL_REF;
        Self {
            items: Vec::new(),
            separator: "/".to_string(),
            max_items: 0,
            focused: false,
            overflow_open: false,
            overflow_highlighted: None,
            layout_requested: Cell::new(false),
            pending_change: Cell::new(None),
            visual,
        }
    }
    /// 追加路径条目，并保证仅有一个活动条目。
    pub fn item(mut self, item: BreadcrumbItem) -> Self {
        self.items.push(item);
        self.normalize_active();
        self
    }
    /// 替换全部路径条目，并保证仅有一个活动条目。
    pub fn items(mut self, items: Vec<BreadcrumbItem>) -> Self {
        self.items = items;
        self.normalize_active();
        self
    }
    // 把末项声明为当前页，空集合保持安全无选中项。
    /// 将最后一个路径条目标记为当前页。
    pub fn last_active(mut self) -> Self {
        // 仅在至少存在一个条目时更新组件拥有的激活状态。
        if !self.items.is_empty() {
            // 先计算末项索引，避免可变借用与长度读取重叠。
            let last = self.items.len() - 1;
            // 复用唯一激活状态归一化入口。
            self.set_active(last);
        }
        // 返回配置完成的 Breadcrumb。
        self
    }
    /// 设置相邻可见路径条目之间的分隔文本。
    pub fn separator(mut self, s: impl Into<String>) -> Self {
        self.separator = s.into();
        self
    }

    /// 设置折叠前最多显示的条目数；零表示显示全部，正数至少显示三项。
    pub fn max_items(mut self, maximum: usize) -> Self {
        self.max_items = maximum;
        self
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        // UIX 视觉随下一声明更新。
        self.visual = next.visual;
        // 记录旧激活项的稳定身份；无 link 的旧条目兼容使用标题。
        let active_identity = self.items.get(self.active_index()).map(|item| {
            // 非空 link 是首选稳定身份。
            if !item.link.is_empty() {
                // 标记当前身份来自 link。
                (true, item.link.clone())
            } else {
                // 旧条目退回显示标题身份。
                (false, item.title.clone())
            }
        });
        self.items = next.items;
        self.normalize_active();
        // 只有旧运行节点确实存在激活项时才尝试保留选择。
        if let Some((uses_link, active_identity)) = active_identity {
            // 按旧身份种类在新声明中查找同一条目。
            if let Some(index) = self
                .items
                .iter()
                // link 身份不得因重复显示标题而串到其他条目。
                .position(|item| {
                    // link 条目按稳定 link 匹配，旧条目按标题兼容匹配。
                    if uses_link {
                        // 要求新条目具有同一非空 link。
                        item.link == active_identity
                    } else {
                        // 兼容没有 link 的旧标题选择。
                        item.link.is_empty() && item.title == active_identity
                    }
                })
            {
                self.set_active(index);
            }
        }
        self.separator = next.separator;
        self.max_items = next.max_items;
        self.close_overflow();
        self.pending_change.set(None);
    }

    /// 返回当前活动条目索引；空集合返回零。
    pub fn active_index(&self) -> usize {
        self.items.iter().position(|item| item.active).unwrap_or(0)
    }

    /// 返回当前活动条目的展示标题。
    pub fn active_title(&self) -> Option<&str> {
        self.items
            .get(self.active_index())
            .map(|item| item.title.as_str())
    }

    // 读取当前项的稳定链接；旧无链接条目返回 None。
    /// 返回当前活动条目的非空稳定链接。
    pub fn active_link(&self) -> Option<&str> {
        // 取得当前激活项并过滤空链接兼容值。
        self.items
            // 根据组件拥有的唯一激活状态取条目。
            .get(self.active_index())
            // 借用条目链接。
            .map(|item| item.link.as_str())
            // 空链接只表示旧标题回退，不伪装成稳定身份。
            .filter(|link| !link.is_empty())
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

    // 生成选择事实的稳定文本载荷。
    fn selection_value(item: &BreadcrumbItem) -> String {
        // 非空 link 优先承载路由身份。
        if !item.link.is_empty() {
            // 克隆拥有型 link 进入语义事件。
            item.link.clone()
        } else {
            // 旧条目继续以标题作为兼容载荷。
            item.title.clone()
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
        if pos.y < 0.0 || pos.y >= self.visual.layout.height || pos.x < 0.0 {
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
        let separator_width = self.separator_width(&self.separator);
        let mut x = 0.0;
        let mut layout = Vec::with_capacity(slots.len());
        for (position, slot) in slots.iter().copied().enumerate() {
            let width = match slot {
                BreadcrumbSlot::Item(index) => self.item_width(&self.items[index]),
                BreadcrumbSlot::Overflow => {
                    self.text_width("...", self.visual.layout.title_glyph_width)
                }
            };
            layout.push((slot, Rect::new(x, 0.0, width, self.visual.layout.height)));
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
            .map(|index| {
                self.item_width(&self.items[*index])
                    + self.visual.layout.overflow_horizontal_padding * 2.0
            })
            .fold(self.visual.layout.overflow_min_width, f32::max);
        let menu = Rect::new(
            trigger.x,
            self.visual.layout.height,
            width,
            hidden.len() as f32 * self.visual.layout.overflow_row_height,
        );
        let rows = hidden
            .into_iter()
            .enumerate()
            .map(|(row, index)| {
                (
                    index,
                    Rect::new(
                        menu.x,
                        menu.y + row as f32 * self.visual.layout.overflow_row_height,
                        menu.w,
                        self.visual.layout.overflow_row_height,
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
            let icon_rect = Rect::new(title_x, rect.y, self.visual.layout.icon_slot_width, rect.h);
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                &item.icon,
                icon_rect,
                color,
                self.visual.typography.icon,
            );
            title_x += self.visual.layout.icon_slot_width + self.visual.layout.icon_text_gap;
        }
        let title_rect = Rect::new(
            title_x,
            rect.y,
            self.text_width(&item.title, self.visual.layout.title_glyph_width),
            rect.h,
        );
        ctx.text_center(&item.title, title_rect, color, self.visual.typography.title);
    }

    fn paint_overflow(
        &self,
        frame: Rect,
        ctx: &mut PaintContext,
        visual: &ResolvedBreadcrumbVisual,
    ) {
        let Some((local_menu, rows)) = self.overflow_layout() else {
            return;
        };
        let menu = Self::absolute_rect(local_menu, frame);
        let radius = Some(Radius::uniform(visual.radius));
        let shadow = visual.shadow;
        ctx.draw_box_shadow(
            menu,
            shadow.layer_1.2,
            shadow.layer_1.0,
            shadow.layer_1.1,
            shadow.layer_1.3,
            radius,
        );
        ctx.fill_rect(menu, visual.elevated_background, radius);
        ctx.stroke_rect(menu, visual.border, self.visual.chrome.border_width, radius);

        for (index, local_row) in rows {
            let row = Self::absolute_rect(local_row, frame);
            if self.overflow_highlighted == Some(index) {
                ctx.fill_rect(row, visual.fill_tertiary, radius);
            }
            self.paint_item(
                ctx,
                &self.items[index],
                row,
                visual.text,
                self.visual.layout.overflow_horizontal_padding,
            );
        }
    }

    fn item_width(&self, item: &BreadcrumbItem) -> f32 {
        self.text_width(&item.title, self.visual.layout.title_glyph_width)
            + if item.icon.is_empty() {
                0.0
            } else {
                self.visual.layout.icon_slot_width + self.visual.layout.icon_text_gap
            }
    }

    fn separator_width(&self, separator: &str) -> f32 {
        self.text_width(separator, self.visual.layout.separator_glyph_width)
    }

    fn absolute_rect(rect: Rect, frame: Rect) -> Rect {
        Rect::new(frame.x + rect.x, frame.y + rect.y, rect.w, rect.h)
    }

    fn text_width(&self, text: &str, glyph_width: f32) -> f32 {
        text.chars().count() as f32 * glyph_width + self.visual.layout.text_horizontal_padding
    }

    // 测试目标保留面包屑可见槽位观测入口，供导航布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
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

    // 测试目标保留面包屑溢出触发区域观测入口，供导航交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn overflow_trigger_rect_for_test(&self) -> Option<Rect> {
        self.line_layout()
            .into_iter()
            .find_map(|(slot, rect)| (slot == BreadcrumbSlot::Overflow).then_some(rect))
    }

    // 测试目标保留面包屑溢出行观测入口，供导航交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn overflow_rows_for_test(&self) -> Vec<(usize, Rect)> {
        self.overflow_layout()
            .map(|(_, rows)| rows)
            .unwrap_or_default()
    }

    // 测试目标保留面包屑溢出状态观测入口，供导航交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn overflow_state_for_test(&self) -> (bool, Option<usize>) {
        (self.overflow_open, self.overflow_highlighted)
    }

    // 测试目标保留面包屑内容区域观测入口，供导航布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
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
            self.visual.layout.icon_slot_width,
            item_rect.h,
        ));
        let title_x = item_rect.x
            + if icon.is_some() {
                self.visual.layout.icon_slot_width + self.visual.layout.icon_text_gap
            } else {
                0.0
            };
        let title = Rect::new(
            title_x,
            item_rect.y,
            self.text_width(&item.title, self.visual.layout.title_glyph_width),
            item_rect.h,
        );
        Some((item_rect, icon, title))
    }
}

// UIX 根把声明视觉注入 Rust 路径与折叠内核。
fn build_breadcrumb_view(mut kernel: Breadcrumb, visual: &'static BreadcrumbVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Breadcrumb {
    fn build(self) -> ViewNode {
        build_breadcrumb_view(self, BREADCRUMB_VISUAL_REF)
    }
}
