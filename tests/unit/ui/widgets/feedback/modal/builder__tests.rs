    // 引入 Modal 私有命中目标以验证同模块运行时契约。
    use super::super::{MODAL_VISUAL_REF, ModalPointerTarget};
    use super::*;
    use crate::ui::widget_runtime::traits::WidgetAnimation;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn controlled_modal(state: &State<bool>) -> Modal {
        let mut modal = Modal::new("");
        modal.controlled = Some(ControlledOpen::new(state));
        modal
    }

    // 连续缩窗时对话框必须保持在当前表面安全边距内。
    #[test]
    fn dialog_geometry_is_responsive_to_small_surface() {
        let modal = Modal::new("")
            .visible(true)
            .overlay(true)
            .size(520.0, 300.0);
        let dialog =
            modal.dialog_rect_for_surface(crate::core::Rect::new(0.0, 0.0, 320.0, 180.0));
        assert_eq!(
            dialog,
            crate::core::Rect::new(16.0, 16.0, 288.0, 148.0)
        );
    }

    // 验证直接构造和声明式 View 都共享 UIX 生成的唯一视觉静态项。
    #[test]
    fn view_build_uses_shared_uix_visual() {
        // 直接构造路径不得拥有第二份 Rust 默认表。
        let modal = Modal::new("标题");
        assert!(std::ptr::eq(modal.visual, MODAL_VISUAL_REF));

        // View 构建必须实际经过同目录 UIX 根并保留 Modal 行为内核。
        let node = crate::ui::view::View::build(modal);
        let modal = node
            .widget
            .as_any()
            .downcast_ref::<Modal>()
            .expect("UIX 根必须保留 Modal Rust 内核");
        assert!(std::ptr::eq(modal.visual, MODAL_VISUAL_REF));
    }

    // 验证三档尺寸、分区几何和静态文案仅由 UIX 视觉项提供。
    #[test]
    fn uix_visual_preserves_modal_defaults_and_geometry() {
        use crate::platform::windowing::ControlSize;

        assert_eq!(
            MODAL_VISUAL_REF.defaults.dimensions(ControlSize::Small),
            (400.0, 200.0)
        );
        assert_eq!(
            MODAL_VISUAL_REF.defaults.dimensions(ControlSize::Medium),
            (520.0, 300.0)
        );
        assert_eq!(
            MODAL_VISUAL_REF.defaults.dimensions(ControlSize::Large),
            (720.0, 400.0)
        );
        assert_eq!(MODAL_VISUAL_REF.layout.header_height, 56.0);
        assert_eq!(MODAL_VISUAL_REF.layout.footer_height, 56.0);
        assert_eq!(MODAL_VISUAL_REF.layout.body_padding, 24.0);
        assert_eq!(MODAL_VISUAL_REF.chrome.trigger_label, "打开 Modal");
    }

    // 验证声明式 Modal 的 blur 请求经过真实布局进入唯一 OverlayStack。
    #[cfg(feature = "test-harness")]
    #[test]
    fn backdrop_blur_reaches_surface_overlay_stack() {
        // 使用与真窗演示一致的固定表面构造测试应用。
        let app = crate::ui::automation::TestApp::new((900.0, 560.0), || {
            // 首帧即打开，覆盖真实验收的生命周期。
            let open = State::new(true);
            // 构造显式强模糊的 Modal 根视图。
            crate::ui::view::View::build(
                Modal::builder()
                    // 绑定首帧打开状态。
                    .open(&open)
                    // 设置稳定标题便于语义布局。
                    .title("Backdrop contract")
                    // 下发可精确断言的显式半径。
                    .backdrop_blur(crate::ui::OverlayBackdropBlur::radius(24.0)),
            )
        });
        // 真实布局必须登记 Modal overlay。
        let entry = app
            // 读取同一 WidgetTree 的唯一 overlay 事实。
            .tree()
            // 读取布局后重建的 OverlayStack。
            .overlay_stack()
            // Modal 是当前最上层 overlay。
            .top()
            // 缺失登记即为 UI 到 ScenePaint 桥接失败。
            .expect("modal should register a surface overlay");
        // 请求必须保留到 ScenePaint 聚合前的 typed 值。
        assert_eq!(
            // 读取登记值。
            entry.backdrop_blur_value(),
            // 显式半径不得丢失或退回默认关闭。
            Some(crate::ui::OverlayBackdropBlur::radius(24.0))
        );
        // 当前表面与半径必须能解析为 draw System 效果计划。
        assert!(
            app.tree()
                // 使用生产 OverlayStack。
                .overlay_stack()
                // 主题值不应覆盖显式半径。
                .backdrop_effect(8.0)
                // 合法请求必须形成计划。
                .is_some()
        );
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
        let (cancel, ok) = modal.footer_action_rects(modal.last_dialog_rect.get());
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
