//! FloatButtonGroup 的展开状态、子树所有权、布局与绘制内核。

// 引入父模块拥有的 FloatButton 与共享契约。
use super::*;
// 引入组内过渡运行器与共享折叠预设。
use crate::ui::animation::{TransitionPlayer, presets};
// 引入声明树组合所需的公开 View 契约。
use crate::ui::view::{View, ViewNode};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// 保存 FloatButtonGroup 的触发器、子项间距与保守损伤外扩。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct FloatButtonGroupGeometryVisual {
    pub(super) trigger_size: f32,
    pub(super) item_gap: f32,
    pub(super) dirty_left_outset: f32,
    pub(super) dirty_top_outset: f32,
    pub(super) dirty_width_extra: f32,
    pub(super) dirty_height_extra: f32,
}

// 保存 FloatButtonGroup 触发器阴影、焦点与图标参数。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct FloatButtonGroupPaintVisual {
    pub(super) radius_factor: f32,
    pub(super) shadow_blur: f32,
    pub(super) shadow_offset_x: f32,
    pub(super) shadow_offset_y: f32,
    pub(super) shadow_alpha: u8,
    pub(super) focus_stroke_width: f32,
    pub(super) icon_size: f32,
    pub(super) expanded_icon: &'static str,
    pub(super) collapsed_icon: &'static str,
}

// 保存 FloatButtonGroup 使用的主题颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FloatButtonGroupPaletteVisual {
    primary: ColorValue,
    primary_hover: ColorValue,
    primary_active: ColorValue,
    foreground: ColorValue,
    shadow: ColorValue,
    focus_border: ColorValue,
}

// 全部 FloatButtonGroup 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct FloatButtonGroupVisual {
    pub(super) geometry: FloatButtonGroupGeometryVisual,
    pub(super) paint: FloatButtonGroupPaintVisual,
    palette: FloatButtonGroupPaletteVisual,
}

crate::uix_items!("src/ui/widgets/general/float_button/group/group.uix");

// 保存一次绘制解析后的主题颜色，避免同一帧重复查询 token。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ResolvedFloatButtonGroupVisual {
    pub(super) primary: Color,
    pub(super) primary_hover: Color,
    pub(super) primary_active: Color,
    pub(super) foreground: Color,
    pub(super) shadow: Color,
    pub(super) focus_border: Color,
}

