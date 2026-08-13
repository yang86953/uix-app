// 导入同级测试可访问的声明适配器。
use super::ViewAdapter;
// 把组件胶水宏放入当前作用域，支持其递归内部规则展开。
use crate::impl_widget_component;
// 导入声明树回执转移助手以构造嵌套事务关闭检查点。
use super::super::capture_guards::take_component_state_receipts;
// 导入场景桥的只读根契约以验证半树不会进入绘制遍历。
use crate::draw::scene::ScenePaint;
// 导入事件返回值与系统事件以验证 fail-stop 准入门。
use crate::ui::{EventResult, SystemEvent};
// 导入构造稳定宿主与普通声明节点所需的组件。
use crate::ui::widgets::{Container, Label};
// 导入测试组件能力、生命周期与响应式 Effect 所有权。
use crate::ui::{
    // Effect 用于验证 fail-stop teardown 的资源释放。
    Effect,
    // 快照字段保持手写测试组件的协调接口完整。
    SnapshotFields,
    // 能力集合供仅在 build 阶段 panic 的组件使用。
    WidgetCapabilities,
    // 核心组件 trait 支持手写发布异常组件。
    WidgetComponent,
    // 生命周期 trait 支持在 shutdown destroy 阶段注入异常。
    WidgetLifecycle,
};
// 导入声明节点以构造动态失败批次。
use crate::ui::view::ViewNode;
// 导入类型擦除接口以实现最小测试组件。
use std::any::Any;
// 导入共享资源以观察 poisoned shutdown 的确定释放。
use std::sync::Arc;

// 定义在真实节点 build 阶段触发指定 panic 的测试组件。
struct PanicOnBuild;

// 为发布阶段异常提供最小组件实现。
impl WidgetComponent for PanicOnBuild {
    // 暴露只读类型擦除引用。
    fn as_any(&self) -> &dyn Any {
        // 返回当前测试组件。
        self
    }

    // 暴露可变类型擦除引用。
    fn as_any_mut(&mut self) -> &mut dyn Any {
        // 返回当前测试组件。
        self
    }

    // 把装箱测试组件转交给适配器。
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        // 返回当前测试组件所有权。
        self
    }

    // 声明测试组件不需要布局、绘制或事件能力。
    fn capabilities(&self) -> WidgetCapabilities {
        // 返回空能力集合。
        WidgetCapabilities::new()
    }

    // 在运行时树已经开始发布新子节点时触发异常。
    fn build(&self) -> Vec<Box<dyn WidgetComponent>> {
        // 使用稳定消息验证事务恢复后保留原始 panic payload。
        panic!("发布阶段测试异常")
    }

    // 提供保守快照以保持组件协调接口完整。
    fn snapshot_fields(&self) -> SnapshotFields {
        // 测试组件没有可比较运行态字段。
        SnapshotFields::Unknown
    }
}

// 定义在 shutdown destroy 阶段 panic 且持有可观察资源的组件。
struct PanicOnDestroy {
    // 资源只由节点组件持有，用于验证异常后仍完成释放。
    resource: Arc<()>,
}

// 生成仅暴露生命周期能力的测试组件胶水。
impl_widget_component!(PanicOnDestroy; Lifecycle);

// 为 shutdown 异常门禁提供最小生命周期实现。
impl WidgetLifecycle for PanicOnDestroy {
    // 在 destroy 阶段验证资源仍归当前节点所有后触发稳定异常。
    fn on_destroy(&mut self) {
        // 读取计数让资源捕获成为可观察的真实生命周期依赖。
        let _ = Arc::strong_count(&self.resource);
        // 使用稳定文本验证关闭边界恢复原始 payload。
        panic!("关闭生命周期测试异常")
    }
}

// 从 panic payload 中读取稳定测试消息。
fn panic_message(
    // 接收 catch_unwind 返回的类型擦除异常所有权。
    payload: Box<dyn Any + Send>,
    // 返回可用于精确行为断言的消息文本。
) -> String {
    // 优先处理直接 panic 的静态字符串。
    if let Some(message) = payload.downcast_ref::<&str>() {
        // 复制静态文本供调用方独立比较。
        return (*message).to_owned();
    }
    // 再处理格式化 panic 产生的拥有型字符串。
    if let Some(message) = payload.downcast_ref::<String>() {
        // 克隆消息而不消费未知 payload 类型。
        return message.clone();
    }
    // 未知 payload 违反测试注入点的稳定契约。
    panic!("测试 panic 必须携带文本消息")
}

