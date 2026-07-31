//! FloatButton widget — 浮动按钮，Ant Design 风格。
//!
//! 固定在屏幕角落的圆形按钮，支持图标、tooltip、badge 等。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::{Color, Radius};
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::core::paint_context::PaintContext;
use crate::ui::widgets::feedback::TriggerMode;
use crate::ui::SnapshotFields;
use crate::ui::{
    EventResult, KeyCode, MouseButton, OverlayEntry, OverlayKind, SystemEvent, WidgetTree,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const FLOAT_BUTTON_GROUP_TRIGGER_SIZE: f32 = 40.0;
const FLOAT_BUTTON_GROUP_GAP: f32 = 8.0;

// FloatButton — 浮动操作按钮。
component! {
    pub struct FloatButton {
        icon: String,
        tooltip: String,
        badge_count: i32,
        size: f32,
        x: f32,
        y: f32,
        reserve_layout_space: bool,
        trigger_mode: TriggerMode,
        hovered: bool,
        pressed: bool,
        focused: bool,
        in_group: bool,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    tab_index => (&self) -> i32 { 1 }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        self.button_rect(frame)
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerEnter => {
                self.hovered = true;
                self.pointer_boundary_result()
            }
            SystemEvent::PointerLeave => {
                self.hovered = false;
                self.pressed = false;
                self.pointer_boundary_result()
            }
            SystemEvent::PointerDown { button: MouseButton::Left, .. } => {
                self.pressed = true;
                EventResult::Handled
            }
            SystemEvent::PointerUp { button: MouseButton::Left, .. } => {
                self.pressed = false;
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.pressed = false;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key: KeyCode::Enter | KeyCode::Space, .. } => {
                self.pressed = true;
                EventResult::Handled
            }
            SystemEvent::KeyUp { key: KeyCode::Enter | KeyCode::Space, .. } => {
                self.pressed = false;
                EventResult::Handled
            }
            _ => EventResult::NotHandled
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        self.paint_bounds(frame)
    }

    overlay_entry => (&self, id: crate::ui::ComponentId, frame: Rect) -> Option<OverlayEntry> {
        if self.reserve_layout_space {
            return None;
        }
        Some(
            OverlayEntry::new(id, OverlayKind::Custom)
                .bounds(self.button_rect(frame))
                .z_index(900),
        )
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let loc = crate::ui::locale::use_locale();
        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let white = Color::white();
        let text_sec = ctx.tokens().color_text_quaternary();
        let bg = if self.pressed {
            ctx.tokens().color_primary_active()
        } else if self.hovered {
            primary_hover
        } else {
            primary
        };
        let r = Radius::uniform(self.size * 0.5);
        let btn_rect = self.button_rect(frame);
        let cx = btn_rect.x + btn_rect.w * 0.5;
        let cy = btn_rect.y + btn_rect.h * 0.5;
        // 阴影
        ctx.draw_box_shadow(btn_rect, 8.0, 0.0, 4.0, Color::from_rgba(0, 0, 0, 40), Some(r));
        ctx.fill_rect(btn_rect, bg, Some(r));
        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(btn_rect, ctx.tokens().color_primary_border(), 2.0, Some(r));
        }
        let icon_fs = 16.0;
        crate::ui::widgets::general::icon::Icon::paint_in_frame(
            ctx, &self.icon, btn_rect, white, icon_fs,
        );
        // Badge
        if self.badge_count > 0 {
            let badge_count = self.badge_count.to_string();
            let badge = if self.badge_count > 99 { loc.float_badge_overflow } else { &badge_count };
            ctx.fill_circle(cx + self.size * 0.3, cy - self.size * 0.3, 10.0, ctx.tokens().color_error());
            ctx.draw_text(badge, Point::new(cx + self.size * 0.3 - 7.0, cy - self.size * 0.3 - 7.0), white, 10.0);
        }
        let show_tooltip = match self.trigger_mode {
            TriggerMode::Hover => self.hovered,
            TriggerMode::Focus => self.focused,
            TriggerMode::Click | TriggerMode::ContextMenu => self.pressed,
        };
        if show_tooltip && !self.tooltip.is_empty() {
            let tip = self.tooltip_rect(frame);
            let tip_radius = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
            ctx.fill_rect(tip, ctx.tokens().color_bg_elevated(), tip_radius);
            ctx.stroke_rect(tip, ctx.tokens().color_border_secondary(), 1.0, tip_radius);
            ctx.text_center(&self.tooltip, tip, text_sec, 12.0);
        }
    }
}

