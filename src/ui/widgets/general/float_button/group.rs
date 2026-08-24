// 引入父模块拥有的浮动按钮组私有状态与几何契约。
use super::*;
// 引入声明树组合所需的公开 View 契约。
use crate::ui::view::{View, ViewNode};

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

    pub(super) fn local_trigger_rect() -> Rect {
        Rect::new(
            0.0,
            0.0,
            FLOAT_BUTTON_GROUP_TRIGGER_SIZE,
            FLOAT_BUTTON_GROUP_TRIGGER_SIZE,
        )
    }

    pub(super) fn trigger_rect(frame: Rect) -> Rect {
        Rect::new(
            frame.x,
            frame.y,
            FLOAT_BUTTON_GROUP_TRIGGER_SIZE,
            FLOAT_BUTTON_GROUP_TRIGGER_SIZE,
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
            let child = Self::child_frame(frame, item, preceding_height, progress);
            // 为下一项累加当前直径与固定间距。
            preceding_height += item.size + FLOAT_BUTTON_GROUP_GAP;
            // 同时交付命中/损伤计算仍需要的相对几何。
            (item, child)
        })
    }

    // 使用已累加前缀计算单个子按钮在当前过渡中的 frame。
    fn child_frame(
        frame: Rect,
        item: FloatButtonGroupItemLayout,
        preceding_height: f32,
        progress: f32,
    ) -> Rect {
        let collapsed_x = frame.x + (FLOAT_BUTTON_GROUP_TRIGGER_SIZE - item.size) * 0.5;
        let collapsed_y = frame.y + (FLOAT_BUTTON_GROUP_TRIGGER_SIZE - item.size) * 0.5;
        let expanded_y =
            frame.y + FLOAT_BUTTON_GROUP_TRIGGER_SIZE + FLOAT_BUTTON_GROUP_GAP + preceding_height;
        Rect::new(
            collapsed_x,
            collapsed_y + (expanded_y - collapsed_y) * progress,
            item.size,
            item.size,
        )
    }

    pub(super) fn interaction_bounds(&self, frame: Rect) -> Rect {
        let mut bounds = Self::trigger_rect(frame);
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
        let trigger = Self::trigger_rect(frame);
        let mut bounds = Rect::new(
            trigger.x - 10.0,
            trigger.y - 10.0,
            trigger.w + 20.0,
            trigger.h + 24.0,
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

impl View for FloatButtonGroup {
    fn build(self) -> ViewNode {
        ViewNode::leaf(self)
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
        ViewNode::new(self.group, self.buttons)
    }
}
