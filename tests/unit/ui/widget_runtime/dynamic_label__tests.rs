    // 引入被测 DynamicLabel 私有实现。
    use super::DynamicLabel;
    // 引入精确组件身份与绘制矩形。
    use crate::core::{ComponentId, Rect};
    // 引入共享失效队列。
    use crate::draw::renderer::InvalidationQueue;
    // 引入动态闭包依赖探测入口。
    use crate::ui::reactive::state::StateBindCaptureGuard;
    // 引入实际组件树以验证移除与窗口关闭边界。
    use crate::ui::widget_runtime::widget::WidgetTree;
    // 引入公开响应式状态。
    use crate::ui::State;

    // 验证动态文本读取最新语义并只推送自身矩形的 Paint 失效。
    #[test]
    fn state_change_invalidates_only_dynamic_label_paint() {
        // 创建由测试调用方拥有的响应式值。
        let value = State::new(1_u32);
        // 克隆同一状态槽供动态文本闭包拥有。
        let source = value.clone();
        // 创建每次读取当前值的动态标签。
        let label = DynamicLabel::new(move || format!("tick: {}", source.get()));
        // 创建独立树失效队列。
        let queue = InvalidationQueue::shared();
        // 使用非根组件身份验证失效范围。
        let component_id = ComponentId::new(7);
        // 指定动态文本自身的已布局矩形。
        let paint_rect = Rect::new(12.0, 20.0, 80.0, 24.0);
        // 开始捕获闭包内 State::get 依赖并建立异常清理边界。
        let capture = StateBindCaptureGuard::begin(component_id, queue.clone(), Some(paint_rect));
        // 执行一次与布局后绑定相同的依赖探测。
        label.probe_dependencies();
        // 将捕获依赖交给模拟实际节点租约集合。
        let leases = capture.finish();
        // 初始无障碍语义必须读取当前文本。
        assert_eq!(label.semantic_text(), "tick: 1");
        // 更新 State 应触发已绑定动态文本节点。
        value.set(2);
        // 无障碍语义必须立即反映最新状态。
        assert_eq!(label.semantic_text(), "tick: 2");
        // 读取状态更新产生的失效证据。
        let queue = queue.lock().expect("动态文本失效队列应可读取");
        // 动态文本值变化不得请求结构布局。
        assert!(!queue.has_layout());
        // 只有绑定的动态文本组件需要绘制。
        assert!(queue.node_needs_paint(component_id));
        // 已知组件矩形不得退化为全帧绘制。
        assert!(!queue.needs_full_frame());
        // 释放模拟实际节点拥有的全部绘制订阅。
        drop(leases);
        // 节点离开后 State 不得继续保留其绘制端点。
        assert_eq!(value.paint_site_count(), 0);
    }

    // 验证节点真实移除和窗口关闭都会释放 DynamicLabel 绘制订阅。
    #[test]
    fn tree_removal_and_shutdown_release_dynamic_label_paint_sites() {
        // 创建跨两棵临时窗口树存活的共享状态。
        let value = State::new(1_u32);

        // 克隆共享槽供第一棵树的动态文本闭包拥有。
        let removal_source = value.clone();
        // 创建模拟第一窗口的组件树。
        let mut removal_tree = WidgetTree::new();
        // 安装读取共享状态的动态标签并保存实际组件身份。
        let removal_id = removal_tree.set_root(Box::new(DynamicLabel::new(move || {
            // 每次探测或绘制都读取当前共享值。
            format!("remove: {}", removal_source.get())
        })));
        // 完成布局以触发框架级动态依赖探测。
        removal_tree.layout();
        // 第一棵树只能持有一个合并后的绘制端点。
        assert_eq!(value.paint_site_count(), 1);
        // 真实移除动态标签节点及其所有权资源。
        removal_tree.remove(removal_id);
        // 节点离开后共享 State 不得保留旧树端点。
        assert_eq!(value.paint_site_count(), 0);

        // 克隆共享槽供第二棵树的动态文本闭包拥有。
        let shutdown_source = value.clone();
        // 创建模拟第二窗口的组件树。
        let mut shutdown_tree = WidgetTree::new();
        // 安装读取同一共享槽的窗口根动态标签。
        shutdown_tree.set_root(Box::new(DynamicLabel::new(move || {
            // 每次探测或绘制都读取当前共享值。
            format!("shutdown: {}", shutdown_source.get())
        })));
        // 完成布局并建立由实际根节点持有的绘制租约。
        shutdown_tree.layout();
        // 第二棵树重新形成自己的唯一绘制端点。
        assert_eq!(value.paint_site_count(), 1);
        // 模拟所属窗口在保留树对象期间执行受控关闭。
        shutdown_tree.shutdown();
        // shutdown 必须主动清理仍在物理槽位中的节点租约。
        assert_eq!(value.paint_site_count(), 0);
    }
