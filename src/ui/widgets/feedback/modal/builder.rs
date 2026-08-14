use super::{ControlledOpen, Modal};

use crate::native::windowing::input::ControlSize;
use crate::ui::State;
use crate::ui::animation::AnimationConfig;
use std::rc::Rc;

/// Modal 内容回调上下文；关闭请求只作用于持有该上下文的 Modal。

pub struct ModalBuilder {
    pub(crate) modal: Modal,
    // 保存按声明顺序进入 Modal 内容区的全部 View。
    pub(crate) content: Vec<crate::ui::view::ViewNode>,
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
        // 单内容构建器替换现有内容集合，保持既有 API 的覆盖语义。
        self.content = vec![crate::ui::view::View::build(content())];
        // 返回更新后的构建器。
        self
    }

    // 设置已经物化的有序内容 View，供声明式生成器保留 If/For 展开结果。
    pub fn content_nodes(mut self, content: Vec<crate::ui::view::ViewNode>) -> Self {
        // 直接取得 View 集合所有权，避免复制运行节点或生命周期句柄。
        self.content = content;
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

    // 注册确认操作的同步窄回调。
    pub fn on_ok<F>(mut self, callback: F) -> Self
    where
        // 回调由 Modal 实例持有并在有效确认输入中同步调用。
        F: Fn() + 'static,
    {
        // 使用共享所有权让声明式 reconcile 可替换回调而不借用调用方。
        self.modal.ok_callback = Some(Rc::new(callback));
        // 返回更新后的构建器。
        self
    }

    // 注册取消、遮罩、关闭槽与 Escape 共用的同步窄回调。
    pub fn on_cancel<F>(mut self, callback: F) -> Self
    where
        // 回调由 Modal 实例持有并在有效取消输入中同步调用。
        F: Fn() + 'static,
    {
        // 使用共享所有权让声明式 reconcile 可替换回调而不借用调用方。
        self.modal.cancel_callback = Some(Rc::new(callback));
        // 返回更新后的构建器。
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

    /// 为受控 Modal 显式启用统一 backdrop blur 请求。
    pub fn backdrop_blur(mut self, blur: crate::ui::OverlayBackdropBlur) -> Self {
        // 复用 Modal 自身的便捷属性，不建立第二套效果状态。
        self.modal = self.modal.backdrop_blur(blur);
        // 返回更新后的声明式构建器。
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
        // 把完整有序内容集合交给 Modal 运行节点拥有。
        crate::ui::view::ViewNode::new(self.modal, self.content)
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
    // 引入 Modal 私有命中目标以验证同模块运行时契约。
    use super::super::ModalPointerTarget;
    use super::*;
    use crate::ui::component::traits::WidgetAnimation;
    use std::cell::RefCell;
    use std::rc::Rc;

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

    // 验证确认回调、受控状态写回与离场幂等性。
    #[test]
    // 声明确认操作生命周期测试。
    fn footer_ok_callback_closes_once_and_writes_back_state() {
        // 创建初始打开的唯一业务状态源。
        let state = State::new(true);
        // 保存确认回调的调用次数。
        let calls = Rc::new(RefCell::new(0_usize));
        // 克隆回调拥有的计数句柄。
        let hook = calls.clone();
        // 构造带确认回调的受控 Modal。
        let builder = Modal::builder()
            .open(&state)
            .footer_visible(true)
            .on_ok(move || {
                // 每次有效确认只增加一次计数。
                *hook.borrow_mut() += 1;
            });
        // 取得运行时 Modal 以模拟确认目标激活。
        let mut modal = builder.modal;
        // 激活一次确认按钮。
        modal.activate_target(ModalPointerTarget::Ok);
        // 确认回调必须恰好执行一次。
        assert_eq!(*calls.borrow(), 1);
        // 关闭事实必须写回声明端 State。
        assert!(!state.get());
        // 有动画的关闭应进入离场状态。
        assert!(modal.closing);
        // 离场期间重复激活不得再次调用业务回调。
        modal.activate_target(ModalPointerTarget::Ok);
        // 回调计数必须保持不变。
        assert_eq!(*calls.borrow(), 1);
    }

    // 验证取消按钮、关闭槽和遮罩共用同一取消语义。
    #[test]
    // 声明全部取消入口测试。
    fn cancel_targets_share_single_callback_and_state_write_back() {
        // 枚举全部指针取消目标。
        for target in [
            // 底部取消按钮。
            ModalPointerTarget::Cancel,
            // 标题栏关闭槽。
            ModalPointerTarget::Close,
            // 对话框外遮罩。
            ModalPointerTarget::Mask,
        ] {
            // 每个入口使用独立的初始打开状态。
            let state = State::new(true);
            // 保存当前入口的取消回调次数。
            let calls = Rc::new(RefCell::new(0_usize));
            // 克隆回调拥有的计数句柄。
            let hook = calls.clone();
            // 构造带取消回调的受控 Modal。
            let builder = Modal::builder().open(&state).on_cancel(move || {
                // 每次有效取消只增加一次计数。
                *hook.borrow_mut() += 1;
            });
            // 取得运行时 Modal 以模拟入口激活。
            let mut modal = builder.modal;
            // 激活当前取消目标。
            modal.activate_target(target);
            // 当前入口必须调用一次取消回调。
            assert_eq!(*calls.borrow(), 1);
            // 当前入口必须写回关闭状态。
            assert!(!state.get());
        }
    }

    // 验证底部绘制与命中共同消费的按钮几何。
    #[test]
    // 声明底部操作命中测试。
    fn footer_targets_follow_shared_geometry_and_visibility() {
        // 创建打开且显示底部操作的 Modal。
        let mut modal = Modal::new("").visible(true).footer_visible(true);
        // 设置稳定的本地组件 frame。
        modal
            .last_frame
            .set(crate::core::Rect::new(0.0, 0.0, 520.0, 300.0));
        // 设置绘制阶段确认的最终对话框矩形。
        modal
            // 保存与命中共用的最终几何。
            .last_dialog_rect
            // 使用标准 Modal 尺寸。
            .set(crate::core::Rect::new(0.0, 0.0, 520.0, 300.0));
        // 取得取消与确认按钮的共享矩形。
        let (cancel, ok) = Modal::footer_action_rects(modal.last_dialog_rect.get());
        // 取消中心必须命中取消目标。
        assert_eq!(
            // 查询取消中心的目标。
            modal.pointer_target_at(crate::core::Point::new(
                cancel.x + cancel.w * 0.5,
                cancel.y + cancel.h * 0.5
            )),
            // 期望底部取消目标。
            Some(ModalPointerTarget::Cancel)
        );
        // 确认中心必须命中确认目标。
        assert_eq!(
            // 查询确认中心的目标。
            modal.pointer_target_at(crate::core::Point::new(
                ok.x + ok.w * 0.5,
                ok.y + ok.h * 0.5,
            )),
            // 期望底部确认目标。
            Some(ModalPointerTarget::Ok)
        );
        // 隐藏底部操作区。
        modal.footer_visible = false;
        // 原确认位置不得继续命中隐藏操作。
        assert_eq!(
            // 再次查询原确认中心。
            modal.pointer_target_at(crate::core::Point::new(
                ok.x + ok.w * 0.5,
                ok.y + ok.h * 0.5,
            )),
            // 对话框内容区没有内置目标。
            None
        );
    }

    // 验证 ModalBuilder 不压缩或重排多个内容 View。
    #[test]
    // 声明多内容集合构建测试。
    fn content_nodes_preserve_all_ordered_views() {
        // 物化第一个内容 View。
        let first = crate::ui::view::View::build(crate::ui::widgets::label("第一项"));
        // 物化第二个内容 View。
        let second = crate::ui::view::View::build(crate::ui::widgets::label("第二项"));
        // 构建持有两个有序内容节点的 Modal。
        let view = crate::ui::view::View::build(
            // 直接交出有序 View 集合。
            Modal::builder().content_nodes(vec![first, second]),
        );
        // 构建结果必须保留全部两个内容节点。
        assert_eq!(view.children.len(), 2);
    }
}
