// 声明本文件只编译反馈使用文档，不启动窗口或原生通知。
#![allow(dead_code)]

// 隔离 feedback-modal 围栏中的确认与声明式对话框。
mod feedback_modal {
    // 引入文档承诺的反馈组件公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "feedback-modal";

    // 编译确认对话框与自定义内容对话框的公开构建器。
    fn compile_example() {
        // 构造带确认和取消回调的对话框节点。
        let _confirm = embed(
            // 使用公开快捷构造器声明确认对话框。
            Modal::confirm(
                // 声明对话框标题。
                "确认删除",
                // 声明对话框内容。
                "此操作不可恢复，确定继续？",
                // 提供无副作用的确认回调。
                || {},
                // 提供无副作用的取消回调。
                || {},
            )
            // 覆盖标题配置。
            .title("确认删除")
            // 构建公开 ViewNode。
            .build(),
        );

        // 构造由 ModalContext 关闭自身的声明式对话框。
        let _custom = embed(
            // 使用内容闭包取得当前对话框上下文。
            Modal::show(|ctx| {
                // 纵向组合内容与关闭按钮。
                column((
                    // 添加自定义内容文本。
                    label("自定义内容"),
                    // 添加只包含关闭按钮的操作行。
                    row((
                        // 把关闭动作绑定到当前 ModalContext。
                        button("关闭").on_click_fn(move || ctx.close()),
                    ))
                    // 声明操作行间距。
                    .gap(8.0),
                ))
                // 声明内容区间距。
                .gap(12.0)
            })
            // 声明自定义对话框标题。
            .title("设置")
            // 构建公开 ViewNode。
            .build(),
        );
    }
}

// 隔离 feedback-modal-shortcuts 围栏中的快捷对话框。
mod feedback_modal_shortcuts {
    // 引入文档承诺的反馈组件公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "feedback-modal-shortcuts";

    // 编译三种快捷状态与关闭策略配置。
    fn compile_example() {
        // 构建信息对话框。
        let _info = embed(Modal::info("提示", "操作完成").build());
        // 构建警告对话框。
        let _warning = embed(Modal::warning("警告", "磁盘空间不足").build());
        // 构建错误对话框。
        let _error = embed(Modal::error("错误", "保存失败").build());

        // 构建关闭槽禁用且离场销毁子树的确认对话框。
        let _policy = embed(
            // 声明确认对话框及两个窄回调。
            Modal::confirm("保存", "确定保存？", || {}, || {})
                // 禁用标题栏关闭槽。
                .closable(false)
                // 声明离场后销毁内容子树。
                .destroy_on_close(true)
                // 构建公开 ViewNode。
                .build(),
        );
    }
}

// 隔离 feedback-message 围栏中的消息、通知与气泡确认。
mod feedback_message {
    // 引入文档承诺的反馈组件公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "feedback-message";

    // 编译进程内反馈管理器与 Popconfirm 构建器。
    fn compile_example() {
        // 创建轻量消息管理器。
        let message = Message::new();
        // 发布成功消息并保留稳定标识。
        let _saved_id = message.success("已保存");
        // 发布警告消息并保留稳定标识。
        let _warning_id = message.warning("即将过期");
        // 发布错误消息并保留稳定标识。
        let _error_id = message.error("操作失败");

        // 创建通知管理器。
        let notify = Notification::new();
        // 发布带标题和描述的信息通知。
        let _notification_id = notify.info("更新完成", "v0.0.2 已就绪");

        // 构建气泡确认组件节点。
        let _popconfirm = embed(
            // 创建默认气泡确认组件。
            Popconfirm::new()
                // 声明确认标题。
                .title("确定删除？")
                // 声明确认按钮文本。
                .confirm_text("确认")
                // 声明取消按钮文本。
                .cancel_text("取消"),
        );
    }
}

// 隔离 feedback-toast-options 围栏中的通知条目配置。
mod feedback_toast_options {
    // 引入文档承诺的反馈模型公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "feedback-toast-options";

