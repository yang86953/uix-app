//! 折叠面板行为实现。

use super::free::*;
use super::*;

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
            // 缺省保持既有非受控展开模式。
            active_keys_binding: None,
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
        // 受控模式按稳定 key 同步，非受控模式只归一化面板初值。
        if self.active_keys_binding.is_some() {
            // 支持状态先于数据设置的构建顺序。
            self.sync_bound_active_keys();
        } else {
            // 旧调用方继续由 expanded 初值建立状态。
            self.normalize_accordion();
        }
        self.transitions = Self::settled_transitions(&self.panels);
        self.content_opacities = Self::opacity_handles(&self.transitions);
        self
    }
    pub fn accordion(self) -> Self {
        // 兼容既有无参数手风琴构建器。
        self.accordion_enabled(true)
    }

    // 按布尔值启用或关闭手风琴模式。
    pub fn accordion_enabled(mut self, value: bool) -> Self {
        // 保存声明配置。
        self.accordion = value;
        // 受控模式不写回外部集合，只在界面选择首个有效 key。
        if self.active_keys_binding.is_some() {
            // 外部状态继续保持调用方所有。
            self.sync_bound_active_keys();
        } else {
            // 非受控模式归一化现有展开状态。
            self.normalize_accordion();
        }
        self.transitions = Self::settled_transitions(&self.panels);
        self.content_opacities = Self::opacity_handles(&self.transitions);
        self
    }

    // 将展开面板稳定 key 集合绑定到外部状态。
    pub fn active_keys(mut self, state: &State<Vec<String>>) -> Self {
        // 克隆轻量状态句柄供交互写回与依赖捕获使用。
        self.active_keys_binding = Some(state.clone());
        // 构造时立即按外部唯一事实同步。
        self.sync_bound_active_keys();
        // 首帧动画状态必须与同步后的面板一致。
        self.transitions = Self::settled_transitions(&self.panels);
        // 为每个面板重建匹配的内容透明度句柄。
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

    // 返回当前界面实际展开的稳定 key 集合。
    pub fn expanded_keys(&self) -> Vec<String> {
        // 只投影当前有效且实际展开的面板。
        self.panels
            // 按声明顺序遍历面板。
            .iter()
            // 忽略未展开项。
            .filter(|panel| panel.expanded)
            // 克隆稳定 key 供调用方观察。
            .map(|panel| panel.stable_key().to_owned())
            // 保持确定的声明顺序。
            .collect()
    }

    // 从外部 key 集合同步实际展开面板。
    pub(super) fn sync_bound_active_keys(&mut self) {
        // 未绑定时完整保留组件内部展开状态。
        let Some(active_keys) = self.active_keys_binding.as_ref().map(State::get) else {
            return;
        };
        // 使用集合加速匹配但不改变外部顺序与内容。
        let active_keys = active_keys
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        // 手风琴只允许界面采用首个声明顺序中的有效 key。
        let mut accordion_claimed = false;
        // 重复稳定 key 由首个面板取得显示所有权。
        let mut seen = std::collections::HashSet::new();
        // 逐项精确覆盖运行时展开状态。
        for panel in &mut self.panels {
            // 读取显式或兼容稳定身份。
            let key = panel.stable_key().to_owned();
            // 只有首个同 key 面板可以匹配外部状态。
            let unique_owner = seen.insert(key.clone());
            // 外部失效 key 自然不会展开任何面板。
            let requested = unique_owner && active_keys.contains(&key);
            // 手风琴模式采用首个有效请求，其余保持关闭。
            panel.expanded = requested && (!self.accordion || !accordion_claimed);
            // 记录手风琴是否已经取得一个展开项。
            accordion_claimed |= panel.expanded;
        }
    }

    // 把用户产生的展开集合原子写回外部状态。
    fn write_bound_active_keys(&self) {
        // 非受控模式不产生外部写入。
        let Some(state) = self.active_keys_binding.as_ref() else {
            return;
        };
        // 运行时已按手风琴与稳定 key 规则归一化实际状态。
        let active_keys = self.expanded_keys();
        // 避免重复发布相同集合。
        if state.get() != active_keys {
            // 状态写回发生在 Change 事件登记之前。
            state.set(active_keys);
        }
    }

    // 在绘制期登记受控展开集合的响应式依赖。
    pub(super) fn capture_bound_active_keys_dependency(&self) {
        // 只有受控模式需要触发声明视图重建。
        if let Some(state) = self.active_keys_binding.as_ref() {
            // 读取值即可由状态系统捕获当前组件依赖。
            let _ = state.get();
        }
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
        let key = self
            .panels
            .get(panel_index)
            .map(CollapsePanel::stable_key)
            .unwrap_or_default();
        let occurrence = self.panels[..panel_index.min(self.panels.len())]
            .iter()
            .filter(|panel| panel.stable_key() == key)
            .count();
        format!("uix:collapse-content:{}:{occurrence}:{key}", key.len())
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

    pub(super) fn content_views(
        &self,
        entries: &[CollapseContentEntry],
    ) -> Vec<crate::ui::view::ViewNode> {
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
        let focused_key = current_panels
            .get(self.focused_header)
            .map(|panel| panel.stable_key().to_owned());
        // 下一帧声明决定是否进入受控模式。
        let active_keys_binding = next.active_keys_binding;
        // 受控重建不采用旧内部 expanded 状态。
        let controlled = active_keys_binding.is_some();
        let mut panels = next.panels;
        let same_len = current_panels.len() == panels.len();
        let next_key_is_unique = panels
            .iter()
            .map(|panel| {
                panels
                    .iter()
                    .filter(|candidate| candidate.stable_key() == panel.stable_key())
                    .count()
                    == 1
            })
            .collect::<Vec<_>>();
        let mut used = vec![false; current_panels.len()];
        let mut transitions = Vec::with_capacity(panels.len());
        let mut content_opacities = Vec::with_capacity(panels.len());
        for (idx, panel) in panels.iter_mut().enumerate() {
            let key_is_unique = current_panels
                .iter()
                .filter(|current| current.stable_key() == panel.stable_key())
                .count()
                == 1
                && next_key_is_unique[idx];
            let matched = key_is_unique
                .then(|| {
                    current_panels
                        .iter()
                        .enumerate()
                        .find(|(current_idx, current)| {
                            !used[*current_idx] && current.stable_key() == panel.stable_key()
                        })
                        .map(|(current_idx, _)| current_idx)
                })
                .flatten()
                .or_else(|| same_len.then_some(idx).filter(|index| !used[*index]));
            if let Some(current_idx) = matched {
                used[current_idx] = true;
                // 非受控重建保留旧内部状态，受控重建采用下一声明的外部事实。
                if !controlled {
                    // 稳定 key 而不是易变索引拥有重建身份。
                    panel.expanded = current_panels[current_idx].expanded;
                }
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
        // 下一帧状态句柄替换旧绑定。
        self.active_keys_binding = active_keys_binding;
        self.borderless = next.borderless;
        self.destroy_on_hide = next.destroy_on_hide;
        let expanded_before_normalize = self
            .panels
            .iter()
            .map(|panel| panel.expanded)
            .collect::<Vec<_>>();
        // 受控模式只同步界面，非受控模式归一化内部初值。
        if controlled {
            // 外部无匹配或多 key 状态不被反向修改。
            self.sync_bound_active_keys();
        } else {
            // 保留旧手风琴兼容行为。
            self.normalize_accordion();
        }
        self.focused_header = focused_key
            .as_ref()
            .and_then(|key| {
                (self
                    .panels
                    .iter()
                    .filter(|panel| panel.stable_key() == key)
                    .count()
                    == 1)
                    .then(|| {
                        self.panels
                            .iter()
                            .position(|panel| panel.stable_key() == key)
                    })
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
        // 受控重建可能由外部 key 集合改变 expanded，需要立即对齐动画终态。
        if controlled {
            // 重建不反向播放旧内部动画。
            self.transitions = Self::settled_transitions(&self.panels);
        }
        self.content_opacities = content_opacities;
        // 受控终态重建对应的透明度句柄也必须重新对齐。
        if controlled {
            // 每个句柄与当前稳定面板顺序一一对应。
            self.content_opacities = Self::opacity_handles(&self.transitions);
        }
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
        // 用户产生的新集合先原子写回外部状态。
        self.write_bound_active_keys();
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
