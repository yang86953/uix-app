//! 折叠面板行为实现。

use super::*;
use super::free::*;

impl Collapse {
    pub(super) fn preferred_width(&self) -> f32 {
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

    pub(super) fn intrinsic_height(&self, width: f32) -> f32 {
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

    pub(super) fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(frame.x, frame.y, frame.w.max(0.0), frame.h.max(0.0))
    }

    pub(super) fn content_height(content: &str, width: f32) -> f32 {
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

    pub(super) fn desired_content_entries(&self) -> Vec<CollapseContentEntry> {
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

    pub(super) fn content_views(&self, entries: &[CollapseContentEntry]) -> Vec<crate::ui::view::ViewNode> {
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
                crate::ui::widgets::canvas(
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

    pub(super) fn content_frame(&self, frame: Rect, panel_index: usize) -> Option<Rect> {
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

    pub(super) fn full_dirty_rect(&self, frame: Rect) -> Rect {
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

    pub(super) fn ensure_transition_count(&mut self) {
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

    pub(super) fn header_at_point(&self, point: Point) -> Option<usize> {
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

    pub(super) fn move_focus(&mut self, forward: bool) {
        if self.panels.is_empty() {
            return;
        }
        self.focused_header = if forward {
            (self.focused_header + 1).min(self.panels.len() - 1)
        } else {
            self.focused_header.saturating_sub(1)
        };
    }

    pub(super) fn toggle_panel(&mut self, index: usize) {
        let Some(panel) = self.panels.get(index) else {
            return;
        };
        self.set_panel_expanded(index, !panel.expanded);
    }

    pub(super) fn set_panel_expanded(&mut self, index: usize, expanded: bool) {
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