    // 编译通知快捷入口与完整条目提交入口。
    fn compile_example() {
        // 创建通知管理器。
        let notify = Notification::new();
        // 发布成功通知并保留稳定标识。
        let _success_id = notify.success("完成", "任务已结束");
        // 发布警告通知并保留稳定标识。
        let _warning_id = notify.warning("注意", "即将过期");

        // 提交包含完整产品选项的通知条目。
        let _item_id = notify.add(NotificationItem {
            // 声明信息语义级别。
            type_: StatusLevel::Info,
            // 声明通知标题。
            title: "更新".into(),
            // 声明通知描述。
            description: "v0.0.2 已就绪".into(),
            // 声明可见时长。
            duration_ms: 5_000,
            // 声明用户可手动关闭。
            closable: true,
        });
    }
}

// 隔离 feedback-popover-options 围栏中的浮层锚点配置。
mod feedback_popover_options {
    // 引入文档承诺的浮层组件公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "feedback-popover-options";

    // 编译受控 Popover 与单触发 View 的 Tooltip。
    fn compile_example() {
        // 创建由业务拥有的受控打开状态。
        let open = State::new(false);
        // 构建绑定稳定触发 View 的 Popover。
        let _popover = embed(
            // 创建带内容的 Popover。
            Popover::new("内容")
                // 声明浮层标题。
                .title("标题")
                // 绑定业务受控状态。
                .controlled_open(&open)
                // 提供真实触发 View。
                .trigger_view(button("触发"))
                // 显示指向锚点的箭头。
                .arrow(true)
                // 声明浮层背景色。
                .bg(Color::WHITE),
        );

        // 构建包裹唯一触发节点的 Tooltip 视图。
        let _tooltip = embed(ViewNode::new(
            // 创建不取得业务焦点的 Tooltip。
            Tooltip::new("提示")
                // 优先放置在触发节点上方。
                .placement(TooltipPlacement::Top)
                // 显示箭头。
                .arrow(true)
                // 声明气泡背景色。
                .bg(Color::WHITE),
            // 提供唯一触发按钮子节点。
            vec![button("悬停").into()],
        ));
    }
}

// 隔离 feedback-alert-options 围栏中的警告状态与操作槽。
mod feedback_alert_options {
    // 引入文档承诺的 Alert 公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "feedback-alert-options";

    // 编译 Alert 的状态构造、操作槽、关闭槽与通栏配置。
    fn compile_example() {
        // 构建成功状态 Alert。
        let _success = embed(Alert::success("保存成功"));
        // 构建带独立业务回调的警告 Alert。
        let _action = embed(Alert::warning("磁盘空间不足").action("清理", || {}));
        // 构建可关闭的警告 Alert。
        let _closable = embed(Alert::warning("磁盘空间不足").closable());
        // 构建通栏错误 Alert。
        let _banner = embed(Alert::error("保存失败").banner(true));
    }
}

// 隔离 feedback-spin-options 围栏中的加载延迟与尺寸。
mod feedback_spin_options {
    // 引入公开时间类型。
    use std::time::Duration;
    // 引入文档承诺的 Spin 公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "feedback-spin-options";

    // 编译显式尺寸与快捷大尺寸加载组件。
    fn compile_example() {
        // 构建延迟二百毫秒显示的大尺寸加载组件。
        let _delayed = embed(
            // 创建默认加载组件。
            Spin::new()
                // 声明显示延迟。
                .delay(Duration::from_millis(200))
                // 声明大尺寸档位。
                .size(SpinSize::Large),
        );
        // 构建使用大尺寸快捷入口的加载组件。
        let _large = embed(Spin::new().large());
    }
}

// 隔离 feedback-float-group 围栏中的浮动按钮与事件保留组合。
mod feedback_float_group {
    // 引入文档承诺的浮动按钮与视图公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "feedback-float-group";

