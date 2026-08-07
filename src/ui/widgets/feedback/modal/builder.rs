use super::{ControlledOpen, Modal};



use crate::native::windowing::input::ControlSize;
use crate::ui::animation::AnimationConfig;
use crate::ui::State;
use std::rc::Rc;

/// Modal 内容回调上下文；关闭请求只作用于持有该上下文的 Modal。

pub struct ModalBuilder {
    pub(crate) modal: Modal,
    pub(crate) content: crate::ui::view::ViewNode,
}

impl ModalBuilder {
    /// 设置受控打开状态（E-03）：Modal 可见性跟随 `State<bool>`；
    /// 初始为 `true` 时首帧直接呈现。
    pub fn open(mut self, state: &State<bool>) -> Self {
        self.modal.controlled = Some(ControlledOpen::new(state));
        if state.get() {
            self.modal.visible = true;
        }
        self
    }

    /// 注册可见性变化回调（E-03）：用户侧关闭（mask / close 按钮 / Escape）
    /// 与公开 `close()` 时触发；可与受控 State 双向回写。
    pub fn on_open_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(bool) + 'static,
    {
        if let Some(controlled) = &mut self.modal.controlled {
            controlled.on_change = Some(Rc::new(callback));
        }
        self
    }

    /// 设置 Modal 内容（E-03）：`FnOnce` 闭包构建内容 View。
    pub fn content<V>(mut self, content: impl FnOnce() -> V) -> Self
    where
        V: crate::ui::view::View,
    {
        self.content = crate::ui::view::View::build(content());
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.modal.title = title.into();
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.modal.width = Modal::normalize_dimension(width);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.modal.height = Modal::normalize_dimension(height);
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.modal.width = Modal::normalize_dimension(width);
        self.modal.height = Modal::normalize_dimension(height);
        self
    }

    pub fn modal_size(mut self, size: ControlSize) -> Self {
        self.modal = self.modal.modal_size(size);
        self
    }

    pub fn closable(mut self, closable: bool) -> Self {
        self.modal.closable = closable;
        self
    }

    pub fn mask_closable(mut self, mask_closable: bool) -> Self {
        self.modal.mask_closable = mask_closable;
        self
    }

    pub fn footer_visible(mut self, footer_visible: bool) -> Self {
        self.modal.footer_visible = footer_visible;
        self
    }

    pub fn centered(mut self, centered: bool) -> Self {
        self.modal.centered = centered;
        self
    }

    pub fn overlay(mut self, overlay: bool) -> Self {
        self.modal.overlay = overlay;
        self
    }

    pub fn destroy_on_close(mut self, destroy_on_close: bool) -> Self {
        self.modal.destroy_on_close = destroy_on_close;
        self
    }

    pub fn enter_animation(mut self, animation: AnimationConfig) -> Self {
        self.modal = self.modal.enter_animation(animation);
        self
    }

    pub fn leave_animation(mut self, animation: AnimationConfig) -> Self {
        self.modal = self.modal.leave_animation(animation);
        self
    }
}

impl crate::ui::view::View for ModalBuilder {
    fn build(self) -> crate::ui::view::ViewNode {
        crate::ui::view::ViewNode::new(self.modal, vec![self.content])
    }
}

impl crate::ui::IntoWidgetNode for ModalBuilder {
    fn into_node(self) -> crate::ui::component::widget::WidgetNode {
        crate::ui::adapter::ViewAdapter::expand(crate::ui::view::View::build(self))
    }
}

impl From<ModalBuilder> for crate::ui::view::ViewNode {
    fn from(builder: ModalBuilder) -> Self {
        crate::ui::view::View::build(builder)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use super::*;
    use crate::ui::component::traits::WidgetAnimation;

    fn controlled_modal(state: &State<bool>) -> Modal {
        let mut modal = Modal::new("");
        modal.controlled = Some(ControlledOpen::new(state));
        modal
    }

    #[test]
    fn controlled_follows_external_state() {
        let state = State::new(false);
        let mut modal = controlled_modal(&state);
        assert!(!modal.is_visible());

        // 外部 set(true)：下一帧打开。
        state.set(true);
        modal.update_animation(0.016);
        assert!(modal.is_visible());

        // 外部 set(false)：进入离场动画，动画完成后消失。
        state.set(false);
        modal.update_animation(0.016);
        assert!(!modal.is_visible() || modal.is_present());
        modal.update_animation(10.0);
        assert!(!modal.is_present());
    }

    #[test]
    fn controlled_user_close_writes_back_state() {
        let state = State::new(true);
        let mut modal = controlled_modal(&state);
        modal.open();
        assert!(modal.is_visible());

        modal.close();
        assert!(!state.get(), "用户侧关闭应写回受控 State");
    }

    #[test]
    fn controlled_callback_fires_on_open_and_close() {
        let state = State::new(false);
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut modal = Modal::new("");
        let mut controlled = ControlledOpen::new(&state);
        let hook = calls.clone();
        controlled.on_change = Some(Rc::new(move |open| hook.borrow_mut().push(open)));
        modal.controlled = Some(controlled);

        modal.open();
        modal.close();
        assert_eq!(*calls.borrow(), vec![true, false]);
    }

    #[test]
    fn controlled_sync_does_not_loop_on_write_back() {
        // 用户 close() 写回 false 后，下一帧受控同步不得再次关闭/回调。
        let state = State::new(true);
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut modal = Modal::new("");
        let mut controlled = ControlledOpen::new(&state);
        let hook = calls.clone();
        controlled.on_change = Some(Rc::new(move |open| hook.borrow_mut().push(open)));
        modal.controlled = Some(controlled);
        modal.open();

        modal.close();
        modal.update_animation(0.016);
        modal.update_animation(10.0);
        assert_eq!(*calls.borrow(), vec![true, false], "写回不得重复触发回调");
        assert!(!modal.is_present());
    }

    #[test]
    fn builder_open_initializes_visible_from_state() {
        let state = State::new(true);
        let builder = Modal::builder().open(&state).closable(false);
        let view = crate::ui::view::View::build(builder);
        assert_eq!(view.children.len(), 1);
        let _ = view;
    }

    #[test]
    fn declarative_visibility_sync_updates_runtime_lifecycle() {
        // 创建关闭的运行态 Modal，模拟 G5 初始 overlay。
        let mut modal = Modal::new("");
        // 构造声明式打开配置，模拟 overlay_mode 切换到 Modal。
        let open = Modal::new("").visible(true);
        // 将声明式打开同步到复用中的运行节点。
        modal.sync_from(open);
        // 打开后运行态必须参与呈现。
        assert!(modal.is_present());
        // 构造声明式关闭配置，模拟 overlay_mode 离开 Modal。
        let close = Modal::new("").visible(false);
        // 将声明式关闭同步到同一个运行节点。
        modal.sync_from(close);
        // 关闭请求应进入离场状态，而不是重新打开。
        assert!(modal.is_present());
        // 再次同步相同关闭声明，验证离场动画不会被重复启动。
        modal.sync_from(Modal::new("").visible(false));
        // 推进足够长的时间完成离场动画。
        modal.update_animation(10.0);
        // 离场完成后运行态必须完全释放呈现资格。
        assert!(!modal.is_present());
    }
}

