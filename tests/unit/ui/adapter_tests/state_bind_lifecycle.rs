// 导入父测试模块可访问的声明树适配器。
use super::ViewAdapter;
// 导入最小声明节点及常用组件。
use crate::ui::view::ViewNode;
// 导入动态子节点的最小组件。
use crate::ui::widgets::{Container, Label};
// 导入结构依赖与 Effect 的状态类型。
use crate::ui::{Effect, State};

// 验证真实移除节点仅释放节点租约，不影响根对共享 State 的绑定。
#[test]
// 执行共享 State 的根节点租约生命周期回归。
fn capture_runtime_removing_dynamic_node_releases_only_its_reconcile_lease() {
    // 创建由根与动态节点共同读取的结构性状态。
    let shared = State::new(0_i32);
    // 创建仅由动态节点读取的结构性状态。
    let child_only = State::new(0_i32);
    // 克隆状态供根捕获读取。
    let root_state = shared.clone();
    // 捕获读取共享状态的声明根。
    let root = ViewAdapter::capture_root(|| {
        // 登记根对共享状态的结构依赖。
        let _ = root_state.get();
        // 返回可承载动态子节点的根。
        ViewNode::leaf(Container::new())
    });
    // 建立拥有根租约的运行时树。
    let mut tree = ViewAdapter::build_nodes(root);
    // 读取动态协调所需的实际根标识。
    let root_id = tree.root_id().expect("测试树必须拥有根节点");
    // 克隆共享状态供动态节点捕获读取。
    let child_state = shared.clone();
    // 克隆节点专属状态供动态节点捕获读取。
    let child_only_state = child_only.clone();
    // 捕获读取同一状态的动态节点。
    let child = ViewAdapter::capture_root(|| {
        // 登记节点对共享状态的结构依赖。
        let _ = child_state.get();
        // 登记仅属于节点的结构依赖。
        let _ = child_only_state.get();
        // 返回最小动态子节点。
        ViewNode::leaf(Label::new("child"))
    });
    // 挂载动态节点并让其实际节点接管一份租约。
    ViewAdapter::reconcile_dynamic_children(&mut tree, root_id, vec![child]);
    // 清除挂载路径已有的请求以隔离后续断言。
    let _ = tree.take_reconcile_requested();
    // 修改共享状态应由根和节点的任一租约请求协调。
    shared.set(1);
    // 已挂载节点必须参与结构协调。
    assert!(tree.take_reconcile_requested());
    // 修改仅由动态节点读取的状态。
    child_only.set(1);
    // 节点专属租约在挂载期间必须请求协调。
    assert!(tree.take_reconcile_requested());
    // 真实移除动态节点，使其 BoxedWidget 析构并释放节点租约。
    ViewAdapter::reconcile_dynamic_children(&mut tree, root_id, Vec::new());
    // 清除移除协调本身留下的请求。
    let _ = tree.take_reconcile_requested();
    // 再次修改共享状态。
    shared.set(2);
    // 根仍持有共享状态租约，因此移除节点后请求必须保留。
    assert!(tree.take_reconcile_requested());
    // 再次清除根共享状态引发的请求以隔离节点专属断言。
    let _ = tree.take_reconcile_requested();
    // 修改已真实移除节点专属的状态。
    child_only.set(2);
    // 节点租约必须已解绑，旧状态不得再请求协调。
    assert!(!tree.take_reconcile_requested());
}

// 验证窗口关闭同时释放根、节点 State 租约和两类 Effect 所有权。
#[test]
// 执行关闭后的结构绑定与 Effect 资源释放回归。
fn capture_runtime_shutdown_releases_root_and_node_state_binds_and_effects() {
    // 创建根 Effect 与根结构依赖共同读取的状态。
    let root_state = State::new(0_i32);
    // 创建节点 Effect 与节点结构依赖共同读取的状态。
    let node_state = State::new(0_i32);
    // 创建仅应由根 Effect 持有的资源。
    let root_resource = std::sync::Arc::new(());
    // 记录根资源是否已被树释放。
    let root_weak = std::sync::Arc::downgrade(&root_resource);
    // 克隆根状态供捕获闭包使用。
    let captured_root_state = root_state.clone();
    // 捕获根资源并将唯一强引用转交给根 Effect。
    let root = ViewAdapter::capture_root(move || {
        // 登记根结构依赖。
        let _ = captured_root_state.get();
        // 创建持有根资源和依赖状态的根 Effect。
        let _ = Effect::new(move || {
            // 保持资源仅由 Effect 生命周期拥有。
            let _ = std::sync::Arc::strong_count(&root_resource);
            // 建立 Effect 的状态依赖。
            let _ = captured_root_state.get();
        });
        // 返回根声明节点。
        ViewNode::leaf(Container::new())
    });
    // 建立包含根 State 租约与 Effect 的树。
    let mut tree = ViewAdapter::build_nodes(root);
    // 读取动态协调所需的实际根标识。
    let root_id = tree.root_id().expect("测试树必须拥有根节点");
    // 创建仅应由节点 Effect 持有的资源。
    let node_resource = std::sync::Arc::new(());
    // 记录节点资源是否已被树释放。
    let node_weak = std::sync::Arc::downgrade(&node_resource);
    // 克隆节点状态供捕获闭包使用。
    let captured_node_state = node_state.clone();
    // 捕获带有节点 State 租约与 Effect 的动态节点。
    let child = ViewAdapter::capture_root(move || {
        // 登记节点结构依赖。
        let _ = captured_node_state.get();
        // 创建持有节点资源和依赖状态的节点 Effect。
        let _ = Effect::new(move || {
            // 保持资源仅由节点 Effect 生命周期拥有。
            let _ = std::sync::Arc::strong_count(&node_resource);
            // 建立节点 Effect 的状态依赖。
            let _ = captured_node_state.get();
        });
        // 返回动态子节点。
        ViewNode::leaf(Label::new("child"))
    });
    // 挂载动态节点以交给 BoxedWidget 生命周期管理。
    ViewAdapter::reconcile_dynamic_children(&mut tree, root_id, vec![child]);
    // 根和节点资源在关闭前必须仍被各自 Effect 持有。
    assert!(root_weak.upgrade().is_some());
    // 节点资源在关闭前必须仍被节点 Effect 持有。
    assert!(node_weak.upgrade().is_some());
    // 关闭窗口树以释放所有根和节点生命周期资源。
    tree.shutdown();
    // 根 Effect 被清空后不得再持有根资源。
    assert!(root_weak.upgrade().is_none());
    // 节点 Effect 被清空后不得再持有节点资源。
    assert!(node_weak.upgrade().is_none());
    // 关闭后根和节点 Effect 均不得再报告待执行工作。
    assert!(!tree.has_pending_effects());
    // 关闭后 Effect 调度不得执行任何工作。
    assert!(!tree.tick_effects());
    // 清除关闭前可能已有的协调请求。
    let _ = tree.take_reconcile_requested();
    // 修改关闭后根状态。
    root_state.set(1);
    // 修改关闭后节点状态。
    node_state.set(1);
    // 根和节点 State 租约都必须已经解除。
    assert!(!tree.take_reconcile_requested());
}
