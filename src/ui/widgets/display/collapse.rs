//! Collapse widget — 折叠面板。

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Radius;
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotCollapsePanel,
    SnapshotFields, SystemEvent, WidgetTree,
};
use std::cell::Cell;

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

component! {
    /// Collapse — 可折叠面板组。
    pub struct Collapse {
        pub(crate) panels: Vec<CollapsePanel>,
        accordion: bool,
        focused: bool,
        focused_header: usize,
        pending_change: Cell<Option<usize>>,
        pub(crate) transitions: Vec<TransitionPlayer>,
        transition_dirty: bool,
    }

    tab_index => (&self) -> i32 { i32::from(!self.panels.is_empty()) }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(index) = self.header_at_y(pos.y) {
                    self.focused_header = index;
                    self.toggle_panel(index);
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
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

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let primary = ctx.tokens().color_primary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        let mut y = frame.y;

        for (idx, p) in self.panels.iter().enumerate() {
            let header_rect = Rect::new(frame.x, y, frame.w, 36.0);
            // header 背景
            ctx.fill_rect(header_rect, bg, r);
            ctx.stroke_rect(header_rect, border, 1.0, r);
            if self.focused && idx == self.focused_header {
                ctx.stroke_rect(header_rect, primary, 1.5, r);
            }
            // 展开指示符
            let arrow = if p.expanded { "▼" } else { "▶" };
            let arrow_y = ctx.visual_center_y(header_rect, 12.0);
            ctx.draw_text(arrow, crate::core::Point::new(frame.x + 10.0, arrow_y), text_secondary, 12.0);
            let header_y = ctx.visual_center_y(header_rect, 14.0);
            ctx.draw_text(&p.header, crate::core::Point::new(frame.x + 28.0, header_y), text_color, 14.0);
            y += 36.0;

            if self.panel_present(idx, p) {
                let content_y = y + 8.0;
                let alpha = (text_secondary.a as f32 * self.panel_opacity(idx, p))
                    .round()
                    .clamp(0.0, 255.0) as u8;
                if alpha > 0 {
                    ctx.draw_text(&p.content, crate::core::Point::new(frame.x + 16.0, content_y), text_secondary.with_alpha(alpha), 12.0);
                }
                y += Self::content_height(&p.content);
            }
        }
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
        for transition in &mut self.transitions {
            if !transition.finished {
                had_active = true;
                transition.update(dt);
                still_active |= !transition.finished;
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
    fn intrinsic_size(&self) -> Size {
        let mut h = 0.0f32;
        for (idx, p) in self.panels.iter().enumerate() {
            h += 36.0;
            if self.panel_present(idx, p) {
                h += Self::content_height(&p.content);
            }
        }
        Size::new(0.0, h)
    }

    pub fn new() -> Self {
        Self {
            panels: Vec::new(),
            accordion: false,
            focused: false,
            focused_header: 0,
            pending_change: Cell::new(None),
            transitions: Vec::new(),
            transition_dirty: false,
        }
    }
    pub fn panels(mut self, ps: Vec<CollapsePanel>) -> Self {
        self.panels = ps;
        self.normalize_accordion();
        self.transitions = Self::settled_transitions(&self.panels);
        self
    }
    pub fn accordion(mut self) -> Self {
        self.accordion = true;
        self.normalize_accordion();
        self.transitions = Self::settled_transitions(&self.panels);
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

    fn content_height(content: &str) -> f32 {
        let line_count = content.lines().count().max(1) as f32;
        line_count * 12.0 * 1.5 + 16.0
    }

    fn full_dirty_rect(&self, frame: Rect) -> Rect {
        let mut h = self.panels.len() as f32 * 36.0;
        for panel in &self.panels {
            h += Self::content_height(&panel.content);
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

    fn ensure_transition_count(&mut self) {
        if self.transitions.len() == self.panels.len() {
            return;
        }
        self.transitions = Self::settled_transitions(&self.panels);
    }

    fn start_panel_transition(&mut self, idx: usize, expanded: bool) {
        self.ensure_transition_count();
        if let Some(transition) = self.transitions.get_mut(idx) {
            *transition = if expanded {
                TransitionPlayer::new(presets::collapse_expand())
            } else {
                TransitionPlayer::new(presets::collapse_collapse())
            };
            self.transition_dirty = true;
        }
    }

    pub(crate) fn panel_present(&self, idx: usize, panel: &CollapsePanel) -> bool {
        panel.expanded
            || self
                .transitions
                .get(idx)
                .is_some_and(|transition| !transition.finished)
    }

    fn panel_opacity(&self, idx: usize, panel: &CollapsePanel) -> f32 {
        self.transitions
            .get(idx)
            .map(|transition| transition.opacity_progress.clamp(0.0, 1.0))
            .unwrap_or(if panel.expanded { 1.0 } else { 0.0 })
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let expanded_before_sync = self
            .panels
            .iter()
            .map(|panel| panel.expanded)
            .collect::<Vec<_>>();
        let mut panels = next.panels;
        for (idx, panel) in panels.iter_mut().enumerate() {
            if let Some(current) = self.panels.get(idx) {
                panel.expanded = current.expanded;
            }
        }
        self.panels = panels;
        self.accordion = next.accordion;
        self.normalize_accordion();
        self.focused_header = self.focused_header.min(self.panels.len().saturating_sub(1));
        self.transitions = self
            .panels
            .iter()
            .enumerate()
            .map(|(index, panel)| {
                if expanded_before_sync.get(index) == Some(&panel.expanded) {
                    self.transitions
                        .get(index)
                        .cloned()
                        .unwrap_or_else(|| Self::settled_transition(panel.expanded))
                } else {
                    Self::settled_transition(panel.expanded)
                }
            })
            .collect();
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

    fn header_at_y(&self, y: f32) -> Option<usize> {
        if y < 0.0 {
            return None;
        }
        let mut cursor = 0.0;
        for (index, panel) in self.panels.iter().enumerate() {
            if y >= cursor && y < cursor + 36.0 {
                return Some(index);
            }
            cursor += 36.0;
            if self.panel_present(index, panel) {
                cursor += Self::content_height(&panel.content);
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
        self.pending_change.set(Some(index));
        crate::core::log::debug_fn(format!(
            "[Collapse] 面板 \"{name}\" 切换 expanded: {} → {expanded}",
            !expanded
        ));
    }
}