impl FloatButtonGroupVisual {
    pub(super) fn resolve(self, tokens: &dyn ThemeTokens) -> ResolvedFloatButtonGroupVisual {
        ResolvedFloatButtonGroupVisual {
            primary: self.palette.primary.resolve(tokens),
            primary_hover: self.palette.primary_hover.resolve(tokens),
            primary_active: self.palette.primary_active.resolve(tokens),
            foreground: self.palette.foreground.resolve(tokens),
            shadow: self.palette.shadow.resolve(tokens),
            focus_border: self.palette.focus_border.resolve(tokens),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct FloatButtonGroupItemLayout {
    // 保存组内子按钮的基础直径。
    size: f32,
    // 保存包含 description 的相对命中区域。
    hit_bounds: Rect,
    // 保存包含阴影、徽标与提示框的相对绘制区域。
    paint_bounds: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FloatButtonGroupPressTarget {
    Trigger,
}

widget! {
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
        /// 同目录 UIX 生成的唯一静态视觉表。
        pub(crate) visual: &'static group::FloatButtonGroupVisual,
    }

    build_view_children => (&self) -> Vec<crate::ui::view::ViewNode> {
        self.buttons
            .borrow_mut()
            .take()
            .unwrap_or_default()
            .into_iter()
            .map(crate::ui::view::View::build)
            .collect()
    }

    tab_index => (&self) -> i32 { i32::from(!self.item_layouts.is_empty()) }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(
            self.visual.geometry.trigger_size,
            self.visual.geometry.trigger_size,
        ))
    }

    child_overflow_expands_parent => (&self) -> bool { false }

    child_visible => (&self, index: usize) -> bool {
        index < self.item_layouts.len() && self.children_are_visible()
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::WidgetId, Rect)>
    {
        let progress = self.expansion_progress();
        self.child_frames(frame, progress)
            // 与真实子节点一一配对，任一侧较短时立即结束。
            .zip(children.iter())
            // 父组件只交付子节点身份与单次累加前缀计算的 frame。
            .map(|((_item, frame), child)| (child.id, frame))
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
                    || !self.local_trigger_rect().contains(*pos)
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
                    && self.local_trigger_rect().contains(*pos)
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
        let trigger = self.trigger_rect(frame);
        let resolved = self.visual.resolve(ctx.tokens());
        let radius = Some(Radius::uniform(
            self.visual.geometry.trigger_size * self.visual.paint.radius_factor,
        ));
        let background = if self.pressed_target.is_some() || self.pressed_key.is_some() {
            resolved.primary_active
        } else if self.hovered {
            resolved.primary_hover
        } else {
            resolved.primary
        };
        ctx.draw_box_shadow(
            trigger,
            self.visual.paint.shadow_blur,
            self.visual.paint.shadow_offset_x,
            self.visual.paint.shadow_offset_y,
            // 阴影：黑色 token + 原 alpha（保持视觉等价，色相随主题可换）。
            resolved.shadow.with_alpha(self.visual.paint.shadow_alpha),
            radius,
        );
        ctx.fill_rect(trigger, background, radius);
        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                trigger,
                resolved.focus_border,
                self.visual.paint.focus_stroke_width,
                radius,
            );
        }
        crate::ui::widgets::general::icon::Icon::paint_in_frame(
            ctx,
            if self.expanded {
                self.visual.paint.expanded_icon
            } else {
                self.visual.paint.collapsed_icon
            },
            trigger,
            // 触发图标：白色 token。
            resolved.foreground,
            self.visual.paint.icon_size,
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
    /// 创建默认由悬停触发展开的空浮动按钮组。
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
            visual: FLOAT_BUTTON_GROUP_VISUAL_REF,
        }
    }

    /// 设置由按钮组直接持有和布局的浮动按钮列表。
    pub fn buttons(mut self, mut buttons: Vec<FloatButton>) -> Self {
        self.item_layouts = buttons.iter_mut().map(Self::prepare_button).collect();
        self.buttons = Rc::new(RefCell::new(Some(buttons)));
        self
    }

    /// 接收保留样式、key 与事件处理器的直接 FloatButton View 子项。
    pub fn button_views(mut self, mut buttons: Vec<ViewNode>) -> FloatButtonGroupView {
        // 在声明发布前验证每个直接子根，并同步父组件拥有的几何派生状态。
        self.item_layouts = buttons
            // 可变遍历只用于标记组内布局语义，不接管子节点事件。
            .iter_mut()
            // 把每个已验证子按钮映射为父组件布局记录。
            .map(|button| {
                // 直接子根必须是 FloatButton，防止子树数量与布局记录失配。
                let button = button
                    // 只借用组件对象，不移动 ViewNode 上的 handler、key 或样式。
                    .widget
                    // 取得运行时类型检查所需的可变 Any 视图。
                    .as_any_mut()
                    // 把组件窄化为 FloatButton。
                    .downcast_mut::<FloatButton>();
                // 非 FloatButton 子根属于公开构建契约错误，应在发布前立即失败。
                let Some(button) = button else {
                    // 公开构建契约被违反时保留明确诊断。
                    panic!("FloatButtonGroup::button_views 的直接子根必须是 FloatButton");
                };
                // 复用兼容入口相同的组内标记与几何推导。
                Self::prepare_button(button)
            })
            // 固化与直接子项一一对应的布局记录。
            .collect();
        // 返回只负责携带父组件与完整子 ViewNode 的窄组合包装器。
        FloatButtonGroupView {
            // 父组件继续独占展开、布局、命中与过渡状态。
            group: self,
            // 子声明继续独占标准事件处理器及其协调身份。
            buttons,
        }
    }

    /// 设置按钮组展开与收起使用的触发模式。
    pub fn trigger(mut self, trigger: TriggerMode) -> Self {
        self.trigger = trigger;
        self
    }

    // 统一标记组内按钮并生成父组件布局记录。
    fn prepare_button(button: &mut FloatButton) -> FloatButtonGroupItemLayout {
        // 组内按钮的 placement 由父组件相对布局覆盖。
        button.in_group = true;
        // 组内按钮使用零 frame 生成相对共享几何。
        let geometry = button.geometry_for_surface(Rect::zero(), Rect::zero());
        // 保存父组件布局与命中所需的相对矩形。
        FloatButtonGroupItemLayout {
            // 保留展开动画使用的基础直径。
            size: button.size,
            // 命中区域包含可选 description。
            hit_bounds: geometry.control,
            // 损伤区域包含阴影、徽标与提示框。
            paint_bounds: geometry.paint_bounds,
        }
    }

    pub(super) fn local_trigger_rect(&self) -> Rect {
        Rect::new(
            0.0,
            0.0,
            self.visual.geometry.trigger_size,
            self.visual.geometry.trigger_size,
        )
    }

    pub(super) fn trigger_rect(&self, frame: Rect) -> Rect {
        Rect::new(
            frame.x,
            frame.y,
            self.visual.geometry.trigger_size,
            self.visual.geometry.trigger_size,
        )
    }

    pub(super) fn accepts_pointer_button(&self, button: MouseButton) -> bool {
        match self.trigger {
            TriggerMode::Click => button == MouseButton::Left,
            TriggerMode::ContextMenu => button == MouseButton::Right,
            TriggerMode::Hover | TriggerMode::Focus => false,
        }
    }

    pub(super) fn open(&mut self) {
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

    pub(super) fn close(&mut self) {
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

    pub(super) fn toggle(&mut self) {
        if self.expanded && !self.closing {
            self.close();
        } else {
            self.open();
        }
    }

    pub(super) fn cancel_pending_activation(&mut self) {
        self.pressed_target = None;
        self.pressed_button = None;
        self.pressed_key = None;
    }

    pub(super) fn is_present(&self) -> bool {
        self.expanded || self.closing
    }

    pub(super) fn expansion_progress(&self) -> f32 {
        if self.is_present() {
            self.transition.opacity_progress.clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    pub(super) fn children_are_visible(&self) -> bool {
        self.is_present() && self.expansion_progress() > 0.0
    }

    // 单次线性遍历产生全部子按钮 frame，避免每项重新扫描前缀。
    pub(super) fn child_frames(
        &self,
        frame: Rect,
        progress: f32,
    ) -> impl Iterator<Item = (FloatButtonGroupItemLayout, Rect)> + '_ {
        // 前缀高度只在当前遍历中累加一次，不增加持久内存。
        let mut preceding_height = 0.0;
        // 按稳定子项顺序消费只读几何记录。
        self.item_layouts.iter().copied().map(move |item| {
            // 使用已累加的前缀计算当前子项。
            let child = self.child_frame(frame, item, preceding_height, progress);
            // 为下一项累加当前直径与固定间距。
            preceding_height += item.size + self.visual.geometry.item_gap;
            // 同时交付命中/损伤计算仍需要的相对几何。
            (item, child)
        })
    }

    // 使用已累加前缀计算单个子按钮在当前过渡中的 frame。
    fn child_frame(
        &self,
        frame: Rect,
        item: FloatButtonGroupItemLayout,
        preceding_height: f32,
        progress: f32,
    ) -> Rect {
        let trigger_size = self.visual.geometry.trigger_size;
        let collapsed_x = frame.x + (trigger_size - item.size) * 0.5;
        let collapsed_y = frame.y + (trigger_size - item.size) * 0.5;
        let expanded_y = frame.y + trigger_size + self.visual.geometry.item_gap + preceding_height;
        Rect::new(
            collapsed_x,
            collapsed_y + (expanded_y - collapsed_y) * progress,
            item.size,
            item.size,
        )
    }

    pub(super) fn interaction_bounds(&self, frame: Rect) -> Rect {
        let mut bounds = self.trigger_rect(frame);
        if !self.children_are_visible() {
            return bounds;
        }
        let progress = self.expansion_progress();
        for (item, child) in self.child_frames(frame, progress) {
            // 把子按钮相对命中区域平移到当前动画 frame。
            let hit = item.hit_bounds;
            bounds = bounds.union(&Rect::new(
                // 平移相对横坐标。
                child.x + hit.x,
                // 平移相对纵坐标。
                child.y + hit.y,
                // 使用 description 扩展后的命中宽度。
                hit.w,
                // 使用最终控件高度。
                hit.h,
            ));
        }
        bounds
    }

    pub(super) fn full_dirty_bounds(&self, frame: Rect) -> Rect {
        let trigger = self.trigger_rect(frame);
        let mut bounds = Rect::new(
            trigger.x - self.visual.geometry.dirty_left_outset,
            trigger.y - self.visual.geometry.dirty_top_outset,
            trigger.w + self.visual.geometry.dirty_width_extra,
            trigger.h + self.visual.geometry.dirty_height_extra,
        );
        // 折叠与展开端点各做一次线性前缀遍历。
        for ((item, collapsed), (_, expanded)) in self
            .child_frames(frame, 0.0)
            .zip(self.child_frames(frame, 1.0))
        {
            // 损伤边界同时覆盖过渡两端，保持原有质量。
            for child in [collapsed, expanded] {
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

    // 测试入口：把当前过渡推进到完成，避免向父模块暴露状态字段。
    #[cfg(test)]
    pub(super) fn finish_transition_for_test(&mut self) {
        self.transition.update(1.0);
    }

    // 测试入口：返回组内布局记录数量。
    #[cfg(test)]
    pub(super) fn item_layout_count_for_test(&self) -> usize {
        self.item_layouts.len()
    }

    // 测试入口：返回当前公开展开语义。
    #[cfg(test)]
    pub(super) fn is_expanded_for_test(&self) -> bool {
        self.expanded
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let trigger_changed = self.trigger != next.trigger;
        self.buttons = next.buttons;
        self.item_layouts = next.item_layouts;
        self.trigger = next.trigger;
        self.visual = next.visual;
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

impl View for FloatButtonGroup {
    fn build(self) -> ViewNode {
        build_float_button_group_uix_root(self, Vec::new())
    }
}

/// 保留 FloatButton 直接子 View 完整声明所有权的窄组合包装器。
pub struct FloatButtonGroupView {
    // 父组件拥有展开、布局、命中与过渡状态。
    group: FloatButtonGroup,
    // 子节点拥有样式、身份和标准事件处理器。
    buttons: Vec<ViewNode>,
}

impl View for FloatButtonGroupView {
    // 把父组件与已验证子声明组合为同一棵 View 树。
    fn build(self) -> ViewNode {
        // 直接复用子 ViewNode，避免重建时丢失 handler 或 key。
        build_float_button_group_uix_root(self.group, self.buttons)
    }
}

// 把 FloatButtonGroup Rust 内核、声明子树与 UIX 静态视觉组合为单一节点。
fn build_float_button_group_view(
    mut kernel: FloatButtonGroup,
    children: Vec<ViewNode>,
    visual: &'static FloatButtonGroupVisual,
) -> ViewNode {
    kernel.visual = visual;
    ViewNode::new(kernel, children)
}

// 为 UIX 根提供稳定的 Rust 内核与子树绑定名称。
fn build_float_button_group_uix_root(
    kernel: FloatButtonGroup,
    children: Vec<ViewNode>,
) -> ViewNode {
    crate::uix!("src/ui/widgets/general/float_button/group/group.uix")
}
