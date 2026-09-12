//! `ui/coordination/adapter/coordination.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;
use crate::ui::reactive::state::State;

// 组装一棵「根读 outside、scoped 子树读 inside」的声明树；
// 计数器分别记录根闭包与作用域闭包的执行次数。
fn make_root(
    outside: &State<u32>,
    inside: &State<u32>,
    root_runs: std::rc::Rc<std::cell::Cell<u32>>,
    scoped_runs: std::rc::Rc<std::cell::Cell<u32>>,
) -> ViewNode {
    let outside_state = outside.clone();
    let inside_state = inside.clone();
    let scoped_runs_for_closure = std::rc::Rc::clone(&scoped_runs);
    crate::ui::scoped(move || {
        // 根闭包体：读根依赖并内嵌一个作用域子树。
        let _ = outside_state.get();
        let inner = std::rc::Rc::clone(&scoped_runs_for_closure);
        let inside_for_scope = inside_state.clone();
        // 内层 scoped 用容器包住，形成真实父子节点而非覆盖外层工厂。
        crate::ui::column((crate::ui::scoped(move || {
            inner.set(inner.get() + 1);
            let value = inside_for_scope.get();
            ViewNode::leaf(crate::ui::widgets::Space::new().height(value as f32 + 1.0))
        }),))
    })
}

#[test]
fn scoped_state_change_rebuilds_only_the_subtree() {
    let outside = State::new(0u32);
    let inside = State::new(0u32);
    let root_runs = std::rc::Rc::new(std::cell::Cell::new(0u32));
    let scoped_runs = std::rc::Rc::new(std::cell::Cell::new(0u32));

    // 计根闭包执行：每次 make_root 调用记一次。
    let count_root = |runs: &std::rc::Rc<std::cell::Cell<u32>>| runs.set(runs.get() + 1);
    let build_tree =
        |outside: &State<u32>,
         inside: &State<u32>,
         root_runs: &std::rc::Rc<std::cell::Cell<u32>>,
         scoped_runs: &std::rc::Rc<std::cell::Cell<u32>>| {
            count_root(root_runs);
            make_root(
                outside,
                inside,
                std::rc::Rc::clone(root_runs),
                std::rc::Rc::clone(scoped_runs),
            )
        };

    let mut tree = ViewAdapter::build(build_tree(&outside, &inside, &root_runs, &scoped_runs));
    assert_eq!(root_runs.get(), 1);
    assert_eq!(scoped_runs.get(), 1, "挂载时作用域闭包执行一次");

    // 作用域内 State 变化：不请求整树协调，只入作用域重建队列。
    inside.set(1);
    assert!(
        !tree.has_reconcile_requested(),
        "作用域内 State 不得升级为整树 reconcile 请求"
    );
    assert!(tree.has_scoped_rebuild_requested());
    let requests = tree.take_scoped_rebuild_requests();
    assert_eq!(requests.len(), 1, "同帧一次失效合并为一个节点请求");
    for id in requests {
        ViewAdapter::reconcile_scoped_node(&mut tree, id);
    }
    assert_eq!(scoped_runs.get(), 2, "作用域闭包重跑");
    assert_eq!(root_runs.get(), 1, "根闭包保持未重跑");
    assert!(!tree.has_scoped_rebuild_requested(), "消费后请求位归零");

    // 根外 State 变化：整树协调照常重跑根与作用域闭包。
    outside.set(1);
    assert!(tree.take_reconcile_requested(), "根依赖仍走整树协调");
    ViewAdapter::reconcile(
        &mut tree,
        build_tree(&outside, &inside, &root_runs, &scoped_runs),
    );
    assert_eq!(root_runs.get(), 2);
    assert_eq!(scoped_runs.get(), 3, "根协调作为外层重跑作用域闭包");

    // 协调后作用域租约重装：再次变更仍走作用域通道。
    inside.set(2);
    assert!(!tree.has_reconcile_requested());
    let requests = tree.take_scoped_rebuild_requests();
    for id in requests {
        ViewAdapter::reconcile_scoped_node(&mut tree, id);
    }
    assert_eq!(scoped_runs.get(), 4);
    assert_eq!(root_runs.get(), 2);
}

use crate::ui::adapter::ViewAdapter;