// 验证尚未开始发布的事务 panic 会恢复旧树并允许后续协调。
#[test]
// 执行预检失败的可恢复事务门禁。
fn fail_stop_pre_publish_panic_preserves_operational_tree() {
    // 建立拥有稳定根身份的正常运行树。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Container::new()));
    // 记录失败前的根身份以验证旧结构保持不变。
    let old_root = tree.root_id().expect("预检异常测试必须拥有根节点");
    // 捕获尚未调用发布标记的事务异常。
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 空回执事务模拟已完成 capture、尚未接触运行时结构的预检阶段。
        tree.with_component_state_transaction(Vec::new(), |_tree| {
            // 在首个真实 mutator 之前触发稳定异常。
            panic!("预检阶段测试异常");
        });
    }));
    // 预检异常必须继续向调用方传播。
    let payload = result.expect_err("预检阶段必须传播原始 panic");
    // 恢复边界不得替换或吞掉原始 payload。
    assert_eq!(panic_message(payload), "预检阶段测试异常");
    // 未发布失败后旧树必须重新开放外部工作。
    assert!(tree.accepts_external_work());
    // 未发布失败不得改变旧根身份。
    assert_eq!(tree.root_id(), Some(old_root));
    // 恢复后的独立协调必须仍可成功执行。
    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(Label::new("recovered")));
    // 正常协调后树继续保持 Operational。
    assert!(tree.accepts_external_work());
}

// 验证公开直接换根同样受事务保护，发布中 panic 后不得重新开放半树。
#[test]
// 执行 set_root 用户 build 异常的公开 API 门禁。
fn fail_stop_direct_set_root_publish_panic_stops_tree() {
    // 建立带有可观察旧根身份的正常运行树。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Container::new()));
    // 保存换根前仍有效的身份以验证停止后不再暴露。
    let old_root = tree.root_id().expect("直接换根测试必须拥有旧根");
    // 捕获公开 set_root 在用户 build 阶段恢复的异常。
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 直接公开入口必须自行建立事务，调用方无需预先包装。
        tree.set_root(Box::new(PanicOnBuild));
    }));
    // 用户 build 异常必须继续传播到公开调用方。
    let payload = result.expect_err("公开换根必须传播用户 build panic");
    // 事务边界不得替换原始用户异常消息。
    assert_eq!(panic_message(payload), "发布阶段测试异常");
    // 旧树已开始拆除后必须永久拒绝后续外部工作。
    assert!(!tree.accepts_external_work());
    // fail-stop 公开根接口不得暴露半清理后的身份。
    assert!(tree.root_id().is_none());
    // 旧根节点访问必须在停止态返回空。
    assert!(tree.get(old_root).is_none());
    // 所有者仍可幂等关闭并释放内部半发布资源。
    tree.shutdown();
    // 关闭后继续保持停止终态。
    assert!(!tree.accepts_external_work());
}

