//! Segmented widget — 分段选择器，支持 disabled/hover/keyboard/focus。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Radius;
use crate::platform::windowing::ControlSize;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::reactive::state::State;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent,
    WidgetTree,
};
use std::cell::Cell;

component! {
    /// Segmented — 水平分段选择器。
    pub struct Segmented {
        options: Vec<String>,
        selected: usize,
        value_binding: Option<State<String>>,
        disabled: bool,
        disabled_options: Vec<bool>,
        segmented_size: ControlSize,
        hovered_idx: Option<usize>,
        focused: bool,
        pending_change: Cell<Option<usize>>,
        control_rect: Cell<Rect>,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_value();
        if matches!(event, SystemEvent::FocusOut) {
            self.focused = false;
            return EventResult::Handled;
        }
        if matches!(event, SystemEvent::PointerLeave) {
            let changed = self.hovered_idx.take().is_some();
            return if changed {
                EventResult::Handled
            } else {
                EventResult::NotHandled
            };
        }
        if self.disabled { return EventResult::NotHandled; }
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(idx) = self.segment_at(*pos) {
                    if self.select_index(idx) {
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                let next = self
                    .segment_at(*pos)
                    .filter(|index| !self.is_segment_disabled(*index));
                if self.hovered_idx == next {
                    EventResult::NotHandled
                } else {
                    self.hovered_idx = next;
                    EventResult::Handled
                }
            }
            SystemEvent::PointerEnter => { EventResult::NotHandled }
            SystemEvent::FocusIn => { self.focused = true; EventResult::Handled }
            SystemEvent::KeyDown { key, .. } => {
                match key {
                    KeyCode::Right | KeyCode::Down => {
                        self.move_selection(true);
                        EventResult::Handled
                    }
                    KeyCode::Left | KeyCode::Up => {
                        self.move_selection(false);
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|idx| SemanticEvent::change(id, idx.to_string()))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        self.capture_bound_value_dependency();
        let control_height = frame.h.max(0.0).min(self.control_height());
        let control_rect = Rect::new(frame.x, frame.y, frame.w.max(0.0), control_height);
        self.control_rect
            .set(Rect::new(0.0, 0.0, control_rect.w, control_rect.h));
        if control_rect.w <= 0.0 || control_rect.h <= 0.0 {
            return;
        }

        let fill = ctx.tokens().color_fill_tertiary();
        let fill_secondary = ctx.tokens().color_fill_secondary();
        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let text_secondary = ctx.tokens().color_text_secondary();
        let text_quaternary = ctx.tokens().color_text_quaternary();
        let bg = ctx.tokens().color_bg_elevated();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        let fill_quaternary = ctx.tokens().color_fill_quaternary();

        // 整体背景
        let container_bg = if self.disabled { fill_quaternary } else { fill };
        ctx.fill_rect(control_rect, container_bg, r);
        ctx.push_clip(control_rect);

        let mut x = control_rect.x;
        let visual_scale = Self::visual_scale_for_height(control_rect.h);
        let font_size = Self::font_size_for_height(control_rect.h);
        let segment_widths = self.segment_widths_for_frame(control_rect.h, control_rect.w);

        for (i, (opt, seg_w)) in self.options.iter().zip(segment_widths).enumerate() {
            let seg_disabled = self.is_segment_disabled(i);
            let is_hovered = self.hovered_idx == Some(i) && !seg_disabled;
            let segment_rect = Rect::new(x, control_rect.y, seg_w, control_rect.h);

            if i == self.selected {
                // 选中项：白色背景 + 主色文字
                let thumb_bg = if seg_disabled { fill_secondary } else { bg };
                let inset = 2.0 * visual_scale;
                ctx.fill_rect(
                    Rect::new(
                        x + inset,
                        control_rect.y + inset,
                        (seg_w - 2.0 * inset).max(0.0),
                        (control_rect.h - 2.0 * inset).max(0.0),
                    ),
                    thumb_bg,
                    Some(Radius::uniform(3.0 * visual_scale)),
                );
                let tc = if seg_disabled { text_quaternary } else { primary };
                ctx.push_clip(segment_rect);
                ctx.text_center(opt, segment_rect, tc, font_size);
                ctx.pop_clip();
            } else if seg_disabled {
                ctx.push_clip(segment_rect);
                ctx.text_center(opt, segment_rect, text_quaternary, font_size);
                ctx.pop_clip();
            } else if is_hovered {
                ctx.push_clip(segment_rect);
                ctx.text_center(opt, segment_rect, primary_hover, font_size);
                ctx.pop_clip();
            } else {
                ctx.push_clip(segment_rect);
                ctx.text_center(opt, segment_rect, text_secondary, font_size);
                ctx.pop_clip();
            }

            // 分隔线（非选中项之间）
            if i > 0 && i != self.selected && i - 1 != self.selected {
                let divider_color = ctx.tokens().color_border_secondary();
                let inset = 6.0 * visual_scale;
                ctx.draw_line(
                    x,
                    control_rect.y + inset,
                    x,
                    control_rect.y + control_rect.h - inset,
                    divider_color,
                    1.0,
                );
            }

            x += seg_w;
        }
        ctx.pop_clip();

        // focus 边框指示
        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(control_rect, primary, 1.5, r);
        }
    }
}

impl Segmented {
    fn select_index(&mut self, index: usize) -> bool {
        if index >= self.options.len() || self.is_segment_disabled(index) {
            return false;
        }
        if self.selected != index {
            self.selected = index;
            self.write_bound_value();
            self.pending_change.set(Some(index));
        }
        true
    }

    fn move_selection(&mut self, forward: bool) {
        let len = self.options.len();
        if len == 0 {
            return;
        }
        let start = if self.selected < len {
            if forward {
                (self.selected + 1) % len
            } else {
                (self.selected + len - 1) % len
            }
        } else if forward {
            0
        } else {
            len - 1
        };
        for offset in 0..len {
            let index = if forward {
                (start + offset) % len
            } else {
                (start + len - offset) % len
            };
            if !self.is_segment_disabled(index) {
                self.select_index(index);
                break;
            }
        }
    }

    fn sync_bound_value(&mut self) {
        let Some(value) = self.value_binding.as_ref().map(State::get) else {
            return;
        };
        self.selected = self
            .options
            .iter()
            .position(|option| option == &value)
            .unwrap_or(usize::MAX);
    }

    fn capture_bound_value_dependency(&self) {
        if let Some(state) = self.value_binding.as_ref() {
            let _ = state.get();
        }
    }

    fn write_bound_value(&self) {
        let Some(state) = self.value_binding.as_ref() else {
            return;
        };
        let Some(value) = self.options.get(self.selected) else {
            return;
        };
        if state.get() != *value {
            state.set(value.clone());
        }
    }

    fn intrinsic_size(&self) -> Size {
        if self.options.is_empty() {
            return Size::new(0.0, self.control_height());
        }
        let w = self
            .options
            .iter()
            .map(|option| Self::segment_width_for_height(option, self.control_height()))
            .sum::<f32>();
        Size::new(w, self.control_height())
    }

    fn segment_at(&self, pos: Point) -> Option<usize> {
        let control = self.control_rect.get();
        if pos.x < control.x
            || pos.x >= control.x + control.w
            || pos.y < control.y
            || pos.y >= control.y + control.h
        {
            return None;
        }
        let segment_widths = self.segment_widths_for_frame(control.h, control.w);
        let mut cum_x = 0.0f32;
        for (i, seg_w) in segment_widths.into_iter().enumerate() {
            let end = cum_x + seg_w;
            if pos.x >= cum_x && pos.x < end {
                return Some(i);
            }
            cum_x += seg_w;
        }
        None
    }

    fn is_segment_disabled(&self, idx: usize) -> bool {
        self.disabled || self.disabled_options.get(idx).copied().unwrap_or(false)
    }

    fn control_height(&self) -> f32 {
        crate::ui::component::config::control_height(self.segmented_size)
    }

    fn visual_scale_for_height(height: f32) -> f32 {
        (height / crate::ui::component::config::control_height(ControlSize::Medium)).max(0.0)
    }

    fn font_size_for_height(height: f32) -> f32 {
        13.0 * Self::visual_scale_for_height(height).sqrt()
    }

    fn segment_width_for_height(option: &str, height: f32) -> f32 {
        let scale = Self::visual_scale_for_height(height);
        let font_size = Self::font_size_for_height(height);
        crate::draw::resources::font::text_backend::estimate_text_metrics(
            option,
            f32::INFINITY,
            font_size,
        )
        .max_line_width
            + 24.0 * scale
    }

    fn segment_widths_for_frame(&self, height: f32, frame_width: f32) -> Vec<f32> {
        if self.options.is_empty() || frame_width <= 0.0 {
            return Vec::new();
        }
        let nominal = self
            .options
            .iter()
            .map(|option| Self::segment_width_for_height(option, height))
            .collect::<Vec<_>>();
        let total = nominal.iter().sum::<f32>();
        if total <= 0.0 {
            return vec![frame_width / self.options.len() as f32; self.options.len()];
        }

        let ratio = frame_width / total;
        let last = nominal.len() - 1;
        let mut used = 0.0;
        nominal
            .into_iter()
            .enumerate()
            .map(|(index, width)| {
                let width = if index == last {
                    (frame_width - used).max(0.0)
                } else {
                    (width * ratio).max(0.0)
                };
                used += width;
                width
            })
            .collect()
    }

    fn reset_nominal_geometry(&self) {
        let size = self.intrinsic_size();
        self.control_rect.set(Rect::new(0.0, 0.0, size.w, size.h));
    }
}

impl Default for Segmented {
    fn default() -> Self {
        Self::new(Vec::<String>::new())
    }
}

impl Segmented {
    /// 创建按声明顺序持有选项且默认选择首项的分段选择器。
    pub fn new<I, S>(options: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let config = crate::ui::component::config::use_config();
        let options = options
            .into_iter()
            .map(|option| option.as_ref().to_owned())
            .collect::<Vec<_>>();
        let control_height = crate::ui::component::config::control_height(config.size);
        let control_width = options
            .iter()
            .map(|option| Self::segment_width_for_height(option, control_height))
            .sum();
        Self {
            options,
            selected: 0,
            value_binding: None,
            disabled: false,
            disabled_options: Vec::new(),
            segmented_size: config.size,
            hovered_idx: None,
            focused: false,
            pending_change: Cell::new(None),
            control_rect: Cell::new(Rect::new(0.0, 0.0, control_width, control_height)),
        }
    }

    /// 替换全部选项，并按受控值重新解析当前索引与固有尺寸。
    pub fn options<I, S>(mut self, options: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.options = options
            .into_iter()
            .map(|option| option.as_ref().to_owned())
            .collect();
        self.sync_bound_value();
        self.reset_nominal_geometry();
        self
    }

    /// 设置非受控分段选择器的初始索引。
    pub fn default_selected(mut self, idx: usize) -> Self {
        self.value_binding = None;
        self.selected = idx;
        self
    }

    /// 将当前选项值绑定到外部 `State<String>`。
    pub fn value(mut self, state: &State<String>) -> Self {
        self.value_binding = Some(state.clone());
        self.sync_bound_value();
        self
    }

    /// 返回当前索引对应的拥有型选项值；索引无效时返回 `None`。
    pub fn current_value(&self) -> Option<String> {
        self.options.get(self.selected).cloned()
    }

    /// 返回当前有效选项索引。
    pub fn current_index(&self) -> Option<usize> {
        (self.selected < self.options.len()).then_some(self.selected)
    }

    /// 设置整个分段选择器是否禁用交互。
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    /// 设置分段项采用的控件尺寸规格并重算固有尺寸。
    pub fn size(mut self, size: ControlSize) -> Self {
        self.segmented_size = size;
        self.reset_nominal_geometry();
        self
    }
    /// 将指定索引的选项标记为不可交互。
    pub fn disable_option(mut self, idx: usize) -> Self {
        while self.disabled_options.len() <= idx {
            self.disabled_options.push(false);
        }
        self.disabled_options[idx] = true;
        self
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Segmented {
            options: self.options.clone(),
            selected: self.selected,
            disabled: self.disabled,
            disabled_options: self.disabled_options.clone(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_selected = next.value_binding.as_ref().map(|_| next.selected);
        let previous_selected = self.selected;
        let previous_value = self.options.get(previous_selected).cloned();
        self.options = next.options;
        self.value_binding = next.value_binding;
        self.disabled = next.disabled;
        self.disabled_options = next.disabled_options;
        self.segmented_size = next.segmented_size;
        let preserved_selection = previous_value
            .and_then(|value| self.options.iter().position(|option| option == &value))
            .or_else(|| (previous_selected < self.options.len()).then_some(previous_selected))
            .unwrap_or(usize::MAX);
        self.selected = controlled_selected.unwrap_or(preserved_selection);
        if self.disabled {
            self.hovered_idx = None;
            self.focused = false;
        } else if self
            .hovered_idx
            .is_some_and(|index| index >= self.options.len())
        {
            self.hovered_idx = None;
        }
        self.reset_nominal_geometry();
    }
}