impl FloatButton {
    /// 创建使用指定 Lucide 名称的浮动图标按钮。
    pub fn new(icon: &str) -> Self {
        Self {
            icon: icon.to_string(),
            tooltip: String::new(),
            badge_count: 0,
            size: 40.0,
            x: 0.0,
            y: 0.0,
            reserve_layout_space: false,
            trigger_mode: TriggerMode::Hover,
            hovered: false,
            pressed: false,
            focused: false,
            in_group: false,
        }
    }
    /// 设置相对零布局槽左上角的视觉偏移。
    pub fn position(mut self, x: f32, y: f32) -> Self {
        self.x = finite_or_zero(x);
        self.y = finite_or_zero(y);
        self
    }
    pub fn tooltip(mut self, t: &str) -> Self {
        self.tooltip = t.to_string();
        self
    }
    pub fn badge(mut self, count: i32) -> Self {
        self.badge_count = count.max(0);
        self
    }
    pub fn size(mut self, s: f32) -> Self {
        self.size = positive_or(s, 40.0);
        self
    }

    /// 为按钮锚点保留与直径相同的布局空间；默认浮动模式仍保持零占位。
    pub fn reserve_layout_space(mut self, reserve: bool) -> Self {
        self.reserve_layout_space = reserve;
        self
    }

    pub fn trigger(mut self, trigger: TriggerMode) -> Self {
        self.trigger_mode = trigger;
        self
    }

    fn intrinsic_size(&self) -> Size {
        if self.reserve_layout_space {
            Size::new(self.size, self.size)
        } else {
            Size::zero()
        }
    }

    fn button_rect(&self, frame: Rect) -> Rect {
        Rect::new(frame.x + self.x, frame.y + self.y, self.size, self.size)
    }

    fn tooltip_rect(&self, frame: Rect) -> Rect {
        let button = self.button_rect(frame);
        let width = (self.tooltip.chars().count() as f32 * 7.0 + 20.0).max(44.0);
        Rect::new(
            button.x - width - 8.0,
            button.y + (button.h - 28.0) * 0.5,
            width,
            28.0,
        )
    }

    fn paint_bounds(&self, frame: Rect) -> Rect {
        let button = self.button_rect(frame);
        let shadow = Rect::new(
            button.x - 10.0,
            button.y - 10.0,
            button.w + 20.0,
            button.h + 24.0,
        );
        if self.tooltip.is_empty() {
            shadow
        } else {
            shadow.union(&self.tooltip_rect(frame))
        }
    }

