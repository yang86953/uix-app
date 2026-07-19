//! Tabs widget — Ant Design style tab bar with content panels.

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Radius;
use crate::ui::state::State;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent,
    WidgetTree,
};
use std::cell::RefCell;
use std::fmt::Display;
use std::rc::Rc;

/// A single tab definition.
#[derive(Debug, Clone, PartialEq)]
pub struct Tab {
    pub label: String,
    pub key: String,
}

/// Tab bar position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabPosition {
    Top,
    Bottom,
}

trait TabsValueBinding {
    fn selected_index(&self) -> Option<usize>;
    fn select_index(&self, index: usize);
}

struct StateTabsValueBinding<T> {
    state: State<T>,
    values: Vec<T>,
}

impl<T> TabsValueBinding for StateTabsValueBinding<T>
where
    T: Clone + PartialEq + Send + Sync + 'static,
{
    fn selected_index(&self) -> Option<usize> {
        let selected = self.state.get();
        self.values.iter().position(|value| value == &selected)
    }

    fn select_index(&self, index: usize) {
        let Some(value) = self.values.get(index) else {
            return;
        };
        if self.state.get() != *value {
            self.state.set(value.clone());
        }
    }
}

component! {
    /// Tabs widget with a tab bar and content switching.
    pub struct Tabs {
        tabs: Vec<Tab>,
        active_index: usize,
        value_binding: Option<Rc<dyn TabsValueBinding>>,
        position: TabPosition,
        tab_height: f32,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        focused: bool,
        pending_change: RefCell<Option<String>>,
        /// 每帧 render 时计算的各 tab x 坐标（供 on_event 点击定位使用）
        tab_x_positions: RefCell<Vec<(f32, f32)>>,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_value();
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let tab_count = self.tabs.len();
                if tab_count == 0 { return EventResult::NotHandled; }
                // 使用 render 时存储的 tab_x_positions 做点击定位
                let click_x = pos.x;
                let clicked = self
                    .tab_x_positions
                    .borrow()
                    .iter()
                    .position(|&(start, end)| click_x >= start && click_x < end);
                if let Some(index) = clicked {
                    self.select_index(index);
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
            SystemEvent::KeyDown { key, .. } if !self.tabs.is_empty() => match key {
                KeyCode::Right => {
                    let next = if self.active_index < self.tabs.len() {
                        (self.active_index + 1) % self.tabs.len()
                    } else {
                        0
                    };
                    self.select_index(next);
                    EventResult::Handled
                }
                KeyCode::Left => {
                    let previous = if self.active_index < self.tabs.len() {
                        (self.active_index + self.tabs.len() - 1) % self.tabs.len()
                    } else {
                        self.tabs.len() - 1
                    };
                    self.select_index(previous);
                    EventResult::Handled
                }
                KeyCode::Home => {
                    self.select_index(0);
                    EventResult::Handled
                }
                KeyCode::End => {
                    self.select_index(self.tabs.len() - 1);
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|key| SemanticEvent::change(id, key))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        self.capture_bound_value_dependency();
        let bg_container = ctx.tokens().color_bg_container();
        let border_secondary = ctx.tokens().color_border_secondary();
        let primary = ctx.tokens().color_primary();
        let text_secondary = ctx.tokens().color_text_secondary();

        // 预计算各 tab 宽度和位置（使用 measure_text 确保与实际渲染一致）
        let gap = 12.0;
        let pad = 16.0;
        let mut positions = Vec::with_capacity(self.tabs.len());
        let mut cursor_x = pad;
        for tab in &self.tabs {
            let tw = ctx.measure_text(&tab.label, 14.0).w + pad * 2.0;
            positions.push((cursor_x, cursor_x + tw));
            cursor_x += tw + gap;
        }
        *self.tab_x_positions.borrow_mut() = positions;

        let tab_bar_h = self.tab_height;
        let tab_bar_y = match self.position {
            TabPosition::Top => frame.y,
            TabPosition::Bottom => frame.y + frame.h - tab_bar_h,
        };

        ctx.fill_rect(Rect::new(frame.x, tab_bar_y, frame.w, tab_bar_h), bg_container, None);
        ctx.fill_rect(Rect::new(frame.x, tab_bar_y + tab_bar_h - 2.0, frame.w, 2.0), border_secondary, None);

        let mut cursor_x = frame.x + pad;
        for (i, tab) in self.tabs.iter().enumerate() {
            let is_active = i == self.active_index;
            let text_color = if is_active { primary } else { text_secondary };
            let tw = ctx.measure_text(&tab.label, 14.0).w + pad * 2.0;

            let tab_rect = Rect::new(cursor_x, tab_bar_y, tw, tab_bar_h);
            let tab_text_y = ctx.visual_center_y(tab_rect, 14.0);
            ctx.draw_text(&tab.label,
                Point::new(cursor_x + pad, tab_text_y),
                text_color, 14.0);

            if is_active {
                let indicator_w = tw * 0.6;
                let indicator_x = cursor_x + (tw - indicator_w) * 0.5;
                let indicator_y = match self.position {
                    TabPosition::Top => tab_bar_y + tab_bar_h - 2.0,
                    TabPosition::Bottom => tab_bar_y - 2.0,
                };
                ctx.fill_rect(Rect::new(indicator_x, indicator_y, indicator_w, 2.0), primary, Some(Radius::uniform(1.0)));
            }
            cursor_x += tw + gap;
        }

        let content_y = match self.position {
            TabPosition::Top => tab_bar_y + tab_bar_h,
            TabPosition::Bottom => frame.y,
        };
        ctx.fill_rect(Rect::new(frame.x, content_y, frame.w, frame.h - tab_bar_h), bg_container, None);
        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                Rect::new(frame.x, tab_bar_y, frame.w, tab_bar_h),
                primary,
                1.5,
                Some(Radius::uniform(ctx.tokens().border_radius_sm())),
            );
        }
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        if children.is_empty() { return Vec::new(); }
        let tab_bar_h = self.tab_height;
        let content_y = match self.position {
            TabPosition::Top => frame.y + tab_bar_h,
            TabPosition::Bottom => frame.y,
        };
        let content_h = frame.h - tab_bar_h;
        if self.active_index < children.len() {
            let cid = children[self.active_index].id;
            vec![(cid, Rect::new(frame.x + 16.0, content_y + 8.0, frame.w - 32.0, content_h - 16.0))]
        } else {
            Vec::new()
        }
    }
}

