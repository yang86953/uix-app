    // 引入待验证的公开警告提示组件。
    use super::Alert;

    // 验证声明式刷新不会重新打开用户已经关闭的 Alert。
    #[test]
    // 声明关闭状态所有权测试。
    fn refresh_preserves_runtime_closed_state() {
        // 首次物化一条可关闭警告。
        let mut current = Alert::warning("旧警告").closable();
        // 模拟用户通过运行时关闭入口完成关闭。
        current.close();
        // 后续声明更新消息与类型，但仍只声明关闭能力。
        let next = Alert::error("新错误").closable();
        // 执行真实组件同步路径。
        current.sync_from(next);
        // 用户关闭事实必须继续由运行时状态拥有。
        assert!(!current.is_visible());
    }