    // 编译独立浮动按钮与保留子节点事件的按钮组。
    fn compile_example() {
        // 构建锚定窗口右下角的独立浮动按钮。
        let _button = embed(
            // 创建消息图标浮动按钮。
            FloatButton::new("message")
                // 声明胶囊扩展描述。
                .description("意见反馈")
                // 声明辅助提示。
                .tooltip("打开反馈")
                // 保存数量徽标语义。
                .badge(7)
                // 让圆点视觉优先于数字。
                .badge_dot(true)
                // 锚定窗口右下角。
                .placement(Placement::BottomRight)
                // 应用有限作者偏移。
                .position(-8.0, -8.0),
        );

        // 构建保留每个子节点点击处理器的浮动按钮组。
        let _group = embed(View::build(
            // 创建默认按钮组并提交完整子 ViewNode。
            FloatButtonGroup::new().button_views(vec![
                // 添加保留标准点击处理器的编辑按钮。
                ViewNode::leaf(FloatButton::new("edit")).on_click_fn(|| {}),
                // 添加保留标准点击处理器的分享按钮。
                ViewNode::leaf(FloatButton::new("share")).on_click_fn(|| {}),
            ]),
        ));
    }
}

// 隔离 feedback-focustrap 围栏中的焦点作用域。
mod feedback_focustrap {
    // 引入文档承诺的焦点与视图公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "feedback-focustrap";

    // 编译包裹两个可聚焦后代的稳定焦点作用域。
    fn compile_example() {
        // 构建由运行时解析后代焦点顺序的 FocusTrap。
        let _trap = embed(ViewNode::new(
            // 创建无业务状态的焦点陷阱组件。
            FocusTrap::new(),
            // 声明两个可聚焦按钮后代。
            vec![
                // 添加确定按钮。
                button("确定").into(),
                // 添加取消按钮。
                button("取消").into(),
            ],
        ));
    }
}

// 隔离 feedback-controlled-modal 围栏中的受控业务状态。
mod feedback_controlled_modal {
    // 引入文档承诺的反馈与状态公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "feedback-controlled-modal";

    // 编译业务 State 与 Modal 可见性回写的单向适配。
    fn compile_example() {
        // 创建由业务拥有的保存中状态。
        let saving = State::new(false);
        // 为静态回调克隆同一状态句柄。
        let saving_open = saving.clone();

        // 构建由业务状态驱动的受控 Modal。
        let _modal = embed(
            // 创建声明式 Modal 构建器。
            Modal::builder()
                // 绑定业务可见性状态。
                .open(&saving)
                // 声明保存中标题。
                .title("保存中")
                // 使用大尺寸 Spin 作为内容。
                .content(|| embed(Spin::new().large()))
                // 禁用标题栏关闭槽。
                .closable(false)
                // 把用户侧可见性变化回写同一状态。
                .on_open_change(move |open| saving_open.set(open))
                // 构建公开 ViewNode。
                .build(),
        );
    }
}

// 隔离 feedback-window-handle 围栏中的窗口反馈投递。
mod feedback_window_handle {
    // 引入文档承诺的 AppHandle 与反馈模型公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "feedback-window-handle";

    // 编译向明确窗口宿主投递消息与通知的公开接口。
    fn report_saved(app: &AppHandle) -> std::result::Result<(), Error> {
        // 向当前 AppHandle 对应的窗口提交轻量消息。
        app.push_message(MessageItem {
            // 声明成功语义级别。
            type_: StatusLevel::Success,
            // 声明消息内容。
            content: "已保存".to_string(),
            // 声明消息可见时长。
            duration_ms: 3_000,
            // 允许用户手动关闭消息。
            closable: true,
        })?;
        // 向同一窗口提交持久通知。
        app.push_notification(NotificationItem {
            // 声明信息语义级别。
            type_: StatusLevel::Info,
            // 声明通知标题。
            title: "后台任务完成".to_string(),
            // 声明通知描述。
            description: "同步完成".to_string(),
            // 声明通知可见时长。
            duration_ms: 4_500,
            // 允许用户手动关闭通知。
            closable: true,
        })?;
        // 表示两个窗口反馈投递均成功。
        Ok(())
    }
}

