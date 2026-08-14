//! Card widget — Ant Design style container with elevation, shadow, optional
//! title, body, hover feedback, and configurable border radius.

use std::cell::{Cell, RefCell};

use crate::component;
use crate::core::{Constraints, EdgeInsets, Point, Rect, Size};
use crate::draw::painting::PaintPass;
use crate::draw::{Color, Radius};
use crate::ui::children::WidgetChildren;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::component::tree_measure::child_from_tree_with_constraints;
use crate::ui::layout::{
    flex::compute_flex_layout, AlignItems, FlexChild, FlexDirection, FlexInput, JustifyContent,
    LayoutChild,
};
use crate::ui::SnapshotFields;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, WidgetComponent,
    WidgetTree,
};

component! {
    /// Card widget with shadow elevation, hover highlight, and content padding.
    pub struct Card {
        title: Option<String>,
        children: WidgetChildren,
        bordered: bool,
        hoverable: bool,
        hovered: bool,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        padding: f32,
        elevation: u8,
        flex_grow_val: f32,
        actions: Vec<String>,
        focused: bool,
        focused_action: usize,
        hovered_action: Cell<Option<usize>>,
        last_frame: Cell<Option<Rect>>,
        pending_submit: RefCell<Option<String>>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    flex_grow => (&self) -> f32 { self.flex_grow_val }

    tab_index => (&self) -> i32 { i32::from(!self.actions.is_empty()) }

    build => (&self) -> Vec<Box<dyn WidgetComponent>> {
        self.children.take()
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerEnter if self.hoverable => {
                self.hovered = true;
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                let changed = self.hovered || self.hovered_action.get().is_some();
                self.hovered = false;
                self.hovered_action.set(None);
                if changed { EventResult::Handled } else { EventResult::NotHandled }
            }
            SystemEvent::PointerMove { pos, .. } if !self.actions.is_empty() => {
                let action = self.action_index_at(*pos);
                let changed = action != self.hovered_action.get();
                self.hovered_action.set(action);
                if changed { EventResult::Handled } else { EventResult::NotHandled }
            }
            SystemEvent::PointerDown { pos, button: MouseButton::Left, .. } => {
                if let Some(index) = self.action_index_at(*pos) {
                    self.focused = true;
                    self.focused_action = index;
                    self.submit_action(index);
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::FocusIn if !self.actions.is_empty() => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } if !self.actions.is_empty() => match key {
                KeyCode::Left => {
                    self.focused_action = self.focused_action.saturating_sub(1);
                    EventResult::Handled
                }
                KeyCode::Right => {
                    self.focused_action = (self.focused_action + 1).min(self.actions.len() - 1);
                    EventResult::Handled
                }
                KeyCode::Home => {
                    self.focused_action = 0;
                    EventResult::Handled
                }
                KeyCode::End => {
                    self.focused_action = self.actions.len() - 1;
                    EventResult::Handled
                }
                KeyCode::Enter | KeyCode::Space => {
                    self.submit_action(self.focused_action);
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_submit
            .borrow_mut()
            .take()
            .map(|action| SemanticEvent::submit(id, action))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        if ctx.paint_pass() != PaintPass::Content {
            return;
        }
        let frame = Rect::new(frame.x, frame.y, frame.w.max(0.0), frame.h.max(0.0));
        self.last_frame.set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        let border_radius_lg = ctx.tokens().border_radius_lg();
        let bg_container = ctx.tokens().color_bg_container();
        let bg_elevated = ctx.tokens().color_bg_elevated();
        let primary = ctx.tokens().color_primary();
        let border_secondary = ctx.tokens().color_border_secondary();
        let text = ctx.tokens().color_text();

        let card_radius = Some(Radius::uniform(
            border_radius_lg.min(frame.w * 0.5).min(frame.h * 0.5).max(0.0),
        ));

        // 阴影绘制由图形引擎内部处理 clip 绕过。
        // 引擎的 draw_box_shadow 会自动恢复到脏区域 clip，
        // 绕过父级 children_clip 但保持在脏区域内。
        draw_elevation_shadow(ctx, frame, self.elevation);

        ctx.push_clip(frame);

        // Background
        let bg = if self.hovered {
            // 悬浮时向白色混合 5%（等价 Color::lighten(0.05)，基于主题底色 token）。
            bg_container.lighten(0.05)
        } else {
            bg_elevated
        };
        ctx.fill_rect(frame, bg, card_radius);

        // Top accent line
        if self.elevation > 1 {
            let inset = 24.0f32.min(frame.w * 0.5);
            let accent_w = (frame.w - inset * 2.0).max(0.0);
            let accent_h = 3.0f32.min(frame.h);
            if accent_w > 0.0 && accent_h > 0.0 {
                let accent_rect = Rect::new(frame.x + inset, frame.y, accent_w, accent_h);
                ctx.fill_rect(
                    accent_rect,
                    primary,
                    Some(Radius::uniform((accent_h * 0.5).min(accent_w * 0.5))),
                );
            }
        }

        // Border
        if self.bordered {
            ctx.stroke_rect(frame, border_secondary, 1.0, card_radius);
        }

        // Title
        if let Some(ref title) = self.title {
            if let Some(title_rect) = self.title_text_rect(frame) {
                if let Some((visible_title, font_size)) = fitted_text(
                    ctx,
                    title,
                    Self::TITLE_FONT_SIZE,
                    Self::TITLE_MIN_FONT_SIZE,
                    title_rect.w,
                    (title_rect.h - Self::TITLE_VERTICAL_INSET * 2.0).max(0.0),
                ) {
                    let title_y = ctx.visual_center_y(title_rect, font_size);
                    ctx.push_clip(title_rect);
                    ctx.draw_text(
                        &visible_title,
                        Point::new(title_rect.x, title_y),
                        text,
                        font_size,
                    );
                    ctx.pop_clip();
                }
            }
            if let Some(separator) = self.title_separator_rect(frame) {
                ctx.fill_rect(separator, border_secondary, None);
            }
        }

        // Actions
        if let Some(action_rect) = self.action_rect(frame) {
            ctx.fill_rect(action_rect, bg_container, None);
            ctx.stroke_rect(action_rect, border_secondary, 1.0, None);
            let btn_w = action_rect.w / self.actions.len() as f32;
            for (i, action) in self.actions.iter().enumerate() {
                let btn_rect = Rect::new(
                    action_rect.x + i as f32 * btn_w,
                    action_rect.y,
                    btn_w,
                    action_rect.h,
                );
                if self.hovered_action.get() == Some(i) {
                    ctx.fill_rect(btn_rect, ctx.tokens().color_fill_tertiary(), None);
                }
                if self.focused && tree.keyboard_focus_visible() && self.focused_action == i {
                    let inset = 1.0f32.min(btn_rect.w * 0.5).min(btn_rect.h * 0.5);
                    ctx.stroke_rect(
                        Rect::new(
                            btn_rect.x + inset,
                            btn_rect.y + inset,
                            (btn_rect.w - inset * 2.0).max(0.0),
                            (btn_rect.h - inset * 2.0).max(0.0),
                        ),
                        primary,
                        2.0,
                        None,
                    );
                }
                if let Some((visible_action, font_size)) = fitted_text(
                    ctx,
                    action,
                    Self::ACTION_FONT_SIZE,
                    Self::ACTION_MIN_FONT_SIZE,
                    (btn_rect.w - Self::ACTION_HORIZONTAL_INSET * 2.0).max(0.0),
                    (btn_rect.h - Self::ACTION_VERTICAL_INSET * 2.0).max(0.0),
                ) {
                    let ay = ctx.visual_center_y(btn_rect, font_size);
                    let text_w = ctx.measure_text(&visible_action, font_size).w;
                    ctx.push_clip(btn_rect);
                    ctx.draw_text(
                        &visible_action,
                        Point::new(btn_rect.x + (btn_w - text_w) * 0.5, ay),
                        primary,
                        font_size,
                    );
                    ctx.pop_clip();
                }
                if i < self.actions.len() - 1 {
                    let divider_h = (action_rect.h - 16.0).max(0.0);
                    ctx.fill_rect(
                        Rect::new(
                            btn_rect.x + btn_w - 1.0,
                            action_rect.y + 8.0f32.min(action_rect.h),
                            1.0,
                            divider_h,
                        ),
                        border_secondary,
                        None,
                    );
                }
            }
        }
        ctx.pop_clip();
    }

    // 扩展脏区域覆盖完整阴影渲染范围。
    // 引擎的 draw_box_shadow 会自动恢复到脏区域 clip（绕过父级），
    // 若 dirty_rect 不覆盖阴影边界，会产生像素叠加拖影。
    dirty_rect => (&self, frame: Rect) -> Rect {
        let expand = match self.elevation {
            // elevation 1: layer_3 blur=48, bounds=±49px → ±50
            1 => 50.0,
            // elevation 2: layer_3 blur=66, bounds=±67px → ±70
            2 => 70.0,
            // elevation 3: layer_3 blur=90, bounds=±91px → ±95
            3 => 95.0,
            _ => 0.0,
        };
        if expand > 0.0 {
            Rect::new(frame.x - expand, frame.y - expand, frame.w + expand * 2.0, frame.h + expand * 2.0)
        } else {
            frame
        }
    }

    measure_children => (&self, frame: Rect, children: &[ComponentId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        let inner = self.body_rect(frame);
        let constraints = Constraints::loose(Size::new(inner.w, inner.h));
        children
            .iter()
            .map(|&id| {
                let mut child = child_from_tree_with_constraints(id, tree, constraints);
                child.measured_size = constraints.clamp(child.measured_size);
                if tree.get(id).and_then(|node| node.as_layout()).is_none() {
                    child.flex_shrink = 0.0;
                }
                child
            })
            .collect()
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        if children.is_empty() { return Vec::new(); }

        let inner = self.body_rect(frame);
        if inner.w <= 0.0 || inner.h <= 0.0 {
            return children
                .iter()
                .map(|child| (child.id, Rect::new(inner.x, inner.y, 0.0, 0.0)))
                .collect();
        }

        let flex_children: Vec<FlexChild> = children
            .iter()
            .map(|child| FlexChild {
                flex_grow: child.flex_grow,
                flex_shrink: child.flex_shrink,
                align_self: child.align_self,
                measured_size: child.measured_size,
                // Card 的定制 Flex 转换必须保留共享子项外边距契约。
                margin: child.margin,
                ..FlexChild::default()
            })
            .collect();

        let input = FlexInput {
            direction: FlexDirection::Column,
            gap: 0.0,
            padding: EdgeInsets::zero(),
            container: inner,
            children: &flex_children,
            justify_content: JustifyContent::Start,
            align_items: AlignItems::Stretch,
            ..FlexInput::default()
        };

        let output = compute_flex_layout(&input);
        children
            .iter()
            .zip(output.child_rects)
            .map(|(child, rect)| (child.id, rect))
            .collect()
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        Some(self.body_rect(frame))
    }
}

/// Render a multi-layer elevation shadow with directional and ambient layers.
///
/// Three distinct layers produce a rich, realistic shadow:
/// 1. **Contact shadow** (standard smoothstep) — tight, y-down offset, sharp edge
/// 2. **Ambient shadow** (super-gaussian falloff) — wider, softer, spread around
/// 3. **Diffuse glow** (super-gaussian) — very wide, no offset, fills uniformly
///
/// Each elevation level scales the layers differently so the visual depth
/// increases naturally: the contact shadow grows slightly, while the ambient
/// and glow layers expand significantly.
fn draw_elevation_shadow(ctx: &mut PaintContext, frame: Rect, elevation: u8) {
    if elevation == 0 {
        return;
    }
    let shadow = ctx.tokens().box_shadow();
    let corner_radius = Some(Radius::uniform(ctx.tokens().border_radius_lg()));

    // Per-elevation scaling factors for each layer role:
    //   (directional_scale, ambient_spread, glow_spread, alpha_boost)
    let (dir_s, amb_s, glow_s, alpha_b) = match elevation {
        1 => (0.8, 1.2, 1.6, 1.1),
        2 => (1.0, 1.6, 2.2, 1.0),
        3 => (1.2, 2.2, 3.0, 0.9),
        _ => return,
    };

    let boost = |c: Color| c.with_alpha((c.a as f32 * alpha_b).min(255.0) as u8);

    // ── Layer 1: Contact shadow ──
    // Directional (y-down), tight blur, sharp smoothstep falloff.
    let (ox1, oy1, bl1, col1) = shadow.layer_1;
    if bl1 > 0.0 && col1.a > 0 {
        ctx.draw_box_shadow(
            frame,
            bl1 * dir_s,       // blur
            ox1,               // x-offset (0 = centered contact)
            oy1 * dir_s * 1.5, // y-offset (emphasise downward)
            boost(col1),
            corner_radius,
        );
    }

    // ── Layer 2: Ambient shadow ──
    // Wider soft shadow that spreads around the card.
    // Uses ambient falloff (super-gaussian) for a softer transition.
    let (ox2, oy2, bl2, col2) = shadow.layer_2;
    if bl2 > 0.0 && col2.a > 0 {
        ctx.draw_box_shadow_ambient(
            frame,
            bl2 * amb_s,       // larger blur = wider ambient
            ox2 * 0.3,         // slight x-spread
            oy2 * amb_s * 0.6, // moderate y-offset
            boost(col2),
            corner_radius,
        );
    }

    // ── Layer 3: Diffuse glow ──
    // Wide, uniform glow with no directional bias.
    // Pure ambient occlusion fill — creates the "floating" feel.
    let (_, _, bl3, col3) = shadow.layer_3;
    if bl3 > 0.0 && col3.a > 0 {
        ctx.draw_box_shadow_ambient(
            frame,
            bl3 * glow_s, // very wide blur
            0.0,
            0.0, // no offset — uniform
            boost(col3),
            corner_radius,
        );
    }
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}

impl Card {
    const ACTION_HEIGHT: f32 = 40.0;
    // Card 动作区字号（13.0）；Message/Notification toast 家族同名常量为 12.0，属各自设计。
    const ACTION_FONT_SIZE: f32 = 13.0;
    const ACTION_HORIZONTAL_INSET: f32 = 6.0;
    const ACTION_MIN_FONT_SIZE: f32 = 10.0;
    const ACTION_VERTICAL_INSET: f32 = 2.0;
    // Card 默认尺寸；各组件同名常量（descriptions 600 / selectable_list 220 / 图表 300 等）为各自设计。
    const DEFAULT_WIDTH: f32 = 200.0;
    const DEFAULT_HEIGHT: f32 = 120.0;
    const TITLE_BLOCK_HEIGHT: f32 = 56.0;
    const TITLE_FONT_SIZE: f32 = 15.0;
    const TITLE_MIN_FONT_SIZE: f32 = 11.0;
    const TITLE_SEPARATOR_OFFSET: f32 = 48.0;
    const TITLE_TEXT_HEIGHT: f32 = 44.0;
    const TITLE_VERTICAL_INSET: f32 = 2.0;

    fn body_rect(&self, frame: Rect) -> Rect {
        // 标题和 actions 为固定区，body 只使用二者之间的剩余空间。
        let frame_w = frame.w.max(0.0);
        let frame_h = frame.h.max(0.0);
        let padding = self.padding.max(0.0);
        let title_offset = if self.title.is_some() {
            Self::TITLE_BLOCK_HEIGHT
        } else {
            padding
        }
        .min(frame_h);
        let action_top = self
            .action_rect(frame)
            .map(|rect| rect.y)
            .unwrap_or(frame.y + frame_h);
        let body_y = (frame.y + title_offset).min(action_top);
        let left_padding = padding.min(frame_w);
        let right_padding = padding.min((frame_w - left_padding).max(0.0));
        let bottom_padding = padding.min((action_top - body_y).max(0.0));
        Rect::new(
            frame.x + left_padding,
            body_y,
            (frame_w - left_padding - right_padding).max(0.0),
            (action_top - body_y - bottom_padding).max(0.0),
        )
    }

    fn padded_horizontal_rect(&self, frame: Rect, y: f32, height: f32) -> Rect {
        let frame_w = frame.w.max(0.0);
        let padding = self.padding.max(0.0);
        let left_padding = padding.min(frame_w);
        let right_padding = padding.min((frame_w - left_padding).max(0.0));
        Rect::new(
            frame.x + left_padding,
            y,
            (frame_w - left_padding - right_padding).max(0.0),
            height.max(0.0),
        )
    }

    // `max`/`min` intentionally collapse a NaN delta to zero; `clamp` would retain NaN.
    #[allow(clippy::manual_clamp)]
    fn title_available_height(&self, frame: Rect) -> f32 {
        let frame_h = frame.h.max(0.0);
        let action_top = self
            .action_rect(frame)
            .map(|rect| rect.y)
            .unwrap_or(frame.y + frame_h);
        (action_top - frame.y)
            .max(0.0)
            .min(Self::TITLE_BLOCK_HEIGHT)
    }

    fn title_text_rect(&self, frame: Rect) -> Option<Rect> {
        self.title.as_ref()?;
        let height = self
            .title_available_height(frame)
            .min(Self::TITLE_TEXT_HEIGHT);
        (height > 0.0).then(|| self.padded_horizontal_rect(frame, frame.y, height))
    }

    fn title_separator_rect(&self, frame: Rect) -> Option<Rect> {
        self.title.as_ref()?;
        let available_height = self.title_available_height(frame);
        if available_height < Self::TITLE_SEPARATOR_OFFSET + 1.0 {
            return None;
        }
        let rect = self.padded_horizontal_rect(frame, frame.y + Self::TITLE_SEPARATOR_OFFSET, 1.0);
        (rect.w > 0.0).then_some(rect)
    }

    pub(crate) fn action_rect(&self, frame: Rect) -> Option<Rect> {
        if self.actions.is_empty() {
            return None;
        }
        let frame_h = frame.h.max(0.0);
        let action_h = Self::ACTION_HEIGHT.min(frame_h);
        Some(Rect::new(
            frame.x,
            frame.y + frame_h - action_h,
            frame.w.max(0.0),
            action_h,
        ))
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(
            self.fixed_width.unwrap_or(Self::DEFAULT_WIDTH),
            self.fixed_height.unwrap_or(Self::DEFAULT_HEIGHT),
        )
    }

    pub fn new() -> Self {
        Self {
            title: None,
            children: WidgetChildren::new(),
            bordered: true,
            hoverable: false,
            hovered: false,
            fixed_width: None,
            fixed_height: None,
            padding: 16.0,
            elevation: 1,
            flex_grow_val: 0.0,
            actions: Vec::new(),
            focused: false,
            focused_action: 0,
            hovered_action: Cell::new(None),
            last_frame: Cell::new(None),
            pending_submit: RefCell::new(None),
        }
    }

    pub fn title(mut self, t: &str) -> Self {
        self.title = Some(t.to_string());
        self
    }
    pub fn bordered(mut self, v: bool) -> Self {
        self.bordered = v;
        self
    }
    pub fn hoverable(mut self) -> Self {
        self.hoverable = true;
        self
    }
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.fixed_width = Self::optional_dimension(w);
        self.fixed_height = Self::optional_dimension(h);
        self
    }
    pub fn padding(mut self, p: f32) -> Self {
        self.padding = if p.is_finite() { p.max(0.0) } else { 0.0 };
        self
    }
    pub fn elevation(mut self, e: u8) -> Self {
        self.elevation = e.min(3);
        self
    }
    pub fn flex_grow(mut self, v: f32) -> Self {
        self.flex_grow_val = if v.is_finite() { v.max(0.0) } else { 0.0 };
        self
    }
    pub fn actions(mut self, list: Vec<impl Into<String>>) -> Self {
        self.actions = list
            .into_iter()
            .map(Into::into)
            .filter(|action: &String| !action.trim().is_empty())
            .collect();
        self
    }
    pub fn focused_action(&self) -> Option<usize> {
        (!self.actions.is_empty()).then_some(self.focused_action)
    }
    pub fn action_labels(&self) -> &[String] {
        &self.actions
    }
    pub fn child(self, w: impl WidgetComponent + 'static) -> Self {
        self.children.add(w);
        self
    }
    pub fn children(self, widgets: Vec<Box<dyn WidgetComponent>>) -> Self {
        self.children.set_all(widgets);
        self
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Card {
            title: self.title.clone(),
            bordered: self.bordered,
            hoverable: self.hoverable,
            fixed_width: self.fixed_width,
            fixed_height: self.fixed_height,
            padding: self.padding,
            elevation: self.elevation,
            flex_grow: self.flex_grow_val,
            actions: self.actions.clone(),
            focused_action: self.focused_action(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.title = next.title;
        self.bordered = next.bordered;
        self.hoverable = next.hoverable;
        self.fixed_width = next.fixed_width;
        self.fixed_height = next.fixed_height;
        self.padding = next.padding;
        self.elevation = next.elevation;
        self.flex_grow_val = next.flex_grow_val;
        self.actions = next.actions;
        self.focused_action = self
            .focused_action
            .min(self.actions.len().saturating_sub(1));
        self.hovered_action.set(
            self.hovered_action
                .get()
                .filter(|index| *index < self.actions.len()),
        );
        let pending_is_valid = self
            .pending_submit
            .borrow()
            .as_ref()
            .is_none_or(|pending| self.actions.contains(pending));
        if !pending_is_valid {
            self.pending_submit.borrow_mut().take();
        }
        if self.actions.is_empty() {
            self.focused = false;
        }
    }

    fn optional_dimension(value: f32) -> Option<f32> {
        (value.is_finite() && value > 0.0).then_some(value)
    }

    fn action_index_at(&self, pos: Point) -> Option<usize> {
        let frame = self.last_frame.get()?;
        let action_rect = self.action_rect(frame)?;
        if !action_rect.contains(pos) || action_rect.w <= 0.0 {
            return None;
        }
        let width = action_rect.w / self.actions.len() as f32;
        let index = ((pos.x - action_rect.x) / width) as usize;
        Some(index.min(self.actions.len() - 1))
    }

    fn submit_action(&self, index: usize) {
        if let Some(action) = self.actions.get(index) {
            self.pending_submit.replace(Some(action.clone()));
        }
    }

    // 测试目标保留卡片 frame 注入入口，供交互几何测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn set_frame_for_test(&self, frame: Rect) {
        self.last_frame.set(Some(Rect::new(
            0.0,
            0.0,
            frame.w.max(0.0),
            frame.h.max(0.0),
        )));
    }
}

fn fitted_text(
    ctx: &mut PaintContext,
    text: &str,
    base_size: f32,
    min_size: f32,
    max_width: f32,
    max_height: f32,
) -> Option<(String, f32)> {
    if !base_size.is_finite()
        || base_size <= 0.0
        || !min_size.is_finite()
        || min_size <= 0.0
        || !max_width.is_finite()
        || max_width <= 0.0
        || !max_height.is_finite()
        || max_height <= 0.0
    {
        return None;
    }
    let visible = text.replace(['\r', '\n'], " ");
    let base_height = conservative_text_height(ctx, &visible, base_size);
    let height_scale = if base_height > 0.0 {
        (max_height / base_height).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let font_size = (base_size * height_scale).min(base_size);
    let minimum = min_size.min(base_size);
    let font_size = if font_size >= minimum {
        font_size
    } else if conservative_text_height(ctx, &visible, minimum) <= max_height {
        minimum
    } else {
        return None;
    };
    let visible = elide_text_to_width(ctx, &visible, font_size, max_width)?;
    Some((visible, font_size))
}

fn conservative_text_width(ctx: &mut PaintContext, text: &str, font_size: f32) -> f32 {
    let measured = ctx.measure_text(text, font_size).w;
    let estimated = crate::draw::resources::font::text_backend::estimate_text_metrics(
        text,
        f32::INFINITY,
        font_size,
    )
    .max_line_width;
    measured.max(estimated)
}

fn conservative_text_height(ctx: &mut PaintContext, text: &str, font_size: f32) -> f32 {
    ctx.measure_text(text, font_size)
        .h
        .max(ctx.line_box_height(font_size))
}

fn elide_text_to_width(
    ctx: &mut PaintContext,
    text: &str,
    font_size: f32,
    max_width: f32,
) -> Option<String> {
    if conservative_text_width(ctx, text, font_size) <= max_width {
        return Some(text.to_string());
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
