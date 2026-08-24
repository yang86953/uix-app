//! 气泡确认框的构造、声明式配置与可见性生命周期。

use super::*;

impl Popconfirm {
    /// 创建默认置于上方、显示箭头和警告图标且初始隐藏的确认框。
    pub fn new() -> Self {
        let visual = POPCONFIRM_VISUAL_REF;
        Self {
            title: String::new(),
            confirm_text: visual.defaults.confirm_text.to_string(),
            cancel_text: visual.defaults.cancel_text.to_string(),
            visible: false,
            placement: visual.defaults.placement,
            arrow: visual.defaults.arrow,
            icon: visual.defaults.icon,
            // 兼容构造默认使用旧合成触发器。
            custom_trigger: false,
            // 兼容构造不持有自定义 trigger 子树。
            custom_trigger_view: None,
            // 叶构造尚未登记直接 trigger 子树。
            trigger_child_count: 0,
            // 首次布局前没有可用的实际 trigger 尺寸。
            trigger_size: Cell::new(Size::zero()),
            // 默认不执行确认业务回调。
            confirm_callback: None,
            // 默认不执行取消业务回调。
            cancel_callback: None,
            // 新实例尚未提交用户动作。
            action_committed: false,
            transition: TransitionPlayer::new(AnimationConfig::fade_in(
                visual.motion.enter_duration,
            )),
            closing: false,
            transition_dirty: false,
            focused: false,
            focused_action: 0,
            pending_submit: Cell::new(false),
            hovered_target: None,
            pressed_target: None,
            pressed_key: None,
            last_frame: Cell::new(Rect::zero()),
            popup_rect: Cell::new(Rect::new(
                0.0,
                -visual.defaults.popup_height - visual.layout.arrow_gap,
                visual.defaults.popup_width,
                visual.defaults.popup_height,
            )),
            surface_rect: Cell::new(Rect::zero()),
            visual,
        }
    }

    /// 设置确认框标题。
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = t.into();
        self
    }

    /// 设置确认按钮文本。
    pub fn confirm_text(mut self, t: impl Into<String>) -> Self {
        self.confirm_text = t.into();
        self
    }

    /// 设置取消按钮文本。
    pub fn cancel_text(mut self, t: impl Into<String>) -> Self {
        self.cancel_text = t.into();
        self
    }

    /// 设置确认框相对触发区域的放置方向。
    pub fn placement(mut self, p: PopconfirmPlacement) -> Self {
        self.placement = p;
        self
    }

    /// 设置是否绘制指向触发区域的箭头。
    pub fn arrow(mut self, v: bool) -> Self {
        self.arrow = v;
        self
    }

    /// 设置是否显示警告图标。
    pub fn icon(mut self, v: bool) -> Self {
        self.icon = v;
        self
    }

    /// 让 Popconfirm 成为一个完整 trigger View 子树的生命周期 owner。
    pub fn trigger_view<V: crate::ui::view::View>(mut self, trigger: V) -> Self {
        // 关闭旧合成触发器绘制并启用组合生命周期。
        self.custom_trigger = true;
        // 构建并保存包含 handlers、样式与身份的完整 ViewNode。
        self.custom_trigger_view = Some(Rc::new(RefCell::new(Some(
            // 通过公开 View 契约构建调用方 trigger。
            crate::ui::view::View::build(trigger),
        ))));
        // 返回拥有待物化 trigger 子树的组件。
        self
    }

    /// 注册确认按钮使用的同步无载荷回调。
    pub fn on_confirm<F>(mut self, callback: F) -> Self
    where
        // 回调随组件跨帧保存且不允许借用临时值。
        F: Fn() + 'static,
    {
        // 共享回调所有权以支持声明 reconcile。
        self.confirm_callback = Some(Rc::new(callback));
        // 返回配置后的组件。
        self
    }

    /// 注册所有用户取消入口共用的同步无载荷回调。
    pub fn on_cancel<F>(mut self, callback: F) -> Self
    where
        // 回调随组件跨帧保存且不允许借用临时值。
        F: Fn() + 'static,
    {
        // 共享回调所有权以支持声明 reconcile。
        self.cancel_callback = Some(Rc::new(callback));
        // 返回配置后的组件。
        self
    }

    /// 返回确认框是否处于可见阶段。
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// 返回确认框是否可见或仍在执行关闭过渡。
    pub fn is_present(&self) -> bool {
        self.visible || self.closing
    }

    /// 打开确认框、重置动作提交状态并启动进场过渡。
    pub fn open(&mut self) {
        self.cancel_pending_activation();
        self.pending_submit.set(false);
        // 新稳定打开周期允许提交一次用户动作。
        self.action_committed = false;
        self.focused_action = 0;
        self.visible = true;
        self.closing = false;
        self.transition =
            TransitionPlayer::new(AnimationConfig::fade_in(self.visual.motion.enter_duration));
        self.transition_dirty = true;
    }

    pub(crate) fn focused_action(&self) -> Option<usize> {
        self.visible.then_some(self.focused_action)
    }

    /// 关闭确认框，并在当前存在时启动离场过渡。
    pub fn close(&mut self) {
        self.cancel_pending_activation();
        if !self.is_present() {
            self.visible = false;
            self.closing = false;
            self.transition_dirty = false;
            return;
        }
        self.visible = false;
        self.closing = true;
        self.transition =
            TransitionPlayer::new(AnimationConfig::fade_out(self.visual.motion.exit_duration));
        self.transition_dirty = true;
    }
}