// 运行无原生副作用的标记测试，让 Cargo 显式执行本编译消费者。
#[test]
// 确认本批外部消费者覆盖反馈文档的全部十一个围栏。
fn feedback_rust_fences_compile_as_external_consumers() {
    // 收集十一个已经由编译器类型检查的公开示例标识。
    let compile_ids = [
        // 登记 Modal 基础围栏。
        feedback_modal::COMPILE_ID,
        // 登记 Modal 快捷围栏。
        feedback_modal_shortcuts::COMPILE_ID,
        // 登记 Message 与 Notification 围栏。
        feedback_message::COMPILE_ID,
        // 登记通知条目配置围栏。
        feedback_toast_options::COMPILE_ID,
        // 登记 Popover 与 Tooltip 围栏。
        feedback_popover_options::COMPILE_ID,
        // 登记 Alert 配置围栏。
        feedback_alert_options::COMPILE_ID,
        // 登记 Spin 配置围栏。
        feedback_spin_options::COMPILE_ID,
        // 登记浮动按钮组围栏。
        feedback_float_group::COMPILE_ID,
        // 登记 FocusTrap 围栏。
        feedback_focustrap::COMPILE_ID,
        // 登记受控 Modal 围栏。
        feedback_controlled_modal::COMPILE_ID,
        // 登记窗口反馈句柄围栏。
        feedback_window_handle::COMPILE_ID,
    ];
    // 运行阶段核对消费者覆盖标识与 Markdown 围栏一致。
    assert_eq!(
        // 使用实际模块暴露的标识作为结果。
        compile_ids,
        // 使用反馈文档当前声明的稳定标识作为期望。
        [
            // Modal 基础围栏标识。
            "feedback-modal",
            // Modal 快捷围栏标识。
            "feedback-modal-shortcuts",
            // Message 与 Notification 围栏标识。
            "feedback-message",
            // 通知条目配置围栏标识。
            "feedback-toast-options",
            // Popover 与 Tooltip 围栏标识。
            "feedback-popover-options",
            // Alert 配置围栏标识。
            "feedback-alert-options",
            // Spin 配置围栏标识。
            "feedback-spin-options",
            // 浮动按钮组围栏标识。
            "feedback-float-group",
            // FocusTrap 围栏标识。
            "feedback-focustrap",
            // 受控 Modal 围栏标识。
            "feedback-controlled-modal",
            // 窗口反馈句柄围栏标识。
            "feedback-window-handle",
        ],
    );
}

#[cfg(feature = "test-harness")]
#[test]
fn modal_common_size_reconciles_and_refollows_surface() {
    use uix::prelude::*;
    use uix::ui::test_harness::TestApp;

    let open = State::new(true);
    let expanded = State::new(false);
    let root_open = open.clone();
    let root_expanded = expanded.clone();
    let mut app = TestApp::new((760.0, 560.0), move || {
        let (width, height) = if root_expanded.get() {
            (520.0, 340.0)
        } else {
            (420.0, 260.0)
        };
        Modal::builder()
            .open(&root_open)
            .title("尺寸回归")
            .footer_visible(false)
            .content(|| label("正文").automation_id("modal.body"))
            .build()
            .width(width)
            .height(height)
            .automation_id("modal.root")
    });

    let initial = app.snapshot();
    let initial_body = initial.find("modal.body").expect("Modal 正文应存在").frame;
    assert!(initial_body.w > 0.0 && initial_body.h > 0.0);

    expanded.set(true);
    app.settle().expect("Modal 尺寸更新应完成协调");
    let expanded_body = app
        .snapshot()
        .find("modal.body")
        .expect("扩展后的 Modal 正文应存在")
        .frame;
    assert!((expanded_body.w - initial_body.w - 100.0).abs() < 0.01);
    assert!((expanded_body.h - initial_body.h - 80.0).abs() < 0.01);

    app.resize(360.0, 240.0).expect("缩小表面应完成重排");
    let compact_body = app
        .snapshot()
        .find("modal.body")
        .expect("缩小表面后的 Modal 正文应存在")
        .frame;
    assert!(compact_body.w >= 0.0 && compact_body.w < expanded_body.w);
    assert!(compact_body.h >= 0.0 && compact_body.h < expanded_body.h);

    app.resize(760.0, 560.0).expect("恢复表面应完成重排");
    let restored_body = app
        .snapshot()
        .find("modal.body")
        .expect("恢复表面后的 Modal 正文应存在")
        .frame;
    assert!((restored_body.w - expanded_body.w).abs() < 0.01);
    assert!((restored_body.h - expanded_body.h).abs() < 0.01);
}