impl Default for Tabs {
    fn default() -> Self {
        Self::new()
    }
}

impl Tabs {
    fn intrinsic_size(&self) -> Size {
        Size::new(
            self.fixed_width.unwrap_or(400.0),
            self.fixed_height.unwrap_or(200.0),
        )
    }

    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_index: 0,
            value_binding: None,
            position: TabPosition::Top,
            tab_height: 40.0,
            fixed_width: None,
            fixed_height: None,
            focused: false,
            pending_change: RefCell::new(None),
            tab_x_positions: RefCell::new(Vec::new()),
        }
    }

    pub fn tab(mut self, label: &str, key: &str) -> Self {
        self.value_binding = None;
        self.tabs.push(Tab {
            label: label.to_string(),
            key: key.to_string(),
        });
        self
    }
    pub fn tabs(mut self, tabs: Vec<Tab>) -> Self {
        self.value_binding = None;
        self.tabs = tabs;
        self
    }
    pub fn active(mut self, index: usize) -> Self {
        self.value_binding = None;
        self.active_index = index.min(self.tabs.len().saturating_sub(1));
        self
    }
    pub fn active_index(&self) -> usize {
        self.active_index
    }
    pub fn current_key(&self) -> Option<&str> {
        self.tabs.get(self.active_index).map(|tab| tab.key.as_str())
    }

    /// 将字符串 tab key 双向绑定到外部 State；应在 `tab` / `tabs` 之后调用。
    pub fn active_key(mut self, state: &State<String>) -> Self {
        let values = self.tabs.iter().map(|tab| tab.key.clone()).collect();
        self.bind_values(state, values);
        self
    }

    /// 以同一种 typed key 构造受控 Tabs；`Display` 文本作为语义事件和快照 key。
    pub fn controlled<K, I, L>(tabs: I, state: &State<K>) -> Self
    where
        K: Clone + PartialEq + Display + Send + Sync + 'static,
        I: IntoIterator<Item = (L, K)>,
        L: Into<String>,
    {
        let mut component = Self::new();
        let mut values = Vec::new();
        for (label, key) in tabs {
            component.tabs.push(Tab {
                label: label.into(),
                key: key.to_string(),
            });
            values.push(key);
        }
        component.bind_values(state, values);
        component
    }
    pub fn position(mut self, pos: TabPosition) -> Self {
        self.position = pos;
        self
    }
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.fixed_width = Some(w);
        self.fixed_height = Some(h);
        self
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let previous_key = self.current_key().map(str::to_owned);
        let controlled_index = next
            .value_binding
            .as_ref()
            .map(|binding| binding.selected_index().unwrap_or(usize::MAX));
        self.tabs = next.tabs;
        self.value_binding = next.value_binding;
        self.active_index = controlled_index.unwrap_or_else(|| {
            previous_key
                .as_deref()
                .and_then(|key| self.tabs.iter().position(|tab| tab.key == key))
                .unwrap_or_else(|| self.active_index.min(self.tabs.len().saturating_sub(1)))
        });
        self.position = next.position;
        self.tab_height = next.tab_height;
        self.fixed_width = next.fixed_width;
        self.fixed_height = next.fixed_height;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Tabs {
            tabs: self.tabs.clone(),
            active_index: self.active_index,
            active_key: self.current_key().map(str::to_owned),
            position: self.position,
            tab_height: self.tab_height,
            fixed_width: self.fixed_width,
            fixed_height: self.fixed_height,
        }
    }

    fn select_index(&mut self, index: usize) {
        let Some(tab) = self.tabs.get(index) else {
            return;
        };
        if index != self.active_index {
            self.active_index = index;
            if let Some(binding) = self.value_binding.as_ref() {
                binding.select_index(index);
            }
            self.pending_change.replace(Some(tab.key.clone()));
        }
    }

    fn bind_values<T>(&mut self, state: &State<T>, values: Vec<T>)
    where
        T: Clone + PartialEq + Send + Sync + 'static,
    {
        let binding: Rc<dyn TabsValueBinding> = Rc::new(StateTabsValueBinding {
            state: state.clone(),
            values,
        });
        self.active_index = binding.selected_index().unwrap_or(usize::MAX);
        self.value_binding = Some(binding);
    }

    fn sync_bound_value(&mut self) {
        let Some(index) = self
            .value_binding
            .as_ref()
            .and_then(|binding| binding.selected_index())
        else {
            if self.value_binding.is_some() {
                self.active_index = usize::MAX;
            }
            return;
        };
        self.active_index = index;
    }

    fn capture_bound_value_dependency(&self) {
        if let Some(binding) = self.value_binding.as_ref() {
            let _ = binding.selected_index();
        }
    }
}
