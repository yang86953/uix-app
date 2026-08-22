    // 引入当前模块私有的应用根准备函数。
    use super::*;
    // 引入最小文本组件构造测试根节点。
    use crate::ui::widgets::Label;
    // 引入反馈状态等级以构造最小消息条目。
    #[cfg(feature = "feedback")]
    use crate::platform::capabilities::StatusLevel;
    // 引入声明组件挂载与卸载生命周期入口。
    #[cfg(feature = "feedback")]
    use crate::ui::widget_runtime::traits::WidgetLifecycle;

    // 验证透明根节点获得主题布局背景。
    #[test]
    fn app_root_uses_layout_background_when_unspecified() {
        // 构造没有显式背景的最小应用根节点。
        let root = ViewNode::leaf(Label::new("root"));
        // 在不安装通知浮层的路径准备应用根节点。
        let prepared = prepare_app_root(root, None, WindowId::ROOT);
        // 默认值必须保持为可随主题解析的布局背景语义令牌。
        assert_eq!(
            prepared.style.background,
            Some(ColorValue::Neutral(NeutralRole::BgLayout))
        );
    }

    // 验证用户显式背景高于应用根默认值。
    #[test]
    fn app_root_preserves_explicit_background() {
        // 选择与默认布局背景不同的显式容器背景。
        let explicit = ColorValue::Neutral(NeutralRole::BgContainer);
        // 构造已经声明显式背景的最小应用根节点。
        let root = ViewNode::leaf(Label::new("root")).bg(explicit);
        // 在不安装通知浮层的路径准备应用根节点。
        let prepared = prepare_app_root(root, None, WindowId::ROOT);
        // 应用默认处理不得覆盖调用方声明的背景。
        assert_eq!(prepared.style.background, Some(explicit));
    }

    // 验证反馈浮层不会遮断内部应用根背景默认处理。
    #[cfg(feature = "feedback")]
    #[test]
    fn app_root_installs_window_feedback_hosts_after_business_root() {
        // 构造没有显式背景的最小应用根节点。
        let root = ViewNode::leaf(Label::new("root"));
        // 安装反馈状态并准备带浮层的应用根节点。
        let prepared = prepare_app_root(root, Some(AppFeedbackState::new()), WindowId::ROOT);
        // 应用根必须依次包含业务根、Message Host 与 Notification Host。
        assert_eq!(prepared.children.len(), 3);
        // 浮层下的业务根必须获得主题布局背景。
        assert_eq!(
            prepared.children[0].style.background,
            Some(ColorValue::Neutral(NeutralRole::BgLayout))
        );
        // 第二个子节点必须绑定 Message Host。
        assert!(prepared.children[1].widget.as_any().is::<Message>());
        // 第三个子节点必须绑定 Notification Host。
        assert!(prepared.children[2].widget.as_any().is::<Notification>());
    }

    // 验证反馈浮层根提供稳定快照，页面协调不得把无配置根误判为整窗变化。
    #[cfg(feature = "feedback")]
    #[test]
    fn app_overlay_root_snapshot_is_stable() {
        // 两轮根准备应生成配置等价的零状态浮层根。
        let first = prepare_app_root(
            ViewNode::leaf(Label::new("first")),
            Some(AppFeedbackState::new()),
            WindowId::ROOT,
        );
        let second = prepare_app_root(
            ViewNode::leaf(Label::new("second")),
            Some(AppFeedbackState::new()),
            WindowId::ROOT,
        );
        // 业务子树内容不同也不能污染外层浮层根的配置快照。
        assert_eq!(
            first.widget.snapshot_fields(),
            second.widget.snapshot_fields()
        );
        // 零状态根必须脱离 Unknown 的保守整窗失效路径。
        assert_ne!(
            first.widget.snapshot_fields(),
            crate::ui::SnapshotFields::Unknown
        );
    }

    // 验证不同 WindowId 不会共享反馈队列。
    #[cfg(feature = "feedback")]
    #[test]
    fn feedback_state_isolates_message_queues_by_window() {
        // 创建 Application System 唯一反馈 owner。
        let feedback = AppFeedbackState::new();
        // 构造可观察的最小消息条目。
        let item = MessageItem {
            // 使用信息等级避免引入错误语义。
            type_: StatusLevel::Info,
            // 内容只用于区分测试输入。
            content: "root".to_string(),
            // 零时长避免自动关闭影响队列断言。
            duration_ms: 0,
            // 本测试不需要关闭按钮。
            closable: false,
        };
        // 只向根窗口写入一个条目。
        feedback.push_message(WindowId::ROOT, item);
        // 根窗口队列必须保存该条目。
        assert_eq!(feedback.handles(WindowId::ROOT).message.len(), 1);
        // 第二个窗口必须获得独立的空队列。
        assert_eq!(feedback.handles(WindowId::new(1)).message.len(), 0);
    }

    // 验证应用根准备阶段注入端口且声明生命周期只操作 Host 队列。
    #[cfg(feature = "feedback")]
    #[test]
    fn prepared_message_declaration_mounts_updates_and_releases_window_lease() {
        // 创建可观察的逐窗 owner。
        let feedback = AppFeedbackState::new();
        // 构造零布局 Message 声明业务根。
        let root = ViewNode::leaf(MessageDeclaration::new("saved", "初始"));
        // 应用根准备阶段递归注入根窗口端口。
        let mut prepared = prepare_app_root(root, Some(feedback.clone()), WindowId::ROOT);
        // 业务根仍位于覆盖 Host 之前。
        let declaration = prepared.children[0]
            .widget
            .as_any_mut()
            .downcast_mut::<MessageDeclaration>()
            .expect("业务根必须保留 MessageDeclaration 类型");
        // 模拟树执行首次挂载生命周期。
        WidgetLifecycle::on_mount(declaration);
        // 首次挂载只写入一个 Message Host 条目。
        assert_eq!(feedback.handles(WindowId::ROOT).message.len(), 1);
        // 同 key reconcile 更新内容但保留租约和稳定 ID。
        declaration.sync_from(MessageDeclaration::new("saved", "更新"));
        // 队列仍只有一个条目。
        assert_eq!(feedback.handles(WindowId::ROOT).message.len(), 1);
        // 最新内容必须进入同一 Host 队列。
        assert_eq!(
            feedback.handles(WindowId::ROOT).message.items()[0].content,
            "更新"
        );
        // 模拟树执行真正卸载生命周期。
        WidgetLifecycle::on_unmount(declaration);
        // 卸载释放条目且不保留不可达队列项。
        assert_eq!(feedback.handles(WindowId::ROOT).message.len(), 0);
    }

    // 验证关闭后的 AppHandle 返回类型化失败而不是伪 ID。
    #[cfg(feature = "feedback")]
    #[test]
    fn closed_app_handle_rejects_feedback_with_invalid_state() {
        // 容器仍安装反馈 owner，用于证明失败来自窗口生命周期。
        let mut container = Container::new();
        // 注册 Application System 唯一反馈状态。
        container.singleton(AppFeedbackState::new());
        // 构造已经关闭的窗口存活标记。
        let alive = Arc::new(AtomicBool::new(false));
        // 创建只用于聚焦契约测试的 AppHandle。
        let handle = AppHandle::new(
            // 绑定根窗口身份。
            WindowId::ROOT,
            // 使用独立应用组件状态。
            AppState::new(),
            // 使用独立运行时，不注册窗口会话。
            AppRuntime::new(),
            // 注入已安装反馈 owner 的容器。
            container,
            // 注入关闭状态。
            alive,
        );
        // 构造不会自动关闭的最小消息条目。
        let item = MessageItem {
            // 使用信息等级。
            type_: StatusLevel::Info,
            // 内容只用于触发公开入口。
            content: "closed".to_string(),
            // 零时长避免无关计时语义。
            duration_ms: 0,
            // 本测试不需要关闭按钮。
            closable: false,
        };
        // 关闭窗口必须返回 InvalidState，禁止 no-op 或返回零 ID。
        let error = handle.push_message(item).expect_err("关闭窗口必须拒绝反馈");
        // 错误码必须稳定表达生命周期违规。
        assert_eq!(error.code(), Errc::InvalidState);
    }
