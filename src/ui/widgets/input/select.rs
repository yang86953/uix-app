use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::native::traits::input::ControlSize;
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::foundation::virtual_scroll::VirtualListScroll;
use crate::ui::state::State;
use crate::ui::{
    ComponentId, EventResult, KeyCode, LayoutChild, MouseButton, SemanticEvent, SnapshotFields,
    SystemEvent, WidgetTree,
};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;

mod render;
mod search;

use self::search::VisibleRow;

const DROPDOWN_ROW_HEIGHT: f32 = 28.0;
const MAX_DROPDOWN_VIEWPORT_HEIGHT: f32 = 280.0;

pub(crate) type SelectOptionRenderer = Box<dyn Fn(&str) -> crate::ui::view::ViewNode>;

/// 选项组。
#[derive(Debug, Clone, PartialEq)]
pub struct OptGroup {
    pub label: String,
    pub options: Vec<String>,
}

impl OptGroup {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            options: Vec::new(),
        }
    }

    #[allow(
        clippy::should_implement_trait,
        reason = "add is the established fluent builder API, not arithmetic addition"
    )]
    pub fn add(mut self, opt: &str) -> Self {
        self.options.push(opt.to_string());
        self
    }
}

/// 使用一次性选项集合构造的 `Select` 分组。
///
/// 这是推荐的公开入口；`OptGroup` 保留给已有的逐项 `.add(...)` 写法。
#[derive(Debug, Clone, PartialEq)]
pub struct SelectOptionGroup {
    pub label: String,
    pub options: Vec<String>,
}

impl SelectOptionGroup {
    pub fn new<I, S>(label: impl Into<String>, options: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            label: label.into(),
            options: options
                .into_iter()
                .map(|option| option.as_ref().to_owned())
                .collect(),
        }
    }
}

impl From<SelectOptionGroup> for OptGroup {
    fn from(group: SelectOptionGroup) -> Self {
        Self {
            label: group.label,
            options: group.options,
        }
    }
}

/// 可绑定到 `Select` 的外部值类型。
pub trait SelectValue: Clone + PartialEq + Send + Sync + 'static {
    const MULTIPLE: bool;

    fn selected_indices(&self, options: &[&str]) -> Vec<usize>;
    fn from_selected_indices(options: &[&str], indices: &[usize]) -> Self;
}

impl SelectValue for String {
    const MULTIPLE: bool = false;

    fn selected_indices(&self, options: &[&str]) -> Vec<usize> {
        options
            .iter()
            .position(|option| *option == self)
            .into_iter()
            .collect()
    }

    fn from_selected_indices(options: &[&str], indices: &[usize]) -> Self {
        indices
            .first()
            .and_then(|index| options.get(*index))
            .copied()
            .unwrap_or_default()
            .to_owned()
    }
}

impl SelectValue for HashSet<String> {
    const MULTIPLE: bool = true;

    fn selected_indices(&self, options: &[&str]) -> Vec<usize> {
        options
            .iter()
            .enumerate()
            .filter_map(|(index, option)| self.contains(*option).then_some(index))
            .collect()
    }

    fn from_selected_indices(options: &[&str], indices: &[usize]) -> Self {
        indices
            .iter()
            .filter_map(|index| options.get(*index))
            .map(|option| (*option).to_owned())
            .collect()
    }
}

type ReadSelection = Box<dyn Fn(&[&str]) -> Vec<usize> + Send + Sync>;
type WriteSelection = Box<dyn Fn(&[&str], &[usize]) + Send + Sync>;

struct SelectValueBinding {
    multiple: bool,
    read: ReadSelection,
    write: WriteSelection,
    capture: Box<dyn Fn() + Send + Sync>,
}

impl SelectValueBinding {
    fn new<T: SelectValue>(state: &State<T>) -> Self {
        let read_state = state.clone();
        let write_state = state.clone();
        let capture_state = state.clone();
        Self {
            multiple: T::MULTIPLE,
            read: Box::new(move |options| read_state.get().selected_indices(options)),
            write: Box::new(move |options, indices| {
                let value = T::from_selected_indices(options, indices);
                if write_state.get() != value {
                    write_state.set(value);
                }
            }),
            capture: Box::new(move || {
                let _ = capture_state.get();
            }),
        }
    }
}

