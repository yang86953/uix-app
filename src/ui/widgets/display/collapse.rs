//! Collapse widget — 折叠面板。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::api::PaintContext;
use crate::draw::Radius;
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotCollapsePanel,
    SnapshotFields, SystemEvent, WidgetTree,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const HEADER_HEIGHT: f32 = 36.0;
const HEADER_FONT_SIZE: f32 = 14.0;
const CONTENT_FONT_SIZE: f32 = 12.0;
const TEXT_LINE_HEIGHT: f32 = 1.5;
const HEADER_ICON_SLOT: f32 = 28.0;
const HEADER_RIGHT_PADDING: f32 = 12.0;
const CONTENT_HORIZONTAL_PADDING: f32 = 16.0;
const CONTENT_VERTICAL_PADDING: f32 = 8.0;
const DEFAULT_WIDTH: f32 = 240.0;
const MAX_INTRINSIC_WIDTH: f32 = 320.0;

/// 单个折叠面板。
#[derive(Debug, Clone)]
pub struct CollapsePanel {
    pub header: String,
    pub content: String,
    pub expanded: bool,
}

impl CollapsePanel {
    pub fn new(header: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            header: header.into(),
            content: content.into(),
            expanded: false,
        }
    }
    pub fn expanded(mut self) -> Self {
        self.expanded = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollapseContentEntry {
    panel_index: usize,
    key: String,
    content: String,
}

component! {
    /// Collapse — 可折叠面板组。
    pub struct Collapse {
        pub(crate) panels: Vec<CollapsePanel>,
        accordion: bool,
        borderless: bool,
        destroy_on_hide: bool,
        focused: bool,
        focused_header: usize,
        hovered_header: Cell<Option<usize>>,
        pressed_header: Cell<Option<usize>>,
        last_frame: Cell<Option<Rect>>,
        pending_change: Cell<Option<usize>>,
        pub(crate) transitions: Vec<TransitionPlayer>,
        transition_dirty: bool,
        layout_requested: Cell<bool>,
        #[snapshot(skip)]
        content_opacities: Vec<Rc<Cell<f32>>>,
        #[snapshot(skip)]
        materialized_content: RefCell<Vec<CollapseContentEntry>>,
    }

    tab_index => (&self) -> i32 { i32::from(!self.panels.is_empty()) }

    measure => (&self, constraints: Constraints) -> Size {
        let preferred_width = self.preferred_width();
        let width = constraints.clamp(Size::new(preferred_width, 0.0)).w;
        constraints.clamp(Size::new(width, self.intrinsic_height(width)))
    }

    build_view_children => (&self) -> Vec<crate::ui::view::ViewNode> {
        let entries = self.desired_content_entries();
        let views = self.content_views(&entries);
        self.materialized_content.replace(entries);
        views
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        let entries = self.materialized_content.borrow();
        children
            .iter()
            .enumerate()
            .filter_map(|(child_index, child)| {
                let entry = entries.get(child_index)?;
                self.content_frame(frame, entry.panel_index)
                    .map(|content_frame| (child.id, content_frame))
            })
            .collect()
    }

    child_visible => (&self, index: usize) -> bool {
        let entries = self.materialized_content.borrow();
        let Some(entry) = entries.get(index) else {
            return false;
        };
        self.panels
            .get(entry.panel_index)
            .is_some_and(|panel| self.panel_present(entry.panel_index, panel))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(index) = self.header_at_point(*pos) {
                    self.focused_header = index;
                    self.pressed_header.set(Some(index));
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let pressed = self.pressed_header.replace(None);
                if let Some(index) = pressed {
                    if self.header_at_point(*pos) == Some(index) {
                        self.focused_header = index;
                        self.toggle_panel(index);
                    }
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerMove { pos, .. } => {
                let next = self.header_at_point(*pos);
                if self.hovered_header.replace(next) != next {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave => {
                let changed = self.hovered_header.replace(None).is_some()
                    | self.pressed_header.replace(None).is_some();
                if changed { EventResult::Handled } else { EventResult::NotHandled }
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.pressed_header.set(None);
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Down => {
                    self.move_focus(true);
                    EventResult::Handled
                }
                KeyCode::Up => {
                    self.move_focus(false);
                    EventResult::Handled
                }
                KeyCode::Home => {
                    self.focused_header = 0;
                    EventResult::Handled
                }
                KeyCode::End if !self.panels.is_empty() => {
                    self.focused_header = self.panels.len() - 1;
                    EventResult::Handled
                }
                KeyCode::Enter | KeyCode::Space if !self.panels.is_empty() => {
                    self.toggle_panel(self.focused_header);
                    EventResult::Handled
                }
                KeyCode::Right if !self.panels.is_empty() => {
                    self.set_panel_expanded(self.focused_header, true);
                    EventResult::Handled
                }
                KeyCode::Left if !self.panels.is_empty() => {
                    self.set_panel_expanded(self.focused_header, false);
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|idx| SemanticEvent::change(id, idx.to_string()))
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let frame = Self::normalized_frame(frame);
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let bg = ctx.tokens().color_bg_elevated();
        let body_bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let primary = ctx.tokens().color_primary();
        let hover_bg = ctx.tokens().color_fill_quaternary();
        let pressed_bg = ctx.tokens().color_fill_tertiary();
        let r = (!self.borderless).then(|| Radius::uniform(ctx.tokens().border_radius_sm()));
        let mut y = frame.y;
        let frame_bottom = frame.y + frame.h;
        ctx.push_clip(frame);

        for (idx, p) in self.panels.iter().enumerate() {
            if y >= frame_bottom {
                break;
            }
            let header_rect = Rect::new(frame.x, y, frame.w, HEADER_HEIGHT.min(frame_bottom - y));
            let header_bg = if self.pressed_header.get() == Some(idx) {
                pressed_bg
            } else if self.hovered_header.get() == Some(idx) {
                hover_bg
            } else {
                bg
            };
            ctx.fill_rect(header_rect, header_bg, r);
            if !self.borderless {
                ctx.stroke_rect(header_rect, border, 1.0, r);
            }
            if self.focused && tree.keyboard_focus_visible() && idx == self.focused_header {
                let inset = 0.75_f32.min(header_rect.w * 0.5).min(header_rect.h * 0.5);
                let focus_rect = Rect::new(
                    header_rect.x + inset,
                    header_rect.y + inset,
                    (header_rect.w - inset * 2.0).max(0.0),
                    (header_rect.h - inset * 2.0).max(0.0),
                );
                if focus_rect.w > 0.0 && focus_rect.h > 0.0 {
                    ctx.stroke_rect(focus_rect, primary, 1.5, r);
                }
            }
            let icon_rect = Rect::new(header_rect.x + 4.0, header_rect.y, 24.0, header_rect.h);
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                if p.expanded { "chevron-down" } else { "chevron-right" },
                icon_rect,
                text_secondary,
                12.0_f32.min(header_rect.h * 0.6),
            );
            let text_width = (header_rect.w - HEADER_ICON_SLOT - HEADER_RIGHT_PADDING).max(0.0);
            if let Some(visible_header) = elide_single_line(
                ctx,
                &p.header,
                HEADER_FONT_SIZE,
                text_width,
            ) {
                let header_y = ctx.visual_center_y(header_rect, HEADER_FONT_SIZE);
                let text_rect = Rect::new(
                    header_rect.x + HEADER_ICON_SLOT,
                    header_rect.y,
                    text_width,
                    header_rect.h,
                );
                ctx.push_clip(text_rect);
                ctx.draw_text(
                    &visible_header,
                    Point::new(text_rect.x, header_y),
                    text_color,
                    HEADER_FONT_SIZE,
                );
                ctx.pop_clip();
            }
            y += HEADER_HEIGHT;

            if self.panel_present(idx, p) {
                let content_height = Self::content_height(&p.content, frame.w);
                let body_height = content_height.min((frame_bottom - y).max(0.0));
                let body_rect = Rect::new(frame.x, y, frame.w, body_height);
                if body_rect.w > 0.0 && body_rect.h > 0.0 {
                    ctx.fill_rect(body_rect, body_bg, r);
                    if !self.borderless {
                        ctx.stroke_rect(body_rect, border, 1.0, r);
                    }
                }
                y += content_height;
            }
            if self.borderless && idx + 1 < self.panels.len() && y < frame_bottom {
                ctx.draw_line(frame.x, y, frame.x + frame.w, y, border, 1.0);
            }
        }
        ctx.pop_clip();
    }

    // NOTE(布局): dirty_rect 目前返回所有面板最大展开时的全量区域（frame），
    // 而不是仅返回变化区域（delta）。因为 collapse 无法可靠追踪哪个面板的
    // expanded 状态在上帧到本帧之间发生了变化（on_event 中修改 expanded 时
    // 未保存旧状态），返回全量可确保展开/折叠时残留像素被清除。
    // 优化方向：在 on_event 中记录 changed_panel index，dirty_rect 仅返回
    // 该 header + 内容区域的变化部分。
    // 始终包含最大展开高度，确保 expanded 切换时残留像素被清除
    dirty_rect => (&self, frame: Rect) -> Rect {
        self.full_dirty_rect(frame)
    }

    update_animation => (&mut self, dt: f64) -> bool {
        self.ensure_transition_count();
        let mut had_active = false;
        let mut still_active = false;
        for (index, transition) in self.transitions.iter_mut().enumerate() {
            if !transition.finished {
                had_active = true;
                transition.update(dt);
                if let Some(opacity) = self.content_opacities.get(index) {
                    opacity.set(transition.opacity_progress.clamp(0.0, 1.0));
                }
                still_active |= !transition.finished;
                if transition.finished
                    && self.panels.get(index).is_some_and(|panel| !panel.expanded)
                {
                    self.layout_requested.set(true);
                }
            }
        }
        self.transition_dirty = had_active;
        still_active
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty {
            self.full_dirty_rect(frame)
        } else {
            Rect::zero()
        }
    }
}

impl Default for Collapse {
    fn default() -> Self {
        Self::new()
    }
}

impl Collapse {
    fn preferred_width(&self) -> f32 {
        let mut width = DEFAULT_WIDTH;
        for panel in &self.panels {
            let header = single_line(&panel.header);
            let header_width = crate::draw::resources::font::text_backend::estimate_text_metrics(
                &header,
                f32::INFINITY,
                HEADER_FONT_SIZE,
            )
            .max_line_width
                + HEADER_ICON_SLOT
                + HEADER_RIGHT_PADDING;
            let content_width = crate::draw::resources::font::text_backend::estimate_text_metrics(
                &panel.content,
                f32::INFINITY,
                CONTENT_FONT_SIZE,
            )
            .max_line_width
                + CONTENT_HORIZONTAL_PADDING * 2.0;
            width = width.max(header_width).max(content_width);
        }
        width.min(MAX_INTRINSIC_WIDTH)
    }

    fn intrinsic_height(&self, width: f32) -> f32 {
        let mut h = 0.0f32;
        for (idx, p) in self.panels.iter().enumerate() {
            h += HEADER_HEIGHT;
            if self.panel_present(idx, p) {
                h += Self::content_height(&p.content, width);
            }
        }
        h
    }

    pub fn new() -> Self {
        Self {
            panels: Vec::new(),
            accordion: false,
            borderless: false,
            destroy_on_hide: false,
            focused: false,
            focused_header: 0,
            hovered_header: Cell::new(None),
            pressed_header: Cell::new(None),
            last_frame: Cell::new(None),
            pending_change: Cell::new(None),
            transitions: Vec::new(),
            transition_dirty: false,
            layout_requested: Cell::new(false),
            content_opacities: Vec::new(),
            materialized_content: RefCell::new(Vec::new()),
        }
    }
    pub fn panels(mut self, ps: Vec<CollapsePanel>) -> Self {
        self.panels = ps;
        self.normalize_accordion();
        self.transitions = Self::settled_transitions(&self.panels);
        self.content_opacities = Self::opacity_handles(&self.transitions);
        self
    }
    pub fn accordion(mut self) -> Self {
        self.accordion = true;
        self.normalize_accordion();
        self.transitions = Self::settled_transitions(&self.panels);
        self.content_opacities = Self::opacity_handles(&self.transitions);
        self
    }

    pub fn borderless(mut self, value: bool) -> Self {
        self.borderless = value;
        self
    }

    pub fn destroy_on_hide(mut self, value: bool) -> Self {
        self.destroy_on_hide = value;
        self
    }

    pub fn focused_header(&self) -> usize {
        self.focused_header
    }

    pub fn expanded_indices(&self) -> Vec<usize> {
        self.panels
            .iter()
            .enumerate()
            .filter_map(|(index, panel)| panel.expanded.then_some(index))
            .collect()
    }

    fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(frame.x, frame.y, frame.w.max(0.0), frame.h.max(0.0))
    }

    fn content_height(content: &str, width: f32) -> f32 {
        let text_width = (width - CONTENT_HORIZONTAL_PADDING * 2.0).max(1.0);
        let line_count = crate::draw::resources::font::text_backend::estimate_text_metrics(
            content,
            text_width,
            CONTENT_FONT_SIZE,
        )
        .line_count
        .max(1) as f32;
        line_count * CONTENT_FONT_SIZE * TEXT_LINE_HEIGHT + CONTENT_VERTICAL_PADDING * 2.0
    }

    fn panel_content_key(&self, panel_index: usize) -> String {
        let header = self
            .panels
            .get(panel_index)
            .map(|panel| panel.header.as_str())
            .unwrap_or_default();
        let occurrence = self.panels[..panel_index.min(self.panels.len())]
            .iter()
            .filter(|panel| panel.header == header)
            .count();
        format!(
            "uix:collapse-content:{}:{occurrence}:{header}",
            header.len()
        )
    }

    fn desired_content_entries(&self) -> Vec<CollapseContentEntry> {
        self.panels
            .iter()
            .enumerate()
            .filter(|(index, panel)| !self.destroy_on_hide || self.panel_present(*index, panel))
            .map(|(panel_index, panel)| CollapseContentEntry {
                panel_index,
                key: self.panel_content_key(panel_index),
                content: panel.content.clone(),
            })
            .collect()
    }

    fn content_views(&self, entries: &[CollapseContentEntry]) -> Vec<crate::ui::view::ViewNode> {
        entries
            .iter()
            .map(|entry| {
                let text = entry.content.clone();
                let opacity = self
                    .content_opacities
                    .get(entry.panel_index)
                    .cloned()
                    .unwrap_or_else(|| Rc::new(Cell::new(0.0)));
                let natural_height = (Self::content_height(&text, DEFAULT_WIDTH)
                    - CONTENT_VERTICAL_PADDING * 2.0)
                    .max(0.0);
                crate::ui::view::canvas(
                    (DEFAULT_WIDTH - CONTENT_HORIZONTAL_PADDING * 2.0).max(0.0),
                    natural_height,
                    move |frame, ctx| {
                        if frame.w <= 0.0 || frame.h <= 0.0 {
                            return;
                        }
                        let text_color = ctx.tokens().color_text_secondary();
                        let alpha = (text_color.a as f32 * opacity.get())
                            .round()
                            .clamp(0.0, 255.0) as u8;
                        if alpha == 0 {
                            return;
                        }
                        ctx.push_clip(frame);
                        ctx.draw_text_wrapped(
                            &text,
                            frame,
                            text_color.with_alpha(alpha),
                            CONTENT_FONT_SIZE,
                        );
                        ctx.pop_clip();
                    },
                )
                .key(entry.key.clone())
            })
            .collect()
    }

    fn content_frame(&self, frame: Rect, panel_index: usize) -> Option<Rect> {
        let frame = Self::normalized_frame(frame);
        let frame_bottom = frame.y + frame.h;
        let mut y = frame.y;
        for (index, panel) in self.panels.iter().enumerate() {
            y += HEADER_HEIGHT;
            if !self.panel_present(index, panel) {
                continue;
            }
            let content_height = Self::content_height(&panel.content, frame.w);
            if index == panel_index {
                let body_height = content_height.min((frame_bottom - y).max(0.0));
                return Some(Rect::new(
                    frame.x + CONTENT_HORIZONTAL_PADDING,
                    y + CONTENT_VERTICAL_PADDING,
                    (frame.w - CONTENT_HORIZONTAL_PADDING * 2.0).max(0.0),
                    (body_height - CONTENT_VERTICAL_PADDING * 2.0).max(0.0),
                ));
            }
            y += content_height;
        }
        None
    }

    pub(crate) fn content_views_for_refresh(
        &self,
        current_child_count: usize,
    ) -> Option<(Vec<crate::ui::view::ViewNode>, Vec<CollapseContentEntry>)> {
        let entries = self.desired_content_entries();
        if current_child_count == entries.len() && *self.materialized_content.borrow() == entries {
            return None;
        }
        Some((self.content_views(&entries), entries))
    }

    pub(crate) fn mark_content_materialized(&self, entries: Vec<CollapseContentEntry>) {
        self.materialized_content.replace(entries);
    }

    fn full_dirty_rect(&self, frame: Rect) -> Rect {
        let mut h = self.panels.len() as f32 * 36.0;
        for panel in &self.panels {
            h += Self::content_height(&panel.content, frame.w);
        }
        Rect::new(frame.x, frame.y, frame.w, h)
    }

    fn settled_transitions(panels: &[CollapsePanel]) -> Vec<TransitionPlayer> {
        panels
            .iter()
            .map(|panel| Self::settled_transition(panel.expanded))
            .collect()
    }

    fn settled_transition(expanded: bool) -> TransitionPlayer {
        let mut transition = if expanded {
            TransitionPlayer::new(presets::collapse_expand())
        } else {
            TransitionPlayer::new(presets::collapse_collapse())
        };
        transition.update(1.0);
        transition
    }

    fn opacity_handles(transitions: &[TransitionPlayer]) -> Vec<Rc<Cell<f32>>> {
        transitions
            .iter()
            .map(|transition| Rc::new(Cell::new(transition.opacity_progress.clamp(0.0, 1.0))))
            .collect()
    }

    fn ensure_transition_count(&mut self) {
        if self.transitions.len() == self.panels.len() {
            return;
        }
        self.transitions = Self::settled_transitions(&self.panels);
        self.content_opacities = Self::opacity_handles(&self.transitions);
    }

    fn start_panel_transition(&mut self, idx: usize, expanded: bool) {
        self.ensure_transition_count();
        if let Some(transition) = self.transitions.get_mut(idx) {
            let config = if expanded {
                presets::collapse_expand()
            } else {
                presets::collapse_collapse()
            };
            *transition = TransitionPlayer::new_from_current(
                config,
                transition.opacity_progress,
                transition.offset,
                transition.scale,
            );
            if let Some(opacity) = self.content_opacities.get(idx) {
                opacity.set(transition.opacity_progress.clamp(0.0, 1.0));
            }
            self.transition_dirty = true;
        }
    }

    pub(crate) fn panel_present(&self, idx: usize, panel: &CollapsePanel) -> bool {
        if self.destroy_on_hide && !panel.expanded {
            return self
                .transitions
                .get(idx)
                .is_some_and(|transition| !transition.finished);
        }
        panel.expanded
            || self
                .transitions
                .get(idx)
                .is_some_and(|transition| !transition.finished)
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let current_panels = std::mem::take(&mut self.panels);
        let current_transitions = std::mem::take(&mut self.transitions);
        let current_opacities = std::mem::take(&mut self.content_opacities);
        let focused_header = current_panels
            .get(self.focused_header)
            .map(|panel| panel.header.clone());
        let mut panels = next.panels;
        let same_len = current_panels.len() == panels.len();
        let next_header_is_unique = panels
            .iter()
            .map(|panel| {
                panels
                    .iter()
                    .filter(|candidate| candidate.header == panel.header)
                    .count()
                    == 1
            })
            .collect::<Vec<_>>();
        let mut used = vec![false; current_panels.len()];
        let mut transitions = Vec::with_capacity(panels.len());
        let mut content_opacities = Vec::with_capacity(panels.len());
        for (idx, panel) in panels.iter_mut().enumerate() {
            let header_is_unique = current_panels
                .iter()
                .filter(|current| current.header == panel.header)
                .count()
                == 1
                && next_header_is_unique[idx];
            let matched = header_is_unique
                .then(|| {
                    current_panels
                        .iter()
                        .enumerate()
                        .find(|(current_idx, current)| {
                            !used[*current_idx] && current.header == panel.header
                        })
                        .map(|(current_idx, _)| current_idx)
                })
                .flatten()
                .or_else(|| same_len.then_some(idx).filter(|index| !used[*index]));
            if let Some(current_idx) = matched {
                used[current_idx] = true;
                panel.expanded = current_panels[current_idx].expanded;
                let transition = current_transitions
                    .get(current_idx)
                    .cloned()
                    .unwrap_or_else(|| Self::settled_transition(panel.expanded));
                content_opacities.push(current_opacities.get(current_idx).cloned().unwrap_or_else(
                    || Rc::new(Cell::new(transition.opacity_progress.clamp(0.0, 1.0))),
                ));
                transitions.push(transition);
            } else {
                let transition = Self::settled_transition(panel.expanded);
                content_opacities.push(Rc::new(Cell::new(
                    transition.opacity_progress.clamp(0.0, 1.0),
                )));
                transitions.push(transition);
            }
        }
        self.panels = panels;
        self.accordion = next.accordion;
        self.borderless = next.borderless;
        self.destroy_on_hide = next.destroy_on_hide;
        let expanded_before_normalize = self
            .panels
            .iter()
            .map(|panel| panel.expanded)
            .collect::<Vec<_>>();
        self.normalize_accordion();
        self.focused_header = focused_header
            .as_ref()
            .and_then(|header| {
                (self
                    .panels
                    .iter()
                    .filter(|panel| &panel.header == header)
                    .count()
                    == 1)
                    .then(|| self.panels.iter().position(|panel| &panel.header == header))
                    .flatten()
            })
            .unwrap_or_else(|| self.focused_header.min(self.panels.len().saturating_sub(1)));
        self.hovered_header.set(None);
        self.pressed_header.set(None);
        self.last_frame.set(None);
        self.transitions = transitions
            .into_iter()
            .enumerate()
            .map(|(index, transition)| {
                if expanded_before_normalize.get(index)
                    == self.panels.get(index).map(|panel| &panel.expanded)
                {
                    transition
                } else {
                    Self::settled_transition(self.panels[index].expanded)
                }
            })
            .collect();
        self.content_opacities = content_opacities;
        for (index, transition) in self.transitions.iter().enumerate() {
            if let Some(opacity) = self.content_opacities.get(index) {
                opacity.set(transition.opacity_progress.clamp(0.0, 1.0));
            }
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Collapse {
            panels: self
                .panels
                .iter()
                .map(|panel| SnapshotCollapsePanel {
                    header: panel.header.clone(),
                    content: panel.content.clone(),
                    expanded: panel.expanded,
                })
                .collect(),
            accordion: self.accordion,
            focused_header: self.focused_header,
        }
    }

    fn normalize_accordion(&mut self) {
        if !self.accordion {
            return;
        }
        let mut found_expanded = false;
        for panel in &mut self.panels {
            if panel.expanded && !found_expanded {
                found_expanded = true;
            } else if panel.expanded {
                panel.expanded = false;
            }
        }
    }

    fn header_at_point(&self, point: Point) -> Option<usize> {
        let frame = self.last_frame.get().unwrap_or_else(|| {
            let width = self.preferred_width();
            Rect::new(0.0, 0.0, width, self.intrinsic_height(width))
        });
        if !frame.contains(point) {
            return None;
        }
        let mut cursor = 0.0;
        for (index, panel) in self.panels.iter().enumerate() {
            if point.y >= cursor && point.y < cursor + HEADER_HEIGHT {
                return Some(index);
            }
            cursor += HEADER_HEIGHT;
            if self.panel_present(index, panel) {
                cursor += Self::content_height(&panel.content, frame.w);
            }
        }
        None
    }

    fn move_focus(&mut self, forward: bool) {
        if self.panels.is_empty() {
            return;
        }
        self.focused_header = if forward {
            (self.focused_header + 1).min(self.panels.len() - 1)
        } else {
            self.focused_header.saturating_sub(1)
        };
    }

    fn toggle_panel(&mut self, index: usize) {
        let Some(panel) = self.panels.get(index) else {
            return;
        };
        self.set_panel_expanded(index, !panel.expanded);
    }

    fn set_panel_expanded(&mut self, index: usize, expanded: bool) {
        let Some(panel) = self.panels.get(index) else {
            return;
        };
        if panel.expanded == expanded {
            return;
        }
        self.ensure_transition_count();
        let old_states = self
            .panels
            .iter()
            .map(|panel| panel.expanded)
            .collect::<Vec<_>>();
        let name = self.panels[index].header.clone();
        if self.accordion && expanded {
            for panel in &mut self.panels {
                panel.expanded = false;
            }
        }
        self.panels[index].expanded = expanded;
        let changed = self
            .panels
            .iter()
            .enumerate()
            .filter_map(|(panel_index, panel)| {
                (panel.expanded != old_states[panel_index]).then_some((panel_index, panel.expanded))
            })
            .collect::<Vec<_>>();
        for (panel_index, panel_expanded) in changed {
            self.start_panel_transition(panel_index, panel_expanded);
        }
        self.layout_requested.set(true);
        self.pending_change.set(Some(index));
        tracing::debug!(
            "[Collapse] 面板 \"{name}\" 切换 expanded: {} → {expanded}",
            !expanded
        );
    }
}

fn single_line(text: &str) -> String {
    text.replace(['\r', '\n'], " ")
}

fn conservative_text_width(ctx: &mut PaintContext<'_>, text: &str, font_size: f32) -> f32 {
    ctx.measure_text(text, font_size).w.max(
        crate::draw::resources::font::text_backend::estimate_text_metrics(
            text,
            f32::INFINITY,
            font_size,
        )
        .max_line_width,
    )
}

fn elide_single_line(
    ctx: &mut PaintContext<'_>,
    text: &str,
    font_size: f32,
    max_width: f32,
) -> Option<String> {
    if !max_width.is_finite() || max_width <= 0.0 {
        return None;
    }
    let text = single_line(text);
    if conservative_text_width(ctx, &text, font_size) <= max_width {
        return Some(text);
    }
    const ELLIPSIS: &str = "…";
    if conservative_text_width(ctx, ELLIPSIS, font_size) > max_width {
        return None;
    }
    let mut visible = String::new();
    for ch in text.chars() {
        visible.push(ch);
        visible.push_str(ELLIPSIS);
        let fits = conservative_text_width(ctx, &visible, font_size) <= max_width;
        visible.pop();
        if !fits {
            visible.pop();
            break;
        }
    }
    visible.push_str(ELLIPSIS);
    Some(visible)
}