    fn pointer_boundary_result(&self) -> EventResult {
        if self.in_group {
            EventResult::Bubbled
        } else {
            EventResult::Handled
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::FloatButton {
            icon: self.icon.clone(),
            tooltip: self.tooltip.clone(),
            badge_count: self.badge_count,
            size: self.size,
            x: self.x,
            y: self.y,
            reserve_layout_space: self.reserve_layout_space,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.icon = next.icon;
        self.tooltip = next.tooltip;
        self.badge_count = next.badge_count;
        self.size = next.size;
        self.x = next.x;
        self.y = next.y;
        self.reserve_layout_space = next.reserve_layout_space;
        self.trigger_mode = next.trigger_mode;
        self.in_group = next.in_group;
    }
}

impl Default for FloatButton {
    fn default() -> Self {
        Self::new("plus")
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FloatButtonGroupItemLayout {
    size: f32,
    x: f32,
    y: f32,
    paint_bounds: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FloatButtonGroupPressTarget {
    Trigger,
}

component! {
    /// 一组可展开的浮动操作按钮。
    pub struct FloatButtonGroup {
        #[snapshot(skip)]
        buttons: Rc<RefCell<Option<Vec<FloatButton>>>>,
        #[snapshot(skip)]
        item_layouts: Vec<FloatButtonGroupItemLayout>,
        trigger: TriggerMode,
        expanded: bool,
        closing: bool,
        transition: TransitionPlayer,
        transition_dirty: bool,
        layout_requested: Cell<bool>,
        hovered: bool,
        focused: bool,
        focus_within: bool,
        pressed_target: Option<FloatButtonGroupPressTarget>,
        pressed_button: Option<MouseButton>,
        pressed_key: Option<KeyCode>,
    }

    build_view_children => (&self) -> Vec<crate::ui::view::ViewNode> {
        self.buttons
            .borrow_mut()
            .take()
            .unwrap_or_default()
            .into_iter()
            .map(crate::ui::view::ViewNode::leaf)
            .collect()
    }

    tab_index => (&self) -> i32 { i32::from(!self.item_layouts.is_empty()) }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(
            FLOAT_BUTTON_GROUP_TRIGGER_SIZE,
            FLOAT_BUTTON_GROUP_TRIGGER_SIZE,
        ))
    }

    child_overflow_expands_parent => (&self) -> bool { false }

    child_visible => (&self, index: usize) -> bool {
        index < self.item_layouts.len() && self.children_are_visible()
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        let progress = self.expansion_progress();
        children
            .iter()
            .enumerate()
            .filter_map(|(index, child)| {
                self.item_layouts
                    .get(index)
                    .map(|_| (child.id, self.child_frame(frame, index, progress)))
            })
            .collect()
    }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        self.interaction_bounds(frame)
    }

    hit_test_children => (&self) -> bool { self.children_are_visible() }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerEnter => {
                self.hovered = true;
                if self.trigger == TriggerMode::Hover {
                    self.open();
                }
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                let changed = self.hovered || self.pressed_target.is_some();
                self.hovered = false;
                self.cancel_pending_activation();
                if self.trigger == TriggerMode::Hover {
                    self.close();
                    EventResult::Handled
                } else if changed {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerDown { pos, button, .. } => {
                if self.item_layouts.is_empty()
                    || !Self::local_trigger_rect().contains(*pos)
                    || !self.accepts_pointer_button(*button)
                {
                    return EventResult::NotHandled;
                }
                self.pressed_target = Some(FloatButtonGroupPressTarget::Trigger);
                self.pressed_button = Some(*button);
                EventResult::Handled
            }
            SystemEvent::PointerUp { pos, button, .. }
                if self.pressed_target.is_some() || self.pressed_button.is_some() =>
            {
                let target = self.pressed_target.take();
                let pressed_button = self.pressed_button.take();
                if target == Some(FloatButtonGroupPressTarget::Trigger)
                    && pressed_button == Some(*button)
                    && Self::local_trigger_rect().contains(*pos)
                {
                    self.toggle();
                }
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.cancel_pending_activation();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key: key @ (KeyCode::Enter | KeyCode::Space), .. }
                if !self.item_layouts.is_empty() =>
            {
                self.pressed_key = Some(*key);
                EventResult::Handled
            }
            SystemEvent::KeyUp { key: key @ (KeyCode::Enter | KeyCode::Space), .. }
                if self.pressed_key.is_some() =>
            {
                let matches = self.pressed_key.take() == Some(*key);
                if matches {
                    self.toggle();
                }
                EventResult::Handled
            }
            SystemEvent::KeyDown { key: KeyCode::Escape, .. } if self.is_present() => {
                self.cancel_pending_activation();
                self.close();
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    on_focus_within => (&mut self, focused: bool) -> EventResult {
        self.focus_within = focused;
        if self.trigger != TriggerMode::Focus {
            return EventResult::NotHandled;
        }
        if focused {
            self.open();
        } else {
            self.cancel_pending_activation();
            self.close();
        }
        EventResult::Handled
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let trigger = Self::trigger_rect(frame);
        let radius = Some(Radius::uniform(FLOAT_BUTTON_GROUP_TRIGGER_SIZE * 0.5));
        let background = if self.pressed_target.is_some() || self.pressed_key.is_some() {
            ctx.tokens().color_primary_active()
        } else if self.hovered {
            ctx.tokens().color_primary_hover()
        } else {
            ctx.tokens().color_primary()
        };
        ctx.draw_box_shadow(
            trigger,
            8.0,
            0.0,
            4.0,
            Color::from_rgba(0, 0, 0, 40),
            radius,
        );
        ctx.fill_rect(trigger, background, radius);
        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                trigger,
                ctx.tokens().color_primary_border(),
                2.0,
                radius,
            );
        }
        crate::ui::widgets::general::icon::Icon::paint_in_frame(
            ctx,
            if self.expanded { "x" } else { "plus" },
            trigger,
            Color::white(),
            18.0,
        );
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        self.full_dirty_bounds(frame)
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !self.is_present() || self.transition.finished {
            self.transition_dirty = false;
            return false;
        }
        self.transition.update(dt);
        self.transition_dirty = true;
        self.layout_requested.set(true);
        if self.closing && self.transition.finished {
            self.closing = false;
            self.layout_requested.set(true);
        }
        self.is_present() && !self.transition.finished
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty {
            self.full_dirty_bounds(frame)
        } else {
            Rect::zero()
        }
    }
}

impl FloatButtonGroup {
    pub fn new() -> Self {
        let mut transition = TransitionPlayer::new(presets::collapse_collapse());
        transition.update(1.0);
        Self {
            buttons: Rc::new(RefCell::new(Some(Vec::new()))),
            item_layouts: Vec::new(),
            trigger: TriggerMode::Hover,
            expanded: false,
            closing: false,
            transition,
            transition_dirty: false,
            layout_requested: Cell::new(false),
            hovered: false,
            focused: false,
            focus_within: false,
            pressed_target: None,
            pressed_button: None,
            pressed_key: None,
        }
    }

    pub fn buttons(mut self, mut buttons: Vec<FloatButton>) -> Self {
        for button in &mut buttons {
            button.in_group = true;
        }
        self.item_layouts = buttons
            .iter()
            .map(|button| FloatButtonGroupItemLayout {
                size: button.size,
                x: button.x,
                y: button.y,
                paint_bounds: button.paint_bounds(Rect::zero()),
            })
            .collect();
        self.buttons = Rc::new(RefCell::new(Some(buttons)));
        self
    }

    pub fn trigger(mut self, trigger: TriggerMode) -> Self {
        self.trigger = trigger;
        self
    }

    fn local_trigger_rect() -> Rect {
        Rect::new(
            0.0,
            0.0,
            FLOAT_BUTTON_GROUP_TRIGGER_SIZE,
            FLOAT_BUTTON_GROUP_TRIGGER_SIZE,
        )
    }

    fn trigger_rect(frame: Rect) -> Rect {
        Rect::new(
            frame.x,
            frame.y,
            FLOAT_BUTTON_GROUP_TRIGGER_SIZE,
            FLOAT_BUTTON_GROUP_TRIGGER_SIZE,
        )
    }

    fn accepts_pointer_button(&self, button: MouseButton) -> bool {
        match self.trigger {
            TriggerMode::Click => button == MouseButton::Left,
            TriggerMode::ContextMenu => button == MouseButton::Right,
            TriggerMode::Hover | TriggerMode::Focus => false,
        }
    }

    fn open(&mut self) {
        if self.item_layouts.is_empty() || (self.expanded && !self.closing) {
            return;
        }
        let opacity = self.expansion_progress();
        self.expanded = true;
        self.closing = false;
        self.transition = TransitionPlayer::new_from_current(
            presets::collapse_expand(),
            opacity,
            Point::new(0.0, 0.0),
            1.0,
        );
        self.transition_dirty = true;
        self.layout_requested.set(true);
    }

    fn close(&mut self) {
        if !self.is_present() {
            return;
        }
        let opacity = self.expansion_progress();
        self.expanded = false;
        self.closing = true;
        self.transition = TransitionPlayer::new_from_current(
            presets::collapse_collapse(),
            opacity,
            Point::new(0.0, 0.0),
            1.0,
        );
        self.transition_dirty = true;
        self.layout_requested.set(true);
    }

    fn toggle(&mut self) {
        if self.expanded && !self.closing {
            self.close();
        } else {
            self.open();
        }
    }

    fn cancel_pending_activation(&mut self) {
        self.pressed_target = None;
        self.pressed_button = None;
        self.pressed_key = None;
    }

    fn is_present(&self) -> bool {
        self.expanded || self.closing
    }

    fn expansion_progress(&self) -> f32 {
        if self.is_present() {
            self.transition.opacity_progress.clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    fn children_are_visible(&self) -> bool {
        self.is_present() && self.expansion_progress() > 0.0
    }

    fn child_frame(&self, frame: Rect, index: usize, progress: f32) -> Rect {
        let item = self.item_layouts[index];
        let collapsed_x = frame.x + (FLOAT_BUTTON_GROUP_TRIGGER_SIZE - item.size) * 0.5;
        let collapsed_y = frame.y + (FLOAT_BUTTON_GROUP_TRIGGER_SIZE - item.size) * 0.5;
        let preceding_height = self.item_layouts[..index]
            .iter()
            .map(|layout| layout.size + FLOAT_BUTTON_GROUP_GAP)
            .sum::<f32>();
        let expanded_y =
            frame.y + FLOAT_BUTTON_GROUP_TRIGGER_SIZE + FLOAT_BUTTON_GROUP_GAP + preceding_height;
        Rect::new(
            collapsed_x,
            collapsed_y + (expanded_y - collapsed_y) * progress,
            item.size,
            item.size,
        )
    }

    fn interaction_bounds(&self, frame: Rect) -> Rect {
        let mut bounds = Self::trigger_rect(frame);
        if !self.children_are_visible() {
            return bounds;
        }
        let progress = self.expansion_progress();
        for (index, item) in self.item_layouts.iter().enumerate() {
            let child = self.child_frame(frame, index, progress);
            bounds = bounds.union(&Rect::new(
                child.x + item.x,
                child.y + item.y,
                item.size,
                item.size,
            ));
        }
        bounds
    }

    fn full_dirty_bounds(&self, frame: Rect) -> Rect {
        let trigger = Self::trigger_rect(frame);
        let mut bounds = Rect::new(
            trigger.x - 10.0,
            trigger.y - 10.0,
            trigger.w + 20.0,
            trigger.h + 24.0,
        );
        for (index, item) in self.item_layouts.iter().enumerate() {
            for progress in [0.0, 1.0] {
                let child = self.child_frame(frame, index, progress);
                let paint = item.paint_bounds;
                bounds = bounds.union(&Rect::new(
                    child.x + paint.x,
                    child.y + paint.y,
                    paint.w,
                    paint.h,
                ));
            }
        }
        bounds
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::FloatButtonGroup {
            button_count: self.item_layouts.len(),
            trigger: self.trigger,
            expanded: self.expanded,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let trigger_changed = self.trigger != next.trigger;
        self.buttons = next.buttons;
        self.item_layouts = next.item_layouts;
        self.trigger = next.trigger;
        if self.item_layouts.is_empty() {
            self.expanded = false;
            self.closing = false;
            self.transition = next.transition;
            self.transition_dirty = false;
            self.layout_requested.set(true);
        } else if trigger_changed {
            self.cancel_pending_activation();
            if self.trigger == TriggerMode::Focus && self.focus_within {
                self.open();
            } else {
                self.close();
            }
        }
    }
}

impl Default for FloatButtonGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::ui::view::View for FloatButtonGroup {
    fn build(self) -> crate::ui::view::ViewNode {
        crate::ui::view::ViewNode::leaf(self)
    }
}

/// FloatButtonBackTop — 回到顶部按钮（FloatButton 的便捷封装）。
pub struct FloatButtonBackTop;

impl FloatButtonBackTop {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> FloatButton {
        FloatButton::new("chevron-up").tooltip("回到顶部")
    }
}

fn positive_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}