component! {
    pub struct Select {
        options: Vec<String>,
        optgroups: Vec<OptGroup>,
        selected: usize,
        selected_multi: Vec<usize>,
        value_binding: Option<SelectValueBinding>,
        open: bool,
        disabled: bool,
        loading: bool,
        loading_phase: f32,
        loading_dirty: bool,
        select_size: ControlSize,
        hovered: bool,
        focused: bool,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
        placeholder: String,
        hovered_option: Option<usize>,
        highlighted_option: Option<usize>,
        pending_change: RefCell<Option<String>>,
        multiple: bool,
        search: bool,
        search_query: String,
        custom_option_views: bool,
        materialized_custom_options: RefCell<Vec<usize>>,
        search_cursor_rect: Cell<Rect>,
        control_rect: Cell<Rect>,
        dropdown_rect: Cell<Rect>,
        multi_remove_rects: RefCell<Vec<(usize, Rect)>>,
        pub(crate) dropdown_scroll: VirtualListScroll,
        scroll_delta_strip: Cell<(f32, f32)>,
    }

    tab_index => (&self) -> i32 { 1 }

    accepts_text_input => (&self) -> bool { self.search && !self.disabled }

    text_input_cursor_rect => (&self) -> Rect { self.search_cursor_rect.get() }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        let rows = self.visible_rows();
        let popup = self.dropdown_damage_rect();
        let list_y = frame.y + popup.y;
        let text_left = if self.multiple { 32.0 } else { 10.0 };
        let content_width = (frame.w - text_left - 32.0).max(0.0);
        let indices = self.materialized_custom_options.borrow();
        children
            .iter()
            .zip(indices.iter().copied())
            .filter_map(|(child, option_index)| {
                let row_index = rows.iter().position(
                    |row| matches!(row, VisibleRow::Option(index) if *index == option_index),
                )?;
                Some((
                    child.id,
                    Rect::new(
                        frame.x + text_left,
                        list_y + row_index as f32 * DROPDOWN_ROW_HEIGHT
                            - self.dropdown_scroll.scroll_offset(),
                        content_width,
                        DROPDOWN_ROW_HEIGHT,
                    ),
                ))
            })
            .collect()
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        (self.custom_option_views && self.is_present()).then(|| {
            let popup = self.dropdown_damage_rect();
            Rect::new(frame.x + popup.x, frame.y + popup.y, popup.w, popup.h)
        })
    }

    hit_test_children => (&self) -> bool { false }

    scroll_delta_for_dirty => (&self) -> Option<(f32, f32)> {
        let delta = self.scroll_delta_strip.get();
        if delta.0.abs() > 0.01 || delta.1.abs() > 0.01 {
            self.scroll_delta_strip.set((0.0, 0.0));
            Some(delta)
        } else {
            None
        }
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if self.disabled {
            return EventResult::NotHandled;
        }
        self.sync_bound_selection();

        let visible_options = self.visible_option_indices();
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if self.control_rect.get().contains(*pos) {
                    if self.multiple {
                        let remove = self
                            .multi_remove_rects
                            .borrow()
                            .iter()
                            .find_map(|(index, rect)| rect.contains(*pos).then_some(*index));
                        if let Some(index) = remove {
                            if let Some(position) = self
                                .selected_multi
                                .iter()
                                .position(|selected| *selected == index)
                            {
                                self.selected_multi.remove(position);
                                self.publish_multi_change();
                            }
                            return EventResult::Handled;
                        }
                    }
                    if self.open {
                        self.close();
                    } else {
                        self.open();
                    }
                    self.hovered_option = None;
                    return EventResult::Handled;
                }

                if self.is_present() && self.dropdown_rect.get().contains(*pos) {
                    if let Some(flat_idx) = self.dropdown_row_at_y(pos.y) {
                        if let Some(opt_idx) = self.flat_row_option_index(flat_idx) {
                            self.highlighted_option = Some(opt_idx);
                            if self.multiple {
                                if let Some(multi_idx) =
                                    self.selected_multi.iter().position(|&i| i == opt_idx)
                                {
                                    self.selected_multi.remove(multi_idx);
                                } else {
                                    self.selected_multi.push(opt_idx);
                                }
                                self.publish_multi_change();
                            } else {
                                self.select_single(opt_idx);
                                self.close();
                            }
                            return EventResult::Handled;
                        }
                    }
                    return EventResult::Handled;
                }

                self.close();
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if self.is_present() && self.dropdown_rect.get().contains(*pos) {
                    self.hovered_option = self.dropdown_row_at_y(pos.y);
                } else {
                    self.hovered_option = None;
                }
                self.hovered = self.control_rect.get().contains(*pos);
                EventResult::Handled
            }
            SystemEvent::PointerEnter => {
                self.hovered = true;
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                self.hovered = false;
                self.hovered_option = None;
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.close();
                EventResult::Handled
            }
            SystemEvent::Wheel { delta, pos, .. } => {
                if self.is_present() && self.dropdown_rect.get().contains(*pos) {
                    let row_count = self.dropdown_row_count();
                    let viewport_h = self.dropdown_viewport_height(row_count);
                    let dy = self.dropdown_scroll.scroll_by_wheel(
                        delta.y,
                        row_count,
                        DROPDOWN_ROW_HEIGHT,
                        viewport_h,
                    );
                    if dy.abs() > 0.01 {
                        self.push_scroll_delta(0.0, dy);
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Down => {
                    if self.open {
                        if self.search || self.multiple {
                            self.move_highlight(true);
                        } else {
                            let position = visible_options
                                .iter()
                                .position(|index| *index == self.selected);
                            let next = match position {
                                Some(position) => visible_options.get(position + 1),
                                None => visible_options.first(),
                            }
                            .copied();
                            if let Some(next) = next {
                                self.select_single(next);
                            }
                        }
                    } else {
                        self.open();
                    }
                    EventResult::Handled
                }
                KeyCode::Up => {
                    if self.open && !visible_options.is_empty() {
                        if self.search || self.multiple {
                            self.move_highlight(false);
                        } else {
                            let position = visible_options
                                .iter()
                                .position(|index| *index == self.selected);
                            let prev = match position {
                                Some(position) => {
                                    visible_options.get(position.saturating_sub(1))
                                }
                                None => visible_options.last(),
                            }
                            .copied()
                            .unwrap_or(self.selected);
                            self.select_single(prev);
                        }
                    }
                    EventResult::Handled
                }
                KeyCode::Enter => {
                    if self.open {
                        if self.multiple {
                            self.toggle_highlighted_multi();
                        } else if self.search {
                            let selection = self
                                .highlighted_option
                                .filter(|index| visible_options.contains(index))
                                .or_else(|| visible_options.first().copied());
                            if let Some(index) = selection {
                                self.select_single(index);
                            }
                            self.close();
                        } else {
                            self.close();
                        }
                    } else {
                        self.open();
                    }
                    EventResult::Handled
                }
                KeyCode::Space if self.search => EventResult::NotHandled,
                KeyCode::Space => {
                    if self.open {
                        if self.multiple {
                            self.toggle_highlighted_multi();
                        } else {
                            self.close();
                        }
                    } else {
                        self.open();
                    }
                    EventResult::Handled
                }
                KeyCode::Escape => {
                    self.close();
                    EventResult::Handled
                }
                KeyCode::Backspace => {
                    if self.search && !self.search_query.is_empty() {
                        self.search_query.pop();
                        self.refresh_search_results();
                    } else if self.multiple && !self.selected_multi.is_empty() {
                        self.selected_multi.pop();
                        self.publish_multi_change();
                    }
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            SystemEvent::TextInput { text } if self.search => {
                if text.is_empty() || text.chars().any(char::is_control) {
                    return EventResult::NotHandled;
                }
                self.search_query.push_str(text);
                self.refresh_search_results();
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|value| SemanticEvent::change(id, value))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.is_present() {
            select_dirty_rect(frame, self.dropdown_damage_rect())
        } else {
            frame
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.render_select(frame, ctx);
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        select_dirty_rect(frame, self.dropdown_damage_rect())
    }

    overlay_entry => (&self, id: ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        if !self.is_present() {
            return None;
        }

        Some(
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                .bounds(select_popup_rect(frame, self.dropdown_damage_rect()))
                .z_index(900),
        )
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !self.is_present() {
            self.transition_dirty = false;
            self.loading_dirty = false;
            return false;
        }

        let transition_active = if self.transition.finished {
            if self.closing {
                self.open = false;
                self.closing = false;
                self.search_query.clear();
                self.highlighted_option = None;
            }
            self.transition_dirty = false;
            false
        } else {
            self.transition.update(dt);
            self.transition_dirty = true;

            if self.closing && self.transition.finished {
                self.open = false;
                self.closing = false;
                self.search_query.clear();
                self.highlighted_option = None;
            }

            self.is_present() && !self.transition.finished
        };

        self.loading_dirty = false;
        let loading_active = self.loading && self.is_present();
        if loading_active {
            let before = self.loading_phase;
            self.loading_phase = (self.loading_phase
                + dt.max(0.0) as f32 * std::f32::consts::TAU / 0.8)
                .rem_euclid(std::f32::consts::TAU);
            self.loading_dirty = (self.loading_phase - before).abs() > f32::EPSILON;
        }

        transition_active || loading_active
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty || self.loading_dirty {
            select_dirty_rect(frame, self.dropdown_damage_rect())
        } else {
            Rect::zero()
        }
    }
}

impl Select {
    fn control_height(&self) -> f32 {
        crate::ui::config::control_height(self.select_size)
    }

    fn intrinsic_size(&self) -> Size {
        let mut max_text_width = crate::draw::font::text_backend::estimate_text_metrics(
            &self.placeholder,
            f32::INFINITY,
            13.0,
        )
        .max_line_width;
        for option in &self.options {
            max_text_width = max_text_width.max(
                crate::draw::font::text_backend::estimate_text_metrics(option, f32::INFINITY, 13.0)
                    .max_line_width,
            );
        }
        for group in &self.optgroups {
            max_text_width = max_text_width.max(
                crate::draw::font::text_backend::estimate_text_metrics(
                    &group.label,
                    f32::INFINITY,
                    12.0,
                )
                .max_line_width,
            );
            for option in &group.options {
                max_text_width = max_text_width.max(
                    crate::draw::font::text_backend::estimate_text_metrics(
                        option,
                        f32::INFINITY,
                        13.0,
                    )
                    .max_line_width,
                );
            }
        }
        Size::new((max_text_width + 40.0).max(120.0), self.control_height())
    }

    fn push_scroll_delta(&self, dx: f32, dy: f32) {
        if dx.abs() <= 0.01 && dy.abs() <= 0.01 {
            return;
        }
        let current = self.scroll_delta_strip.get();
        self.scroll_delta_strip
            .set((current.0 + dx, current.1 + dy));
    }

    fn refresh_search_results(&mut self) {
        self.hovered_option = None;
        self.dropdown_scroll.set_scroll_offset(0.0);
        self.highlighted_option = self.visible_option_indices().first().copied();
        if !self.open {
            self.open();
        } else {
            self.transition_dirty = true;
        }
    }

    fn selected_multi_payload(&self) -> String {
        self.selected_multi
            .iter()
            .map(|idx| idx.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }

    fn sync_bound_selection(&mut self) {
        let Some(binding) = self.value_binding.as_ref() else {
            return;
        };
        let multiple = binding.multiple;
        let selected = {
            let options = self.all_options();
            (binding.read)(&options)
        };
        if multiple {
            self.selected_multi = selected;
        } else {
            self.selected = selected.first().copied().unwrap_or(usize::MAX);
        }
    }

    fn capture_bound_value_dependency(&self) {
        if let Some(binding) = self.value_binding.as_ref() {
            (binding.capture)();
        }
    }

    fn write_bound_selection(&self) {
        let Some(binding) = self.value_binding.as_ref() else {
            return;
        };
        let options = self.all_options();
        if binding.multiple {
            (binding.write)(&options, &self.selected_multi);
        } else {
            (binding.write)(&options, &[self.selected]);
        }
    }

    fn select_single(&mut self, index: usize) {
        self.highlighted_option = Some(index);
        if self.selected == index {
            return;
        }
        self.selected = index;
        self.write_bound_selection();
        self.pending_change.replace(Some(index.to_string()));
    }

    fn publish_multi_change(&self) {
        self.write_bound_selection();
        self.pending_change
            .replace(Some(self.selected_multi_payload()));
    }

    fn move_highlight(&mut self, forward: bool) {
        let visible = self.visible_option_indices();
        if visible.is_empty() {
            self.highlighted_option = None;
            return;
        }
        let current = self
            .highlighted_option
            .and_then(|index| visible.iter().position(|visible| *visible == index));
        let position = match (current, forward) {
            (Some(position), true) => (position + 1).min(visible.len() - 1),
            (Some(position), false) => position.saturating_sub(1),
            (None, true) => 0,
            (None, false) => visible.len() - 1,
        };
        let option_index = visible[position];
        self.highlighted_option = Some(option_index);
        self.reveal_option(option_index);
    }

    fn toggle_highlighted_multi(&mut self) {
        let Some(option_index) = self
            .highlighted_option
            .or_else(|| self.visible_option_indices().first().copied())
        else {
            return;
        };
        if let Some(position) = self
            .selected_multi
            .iter()
            .position(|selected| *selected == option_index)
        {
            self.selected_multi.remove(position);
        } else {
            self.selected_multi.push(option_index);
        }
        self.highlighted_option = Some(option_index);
        self.publish_multi_change();
    }

    fn reveal_option(&mut self, option_index: usize) {
        let visible_rows = self.visible_rows();
        let Some(row_index) = visible_rows
            .iter()
            .position(|row| matches!(row, VisibleRow::Option(index) if *index == option_index))
        else {
            return;
        };
        let row_count = self.dropdown_row_count();
        let viewport_height = self.dropdown_viewport_height(row_count);
        let old_offset = self.dropdown_scroll.scroll_offset();
        let row_top = row_index as f32 * DROPDOWN_ROW_HEIGHT;
        let row_bottom = row_top + DROPDOWN_ROW_HEIGHT;
        let new_offset = if row_top < old_offset {
            row_top
        } else if row_bottom > old_offset + viewport_height {
            row_bottom - viewport_height
        } else {
            old_offset
        };
        self.dropdown_scroll.set_scroll_offset(new_offset);
        self.dropdown_scroll
            .clamp_to_content(row_count, DROPDOWN_ROW_HEIGHT, viewport_height);
        let applied = self.dropdown_scroll.scroll_offset() - old_offset;
        if applied.abs() > 0.01 {
            self.push_scroll_delta(0.0, applied);
        }
    }

    fn dropdown_damage_rect(&self) -> Rect {
        let control = self.control_rect.get();
        let height = self.dropdown_viewport_height(self.dropdown_damage_row_count());
        let y = if self.dropdown_rect.get().y < 0.0 {
            -height
        } else {
            control.h
        };
        Rect::new(0.0, y, control.w, height)
    }

    pub(crate) fn custom_option_indices(&self) -> Vec<usize> {
        if !self.custom_option_views || !self.is_present() || self.loading {
            return Vec::new();
        }
        let rows = self.visible_rows();
        let row_count = self.dropdown_row_count();
        let viewport_height = self.dropdown_viewport_height(row_count);
        let (start, end) =
            self.dropdown_scroll
                .scroll_range(row_count, DROPDOWN_ROW_HEIGHT, viewport_height);
        rows[start.min(rows.len())..end.min(rows.len())]
            .iter()
            .filter_map(|row| match row {
                VisibleRow::Option(index) => Some(*index),
                VisibleRow::Group(_) => None,
            })
            .collect()
    }

    pub(crate) fn custom_option_labels(&self, indices: &[usize]) -> Vec<String> {
        indices
            .iter()
            .filter_map(|index| self.option_label(*index).map(str::to_owned))
            .collect()
    }

    pub(crate) fn needs_custom_option_refresh(
        &self,
        indices: &[usize],
        child_count: usize,
    ) -> bool {
        self.materialized_custom_options.borrow().as_slice() != indices
            || child_count != indices.len()
    }

    pub(crate) fn mark_custom_options_materialized(&self, indices: Vec<usize>) {
        *self.materialized_custom_options.borrow_mut() = indices;
    }

    pub(crate) fn invalidate_custom_option_materialization(&self) {
        self.materialized_custom_options.borrow_mut().clear();
    }
}

impl Default for Select {
    fn default() -> Self {
        Self::new()
    }
}

impl Select {
    pub fn new() -> Self {
        let config = crate::ui::config::use_config();
        let control_height = crate::ui::config::control_height(config.size);
        Self {
            options: Vec::new(),
            optgroups: Vec::new(),
            selected: 0,
            selected_multi: Vec::new(),
            value_binding: None,
            open: false,
            disabled: config.disabled,
            loading: false,
            loading_phase: 0.0,
            loading_dirty: false,
            select_size: config.size,
            hovered: false,
            focused: false,
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
            placeholder: String::new(),
            hovered_option: None,
            highlighted_option: None,
            pending_change: RefCell::new(None),
            multiple: false,
            search: config.overrides.select.allow_search.unwrap_or(false),
            search_query: String::new(),
            custom_option_views: false,
            materialized_custom_options: RefCell::new(Vec::new()),
            search_cursor_rect: Cell::new(Rect::zero()),
            control_rect: Cell::new(Rect::new(0.0, 0.0, 120.0, control_height)),
            dropdown_rect: Cell::new(Rect::zero()),
            multi_remove_rects: RefCell::new(Vec::new()),
            dropdown_scroll: VirtualListScroll::new(),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
        }
    }

    /// 创建多选选择器。
    pub fn multiple() -> Self {
        let mut select = Self::new();
        select.multiple = true;
        select
    }

    /// 创建可搜索的单选选择器。
    pub fn searchable() -> Self {
        let mut select = Self::new();
        select.search = true;
        select
    }

    pub fn options<I, S>(mut self, opts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.options = opts
            .into_iter()
            .map(|option| option.as_ref().to_owned())
            .collect();
        self.sync_bound_selection();
        self
    }

    /// Render each dropdown option with an arbitrary View while preserving Select interaction.
    pub fn render_option<F, V>(mut self, renderer: F) -> SelectOptionView
    where
        F: Fn(&str) -> V + 'static,
        V: crate::ui::view::View,
    {
        self.custom_option_views = true;
        SelectOptionView {
            select: self,
            renderer: Box::new(move |option| crate::ui::view::View::build(renderer(option))),
        }
    }

    pub fn optgroups(mut self, groups: Vec<OptGroup>) -> Self {
        self.optgroups = groups;
        self.sync_bound_selection();
        self
    }

    /// 设置分组选项；组标题不可选择，组内选项按声明顺序形成值索引。
    pub fn option_groups<I>(mut self, groups: I) -> Self
    where
        I: IntoIterator<Item = SelectOptionGroup>,
    {
        self.optgroups = groups.into_iter().map(OptGroup::from).collect();
        self.sync_bound_selection();
        self
    }

    /// 下拉展开时用加载旋转器替代候选项，并暂停选项提交。
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        if loading {
            self.hovered_option = None;
            self.highlighted_option = None;
        } else {
            self.loading_phase = 0.0;
            self.loading_dirty = false;
        }
        self
    }

    /// 设置非受控选择器的初始索引。
    pub fn default_selected(mut self, idx: usize) -> Self {
        self.value_binding = None;
        self.selected = idx;
        self
    }

    /// 将选择值绑定到外部 State；支持 `String` 单选与 `HashSet<String>` 多选。
    pub fn value<T: SelectValue>(mut self, state: &State<T>) -> Self {
        self.value_binding = Some(SelectValueBinding::new(state));
        self.multiple = T::MULTIPLE;
        self.sync_bound_selection();
        self
    }

    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }

    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.select_size = size;
        let control = self.control_rect.get();
        self.control_rect.set(Rect::new(
            control.x,
            control.y,
            control.w,
            self.control_height(),
        ));
        self
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_present(&self) -> bool {
        self.open || self.closing
    }

    pub fn current_value(&self) -> Option<String> {
        self.all_options()
            .get(self.selected)
            .map(|value| (*value).to_owned())
    }

    pub fn current_values(&self) -> HashSet<String> {
        let options = self.all_options();
        self.selected_multi
            .iter()
            .filter_map(|index| options.get(*index))
            .map(|value| (*value).to_owned())
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn first_multi_remove_rect(&self) -> Option<Rect> {
        self.multi_remove_rects
            .borrow()
            .first()
            .map(|(_, rect)| *rect)
    }

    pub fn open(&mut self) {
        if self.closing {
            self.search_query.clear();
            self.hovered_option = None;
        }
        self.open = true;
        self.closing = false;
        self.dropdown_scroll.set_scroll_offset(0.0);
        let control = self.control_rect.get();
        let row_count = self.dropdown_row_count();
        self.dropdown_rect.set(Rect::new(
            0.0,
            control.h,
            control.w,
            self.dropdown_viewport_height(row_count),
        ));
        let visible = self.visible_option_indices();
        self.highlighted_option = visible
            .contains(&self.selected)
            .then_some(self.selected)
            .or_else(|| visible.first().copied());
        self.transition = TransitionPlayer::new(presets::tooltip_enter());
        self.transition_dirty = true;
    }

    pub fn close(&mut self) {
        if !self.is_present() {
            self.open = false;
            self.closing = false;
            self.transition_dirty = false;
            return;
        }

        self.open = false;
        self.closing = true;
        self.dropdown_scroll.set_scroll_offset(0.0);
        self.transition = TransitionPlayer::new(presets::tooltip_exit());
        self.transition_dirty = true;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Select {
            options: self.options.clone(),
            optgroups: self.optgroups.clone(),
            selected: self.selected,
            selected_multi: self.selected_multi.clone(),
            open: self.open,
            disabled: self.disabled,
            loading: self.loading,
            placeholder: self.placeholder.clone(),
            multiple: self.multiple,
            search: self.search,
            search_query: self.search_query.clone(),
            custom_options: self.custom_option_views,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_selection = next
            .value_binding
            .as_ref()
            .map(|_| (next.selected, next.selected_multi.clone()));
        self.options = next.options;
        self.optgroups = next.optgroups;
        self.value_binding = next.value_binding;
        self.disabled = next.disabled;
        self.loading = next.loading;
        if self.loading {
            self.hovered_option = None;
        } else {
            self.loading_phase = 0.0;
            self.loading_dirty = false;
        }
        self.select_size = next.select_size;
        self.placeholder = next.placeholder;
        self.multiple = next.multiple;
        self.search = next.search;
        self.custom_option_views = next.custom_option_views;
        if !self.custom_option_views {
            self.materialized_custom_options.borrow_mut().clear();
        }
        if !self.search {
            self.search_query.clear();
        }
        if let Some((selected, selected_multi)) = controlled_selection {
            self.selected = selected;
            self.selected_multi = selected_multi;
        }
        let row_count = self.dropdown_row_count();
        self.dropdown_scroll.clamp_to_content(
            row_count,
            DROPDOWN_ROW_HEIGHT,
            self.dropdown_viewport_height(row_count),
        );
        let popup = self.dropdown_rect.get();
        let popup_height = self.dropdown_viewport_height(row_count);
        self.dropdown_rect.set(Rect::new(
            0.0,
            if popup.y < 0.0 {
                -popup_height
            } else {
                self.control_rect.get().h
            },
            self.control_rect.get().w,
            popup_height,
        ));
        let visible = self.visible_option_indices();
        if self
            .highlighted_option
            .is_some_and(|index| !visible.contains(&index))
        {
            self.highlighted_option = visible.first().copied();
        }
    }
}

/// Select builder returned by [`Select::render_option`].
pub struct SelectOptionView {
    select: Select,
    renderer: SelectOptionRenderer,
}

impl crate::ui::view::View for SelectOptionView {
    fn build(self) -> crate::ui::view::ViewNode {
        let mut node = crate::ui::view::ViewNode::leaf(self.select);
        node.render_handlers.push(
            crate::ui::render_handler::RenderHandlerRegistration::SelectOptions(self.renderer),
        );
        node
    }
}

impl From<SelectOptionView> for crate::ui::view::ViewNode {
    fn from(view: SelectOptionView) -> Self {
        crate::ui::view::View::build(view)
    }
}

impl crate::ui::IntoWidgetNode for SelectOptionView {
    fn into_node(self) -> crate::ui::core::widget::WidgetNode {
        crate::ui::IntoWidgetNode::into_node(crate::ui::view::View::build(self))
    }
}

fn select_dirty_rect(frame: Rect, popup: Rect) -> Rect {
    let list = select_popup_rect(frame, popup);
    let expanded = frame.union(&list);
    let expand = 8.0;
    Rect::new(
        expanded.x - expand,
        expanded.y - expand,
        expanded.w + expand * 2.0,
        expanded.h + expand * 2.0,
    )
}

fn select_popup_rect(frame: Rect, popup: Rect) -> Rect {
    Rect::new(frame.x + popup.x, frame.y + popup.y, popup.w, popup.h)
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}
