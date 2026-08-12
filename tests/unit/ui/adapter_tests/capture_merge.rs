// 导入父测试模块可访问的声明树适配器。
use super::ViewAdapter;
// 导入用于记录结构依赖的响应式状态。
use crate::ui::State;
// 导入最小声明根节点。
use crate::ui::view::ViewNode;
// 导入用于构建最小叶节点的标签组件。
use crate::ui::widgets::Label;

// 验证外层捕获直接返回内层节点时不会覆盖已携带的输出。
#[test]
// 执行嵌套捕获输出合并回归。
fn capture_runtime_root_merges_returned_inner_node_outputs() {
    // 创建外层进入内层前读取的状态。
    let outer_before = State::new(1_i32);
    // 创建内层节点读取的状态。
    let inner_state = State::new(2_i32);
    // 创建外层返回内层节点前继续读取的状态。
    let outer_after = State::new(3_i32);
    // 在外层捕获中直接返回已完成内层捕获的节点。
    let node = ViewAdapter::capture_root(|| {
        // 记录外层前置 State 绑定。
        let _ = outer_before.get();
        // 记录外层前置 Effect。
        let _ = crate::ui::Effect::new(|| {});
        // 构建携带自身输出的内层节点。
        let inner = ViewAdapter::capture_root(|| {
            // 记录内层 State 绑定。
            let _ = inner_state.get();
            // 记录内层 Effect。
            let _ = crate::ui::Effect::new(|| {});
            // 返回内层最小声明节点。
            ViewNode::leaf(Label::new("inner"))
        });
        // 记录外层后置 State 绑定。
        let _ = outer_after.get();
        // 记录外层后置 Effect。
        let _ = crate::ui::Effect::new(|| {});
        // 直接返回已携带内层输出的节点。
        inner
    });
    // 返回节点必须同时保留内层和外层共三个 State 绑定。
    assert_eq!(node.captured_state_binds.len(), 3);
    // 返回节点必须同时保留内层和外层共三个 Effect。
    assert_eq!(node.captured_effects.len(), 3);
}