// 验证真实结构发布后的 panic 会永久停止半树并由 shutdown 释放资源。
#[test]
// 执行发布失败、入口拒绝、资源释放与新 owner 重建门禁。
fn fail_stop_publish_panic_blocks_work_until_owner_teardown() {
    // 建立用于验证 Effect 所有权释放的共享资源。
    let effect_resource = Arc::new(());
    // 保存弱引用以避免测试自身延长资源生命周期。
    let weak_effect_resource = Arc::downgrade(&effect_resource);
    // 克隆资源供树拥有的 Effect 闭包持有。
    let captured_effect_resource = Arc::clone(&effect_resource);
    // 建立可接收动态子节点的稳定宿主树。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Container::new()));
    // 让根 Effect 成为 WidgetTree 受控关闭必须释放的框架资源。
    tree.register_root_effects(vec![Effect::new(move || {
        // 读取强引用计数以确保闭包真实捕获资源。
        let _ = Arc::strong_count(&captured_effect_resource);
    })]);
    // 丢弃测试侧强引用，让弱引用只观察树拥有资源。
    drop(effect_resource);
    // 关闭前 Effect 必须仍持有该资源。
    assert!(weak_effect_resource.upgrade().is_some());
    // 读取动态子树共同所属的真实宿主身份。
    let owner = tree.root_id().expect("发布异常测试必须拥有宿主根节点");
    // 构造先成功挂载的第一个兄弟节点。
    let stable_child = ViewNode::leaf(Label::new("published")).key("stable");
    // 构造随后在 build 阶段触发异常的第二个兄弟节点。
    let panicking_child = ViewNode::leaf(PanicOnBuild).key("panic");
    // 捕获发布中断时继续向上传播的异常。
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 同一事务先改写真实结构，再由第二个兄弟节点触发 panic。
        ViewAdapter::reconcile_dynamic_children(
            // 协调当前宿主树。
            &mut tree,
            // 把两个声明节点交给同一运行时父节点。
            owner,
            // 保持稳定节点位于失败节点之前以形成半提交风险。
            vec![stable_child, panicking_child],
        );
    }));
    // 发布阶段异常必须继续向调用方传播。
    let payload = result.expect_err("发布阶段必须传播原始 panic");
    // fail-stop 边界不得用内部诊断替换用户 payload。
    assert_eq!(panic_message(payload), "发布阶段测试异常");
    // 真实结构已开始发布后树必须永久拒绝外部工作。
    assert!(!tree.accepts_external_work());
    // 公开根身份不得暴露半提交结构。
    assert!(tree.root_id().is_none());
    // 公开只读节点访问不得取得可继续调用组件方法的引用。
    assert!(tree.get(owner).is_none());
    // 公开可变节点访问同样必须拒绝半树。
    assert!(tree.get_mut(owner).is_none());
    // 公开遍历不得泄漏任何半提交节点身份。
    assert!(tree.traverse().is_empty());
    // fail-stop 树不得继续暴露或执行 Effect。
    assert!(!tree.has_pending_effects());
    // 显式 tick 同样必须保持中性结果。
    assert!(!tree.tick_effects());
    // 系统事件不得进入半树 handler 或组件回调。
    assert_eq!(
        // 使用主题事件覆盖无需布局坐标的公共分发入口。
        tree.dispatch_event(&SystemEvent::ThemeChanged { is_dark: false }),
        // 非运行态树明确返回未处理。
        EventResult::NotHandled,
    );
    // 场景桥必须把 poisoned 树暴露为空场景。
    assert!(ScenePaint::root_id(&tree).is_none());
    // 布局入口在 fail-stop 后只允许安全 no-op。
    tree.layout();
    // 后续动态协调必须在执行任何用户组件前拒绝输入。
    assert!(!ViewAdapter::reconcile_dynamic_children(
        // 尝试再次协调原半树。
        &mut tree,
        // 使用仍可表示但不再允许执行工作的旧 owner。
        owner,
        // 空批次也不能绕过 fail-stop 准入门。
        Vec::new(),
    ));
    // 受控 teardown 释放所有框架持有资源且不执行半树用户工作。
    tree.shutdown();
    // 根 Effect 捕获资源必须在 shutdown 返回前释放。
    assert!(weak_effect_resource.upgrade().is_none());
    // shutdown 必须清空根身份并隐藏全部半提交节点。
    assert!(tree.root_id().is_none());
    // 重复关闭保持幂等且不得把旧树恢复为 Operational。
    tree.shutdown();
    // 旧 owner 的终止状态不可重置。
    assert!(!tree.accepts_external_work());
    // 创建全新树是窗口恢复的唯一合法路径。
    let replacement = ViewAdapter::build_nodes(ViewNode::leaf(Container::new()));
    // 新 owner 使用独立运行态并正常接受外部工作。
    assert!(replacement.accepts_external_work());
}

