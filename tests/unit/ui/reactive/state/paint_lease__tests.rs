// 引入当前模块的租约和捕获守卫。
use super::{PaintBindLease, StateBindCaptureGuard};
// 引入精确组件身份与绘制范围。
use crate::core::{Rect, WidgetId};
// 引入共享失效队列。
use crate::draw::renderer::InvalidationQueue;
// 引入通用绑定接口与响应式值。
use crate::ui::reactive::state::{Computed, State, StatePaintBind};
// 引入共享所有权容器。
use std::sync::Arc;

// 验证最后一份 State 绘制租约释放站点与窗口队列。
#[test]
fn state_paint_lease_releases_endpoint_and_queue() {
    // 创建由测试调用方长期拥有的响应式状态。
    let state = State::new(1_u32);
    // 创建模拟窗口拥有的失效队列。
    let queue = InvalidationQueue::shared();
    // 保存弱引用以观察状态源是否继续强持有旧窗口。
    let weak_queue = Arc::downgrade(&queue);
    // 将 State 句柄提升为通用绘制绑定源。
    let source: Arc<dyn StatePaintBind> = Arc::new(state.clone());
    // 建立由模拟实际节点持有的唯一租约。
    let lease = PaintBindLease::bind(
        // 交接响应式源。
        source,
        // 使用非根组件身份。
        WidgetId::new(7),
        // 共享模拟窗口队列。
        queue.clone(),
        // 提供精确绘制范围。
        Some(Rect::new(1.0, 2.0, 30.0, 12.0)),
    );
    // State 只能保存一个合并后的组件端点。
    assert_eq!(state.paint_site_count(), 1);
    // 模拟窗口所有者先释放自身队列句柄。
    drop(queue);
    // 活跃节点租约仍应维持当前窗口端点。
    assert!(weak_queue.upgrade().is_some());
    // 模拟节点真实移除并析构租约。
    drop(lease);
    // 最后一份租约离开后 State 不再保留旧端点。
    assert_eq!(state.paint_site_count(), 0);
    // 状态源不得继续延长已关闭窗口队列生命周期。
    assert!(weak_queue.upgrade().is_none());
}

// 验证 Computed 捕获租约和 panic 清理都遵守作用域边界。
#[test]
fn computed_capture_lease_releases_and_panic_restores_stack() {
    // 创建派生值读取的上游状态。
    let state = State::new(2_u32);
    // 克隆上游句柄供派生闭包持有。
    let upstream = state.clone();
    // 创建可在绘制期间被自动捕获的派生值。
    let computed = Computed::new(move || upstream.get() * 3);
    // 创建模拟窗口失效队列。
    let queue = InvalidationQueue::shared();
    // 开始一个会正常完成的 Computed 绘制捕获。
    let capture = StateBindCaptureGuard::begin(
        // 使用独立组件身份。
        WidgetId::new(9),
        // 共享模拟窗口队列。
        queue.clone(),
        // 提供组件绘制范围。
        Some(Rect::new(3.0, 4.0, 20.0, 10.0)),
    );
    // 读取派生值并登记当前最内层捕获。
    assert_eq!(computed.get(), 6);
    // 正常完成后由调用节点取得租约。
    let leases = capture.finish();
    // 派生源只能保存一个当前端点。
    assert_eq!(computed.paint_site_count(), 1);
    // 节点释放租约后必须清空派生端点。
    drop(leases);
    // Computed 不再保留已离开的节点。
    assert_eq!(computed.paint_site_count(), 0);

    // 捕获一次组件绘制 panic，验证守卫自动弹栈。
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 建立只属于异常绘制的捕获作用域。
        let _capture = StateBindCaptureGuard::begin(
            // 使用另一组件身份区分前一轮。
            WidgetId::new(10),
            // 共享同一模拟窗口队列。
            queue.clone(),
            // 异常轮仍提供精确范围。
            Some(Rect::new(5.0, 6.0, 18.0, 9.0)),
        );
        // 在 panic 前登记派生依赖。
        let _ = computed.get();
        // 模拟组件用户绘制逻辑异常展开。
        panic!("模拟组件绘制失败");
    }));
    // 测试必须确实经过 panic 清理路径。
    assert!(unwind.is_err());
    // 未正常完成的捕获不得建立任何绘制站点。
    assert_eq!(computed.paint_site_count(), 0);

    // 再次开始正常捕获，证明异常帧没有污染线程栈。
    let capture = StateBindCaptureGuard::begin(
        // 使用新的组件身份。
        WidgetId::new(11),
        // 继续共享模拟窗口队列。
        queue,
        // 允许未知绘制范围退化为组件级失效。
        None,
    );
    // 正常读取仍应登记到新捕获帧。
    let _ = computed.get();
    // 完成捕获并取得新租约。
    let leases = capture.finish();
    // 新一轮只应建立自己的单一端点。
    assert_eq!(computed.paint_site_count(), 1);
    // 测试结束前释放最后一份租约。
    drop(leases);
    // 所有临时节点离开后端点集合必须为空。
    assert_eq!(computed.paint_site_count(), 0);
}

// 验证嵌套捕获只把读取交给当前最内层组件所有者。
#[test]
fn nested_capture_restores_outer_owner_without_cross_binding() {
    // 创建只应归外层组件订阅的状态。
    let outer_state = State::new(1_u32);
    // 创建只应归内层组件订阅的状态。
    let inner_state = State::new(2_u32);
    // 创建两个模拟窗口端点以同时验证队列身份隔离。
    let outer_queue = InvalidationQueue::shared();
    // 内层使用不同队列，防止组件身份偶然掩盖串绑。
    let inner_queue = InvalidationQueue::shared();

    // 开始外层组件捕获。
    let outer_capture = StateBindCaptureGuard::begin(
        // 使用外层组件身份。
        WidgetId::new(20),
        // 使用外层窗口队列。
        outer_queue,
        // 外层允许组件级失效。
        None,
    );
    // 首次读取必须登记给外层捕获。
    let _ = outer_state.get();

    // 在外层仍活动时压入内层组件捕获。
    let inner_capture = StateBindCaptureGuard::begin(
        // 使用内层组件身份。
        WidgetId::new(21),
        // 使用独立内层窗口队列。
        inner_queue,
        // 内层同样允许组件级失效。
        None,
    );
    // 内层读取不得泄漏到外层捕获。
    let _ = inner_state.get();
    // 完成内层并恢复仍活动的外层捕获。
    let inner_leases = inner_capture.finish();
    // 内层源形成自己的唯一端点。
    assert_eq!(inner_state.paint_site_count(), 1);

    // 内层离开后再次读取外层源，证明栈顶已恢复。
    let _ = outer_state.get();
    // 完成外层并接管它两次读取对应的计数租约。
    let outer_leases = outer_capture.finish();
    // 重复读取仍合并为单一外层端点。
    assert_eq!(outer_state.paint_site_count(), 1);

    // 先释放内层节点全部租约。
    drop(inner_leases);
    // 内层源必须归零，证明没有被外层错误捕获。
    assert_eq!(inner_state.paint_site_count(), 0);
    // 外层节点仍拥有自己的独立站点。
    assert_eq!(outer_state.paint_site_count(), 1);
    // 再释放外层节点全部租约。
    drop(outer_leases);
    // 所有嵌套所有者离开后不得残留端点。
    assert_eq!(outer_state.paint_site_count(), 0);
}
