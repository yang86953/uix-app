//! Radio widget — 单选组，支持 horizontal/vertical、disabled、hover。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Radius;
use crate::native::traits::input::ControlSize;
use crate::ui::state::State;
use crate::ui::SnapshotFields;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, WidgetTree,
};
use std::cell::Cell;

/// 方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadioDirection {
    Horizontal,
    Vertical,
}

component! {
    /// Radio — 单选按钮组。
    pub struct Radio {
        group_name: String,
        options: Vec<String>,
        selected: usize,
        value_binding: Option<State<String>>,
        disabled: bool,
        direction: RadioDirection,
        item_h: f32,
        hovered_idx: Option<usize>,
        focused: bool,
        pending_change: Cell<Option<usize>>,
    }


    tab_index => (&self) -> i32 { 1 }
    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_value();
        if self.disabled { return EventResult::NotHandled; }
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(idx) = self.option_at(pos.x, pos.y) {
                    self.select_index(idx);
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                self.hovered_idx = self.option_at(pos.x, pos.y);
                EventResult::Handled
            }
            SystemEvent::PointerEnter => { EventResult::Handled }
            SystemEvent::PointerLeave => { self.hovered_idx = None; EventResult::Handled }
            SystemEvent::FocusIn => { self.focused = true; EventResult::Handled }
            SystemEvent::FocusOut => { self.focused = false; EventResult::Handled }
            SystemEvent::KeyDown { key, .. } => {
            match key {
                KeyCode::Right | KeyCode::Down => {
                    let next = if self.selected < self.options.len() {
                        self.selected.saturating_add(1)
                    } else {
                        0
                    };
                    if next < self.options.len() {
                        self.select_index(next);
                    }
                    EventResult::Handled
                }
                KeyCode::Left | KeyCode::Up => {
                        if self.selected == usize::MAX {
                            if let Some(last) = self.options.len().checked_sub(1) {
                                self.select_index(last);
                            }
                        } else if self.selected > 0 {
                            let prev = self.selected - 1;
                            self.select_index(prev);
                        }
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

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.capture_bound_value_dependency();
        let cy = frame.y + self.item_h * 0.5;

        match self.direction {
            RadioDirection::Horizontal => {
                let mut x = frame.x;
                for (i, opt) in self.options.iter().enumerate() {
                    let w = self.item_width(opt);
                    self.render_radio_item(ctx, i, opt, x, cy, w);
                    x += w;
                }
            }
            RadioDirection::Vertical => {
                for (i, opt) in self.options.iter().enumerate() {
                    let y = frame.y + i as f32 * self.item_h + self.item_h * 0.5;
                    let w = frame.w;
                    self.render_radio_item(ctx, i, opt, frame.x, y, w);
                }
            }
        }
    }
}

impl Radio {
    fn select_index(&mut self, index: usize) {
        if index >= self.options.len() || self.selected == index {
            return;
        }
        self.selected = index;
        self.write_bound_value();
        self.pending_change.set(Some(index));
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
        let item_w = self
            .options
            .iter()
            .map(|o| self.item_width(o))
            .collect::<Vec<_>>();
        match self.direction {
            RadioDirection::Horizontal => {
                let w = item_w.iter().sum::<f32>().max(120.0);
                Size::new(w, self.item_h)
            }
            RadioDirection::Vertical => {
                let w = item_w
                    .iter()
                    .cloned()
                    .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap_or(120.0)
                    .max(120.0);
                Size::new(w, self.item_h * self.options.len() as f32)
            }
        }
    }

    fn option_at(&self, px: f32, py: f32) -> Option<usize> {
        match self.direction {
            RadioDirection::Horizontal => {
                if py < 0.0 || py > self.item_h {
                    return None;
                }
                let mut cum_x = 0.0f32;
                for (i, opt) in self.options.iter().enumerate() {
                    let w = self.item_width(opt);
                    if px >= cum_x && px <= cum_x + w {
                        return Some(i);
                    }
                    cum_x += w;
                }
                None
            }
            RadioDirection::Vertical => {
                let idx = (py / self.item_h) as usize;
                if idx < self.options.len() && py >= 0.0 {
                    Some(idx)
                } else {
                    None
                }
            }
        }
    }

    fn render_radio_item(
        &self,
        ctx: &mut PaintContext,
        i: usize,
        opt: &str,
        x: f32,
        cy: f32,
        _seg_w: f32,
    ) {
        let scale = self.visual_scale();
        let r = 6.0 * scale;
        let dot_r = 3.5 * scale;
        let selected = i == self.selected;
        let hovered = self.hovered_idx == Some(i);

        let (ring_color, dot_color, text_c) = if self.disabled {
            (
                ctx.tokens().color_border_secondary(),
                ctx.tokens().color_border_secondary(),
                ctx.tokens().color_text_quaternary(),
            )
        } else if selected {
            (
                ctx.tokens().color_primary(),
                ctx.tokens().color_primary(),
                ctx.tokens().color_text(),
            )
        } else if hovered {
            (
                ctx.tokens().color_primary_hover(),
                ctx.tokens().color_primary_hover(),
                ctx.tokens().color_text(),
            )
        } else {
            (
                ctx.tokens().color_border(),
                ctx.tokens().color_border(),
                ctx.tokens().color_text(),
            )
        };

        // 外圈
        let circle_rect = Rect::new(x + scale, cy - r, r * 2.0, r * 2.0);
        ctx.stroke_rect(circle_rect, ring_color, 1.5, Some(Radius::uniform(r)));

        // 选中填充点
        if selected {
            ctx.fill_circle(x + r + scale, cy, dot_r, dot_color);
        }
        // 使用 em-box 高度（font_size）垂直居中，而非字体度量高度
        let row_rect = Rect::new(x, cy - self.item_h * 0.5, _seg_w, self.item_h);
        let font_size = self.font_size();
        let text_y = ctx.visual_center_y(row_rect, font_size);
        ctx.draw_text(opt, Point::new(x + 20.0 * scale, text_y), text_c, font_size);
    }
}

impl Default for Radio {
    fn default() -> Self {
        Self::new()
    }
}

impl Radio {
    pub fn new() -> Self {
        let config = crate::ui::config::use_config();
        Self {
            group_name: String::new(),
            options: Vec::new(),
            selected: 0,
            value_binding: None,
            disabled: false,
            direction: RadioDirection::Horizontal,
            item_h: crate::ui::config::control_height(config.size),
            hovered_idx: None,
            focused: false,
            pending_change: Cell::new(None),
        }
    }

    /// 创建具名受控单选组。
    pub fn group<I, S>(name: impl Into<String>, options: I, state: &State<String>) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self::new().group_name(name).options(options).value(state)
    }

    pub fn group_name(mut self, name: impl Into<String>) -> Self {
        self.group_name = name.into();
        self
    }

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
        self
    }

    /// 设置非受控单选组的初始索引。
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

    pub fn current_value(&self) -> Option<String> {
        self.options.get(self.selected).cloned()
    }

    pub fn current_index(&self) -> Option<usize> {
        (self.selected < self.options.len()).then_some(self.selected)
    }

    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn size(mut self, size: ControlSize) -> Self {
        self.item_h = crate::ui::config::control_height(size);
        self
    }
    pub fn vertical(mut self) -> Self {
        self.direction = RadioDirection::Vertical;
        self
    }

    fn visual_scale(&self) -> f32 {
        self.item_h / crate::ui::config::control_height(ControlSize::Medium)
    }

    fn font_size(&self) -> f32 {
        13.0 * self.visual_scale().sqrt()
    }

    fn item_width(&self, option: &str) -> f32 {
        let scale = self.visual_scale();
        option.len() as f32 * 9.0 * scale.sqrt() + 30.0 * scale
    }
}

impl Radio {
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Radio {
            group_name: self.group_name.clone(),
            options: self.options.clone(),
            selected: self.selected,
            disabled: self.disabled,
            direction: self.direction,
            item_h: self.item_h,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_selected = next.value_binding.as_ref().map(|_| next.selected);
        self.group_name = next.group_name;
        self.options = next.options;
        self.value_binding = next.value_binding;
        self.selected = controlled_selected.unwrap_or_else(|| {
            (self.selected < self.options.len())
                .then_some(self.selected)
                .unwrap_or(usize::MAX)
        });
        self.disabled = next.disabled;
        self.direction = next.direction;
        self.item_h = next.item_h;
    }
}