// 验证 shutdown 即使在用户生命周期 panic 后也会释放全部节点资源。
#[test]
// 执行正常树关闭的异常安全与重复关闭门禁。
fn fail_stop_shutdown_lifecycle_panic_releases_resources() {
    // 建立仅由测试组件最终持有的共享资源。
    let resource = Arc::new(());
    // 保存弱引用以观察节点是否在异常路径完成释放。
    let weak_resource = Arc::downgrade(&resource);
    // 克隆资源并构造具有 destroy 异常的正常运行树。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(PanicOnDestroy {
        // 把组件持有的强引用交给树所有权。
        resource: Arc::clone(&resource),
    }));
    // 丢弃测试侧强引用，后续只观察树拥有资源。
    drop(resource);
    // 关闭前节点必须仍持有测试资源。
    assert!(weak_resource.upgrade().is_some());
    // 捕获 shutdown 在全部清理完成后恢复的生命周期异常。
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 正常树关闭会运行 destroy 并随后释放所有框架资源。
        tree.shutdown();
    }));
    // 生命周期异常必须传播给窗口所有者。
    let payload = result.expect_err("destroy 测试必须传播原始 panic");
    // shutdown 不得替换或吞掉用户生命周期 payload。
    assert_eq!(panic_message(payload), "关闭生命周期测试异常");
    // 即使 destroy panic，节点捕获资源也必须在返回前释放。
    assert!(weak_resource.upgrade().is_none());
    // 异常关闭仍必须清空根与全部节点所有权。
    assert!(tree.root_id().is_none());
    // 重复关闭已经进入终态的树必须保持幂等。
    tree.shutdown();
    // 关闭终态不得重新开放任何外部工作。
    assert!(!tree.accepts_external_work());
}

// 验证嵌套事务内 shutdown 不会破坏 checkpoint 或允许继续发布。
#[test]
// 执行关闭重入、原始 payload 保留与 stopped mutator 拒绝门禁。
fn fail_stop_nested_shutdown_preserves_panic_and_rejects_mutation() {
    // 建立具有合法动态捕获宿主的正常树。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Container::new()));
    // 读取外层捕获命名空间所属的有效节点身份。
    let owner = tree.root_id().expect("嵌套关闭测试必须拥有根节点");
    // 捕获一个携带未提交回执的声明根以形成非零外层检查点。
    let mut captured = ViewAdapter::capture_dynamic_root(
        // 使用当前树唯一组件状态存储。
        &tree,
        // 把捕获实例归属到真实根节点。
        owner,
        // 使用独立延迟工厂槽位避免与其他测试共享身份。
        "shutdown-checkpoint",
        // 使用稳定业务键构造唯一命名空间。
        "pending",
        // 构造无需额外状态的最小声明节点。
        || ViewNode::leaf(Label::new("pending")),
    );
    // 从声明根取走回执，使外层事务在进入内层前持有检查点项。
    let receipts = take_component_state_receipts(&mut captured);
    // 确认测试前提确实包含一个树状态回执。
    assert_eq!(receipts.len(), 1);
    // 外层事务接收回执并在内部捕获 shutdown 后的原始 panic。
    tree.with_component_state_transaction(receipts, |tree| {
        // 捕获内层事务关闭并 panic 的异常，模拟调用方继续执行 action。
        let inner = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // 建立 checkpoint 大于零的嵌套协调事务。
            tree.with_component_state_transaction(Vec::new(), |tree| {
                // shutdown 会释放整树和所有暂存 journal。
                tree.shutdown();
                // 在关闭后触发稳定异常以验证 checkpoint 恢复不覆盖 payload。
                panic!("嵌套关闭测试异常");
            });
        }));
        // 内层异常必须越过事务边界交还原始 payload。
        let payload = inner.expect_err("嵌套关闭必须传播原始 panic");
        // 回滚边界不得以 split_off 越界异常替换用户消息。
        assert_eq!(panic_message(payload), "嵌套关闭测试异常");
        // 即使调用方捕获异常并继续，停止树也必须拒绝新的真实结构发布。
        let mutation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // 直接 mutator 必须在改写 root 或 nodes 前失败。
            tree.set_root(Box::new(Container::new()));
        }));
        // shutdown 后的结构修改不能静默重建半树。
        assert!(mutation.is_err());
    });
    // 外层事务正常返回后树仍必须保持永久关闭终态。
    assert!(!tree.accepts_external_work());
    // 被拒绝的 mutator 不得重新建立根节点。
    assert!(tree.root_id().is_none());
    // 重复关闭确认剩余资源与 journal 清理保持幂等。
    tree.shutdown();
}
