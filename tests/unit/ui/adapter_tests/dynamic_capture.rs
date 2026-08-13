// 导入父测试模块可访问的声明树适配器。
use super::ViewAdapter;
// 导入动态组件私有状态作用域与字段状态构造入口。
use crate::ui::component_state::{uix_component_scope, uix_component_state};
// 导入检查运行时子节点集合所需的树节点核心接口。
use crate::ui::component::widget::WidgetCore;
// 导入构造动态捕获声明节点所需的节点类型。
use crate::ui::view::ViewNode;
// 导入构造最小宿主和动态子节点所需的组件。
use crate::ui::widgets::{Container, Label};
// 导入验证树级动画所有权与私有状态复用所需的公开类型。
use crate::ui::{
    // 导入活动动画值。
    Animated,
    // 导入稳定运行时组件身份。
    ComponentId,
    // 导入动画缓动。
    Easing,
    // 导入组件快照字段。
    SnapshotFields,
    // 导入响应式状态。
    State,
    // 导入最小测试组件能力契约。
    WidgetCapabilities,
    // 导入测试组件接口。
    WidgetComponent,
    // 导入宿主运行时树。
    WidgetTree,
};
// 导入测试组件类型擦除接口。
use std::any::Any;

// 在动态协调期间按配置触发建树或快照异常。
struct PanicSibling {
    // 控制新节点建树阶段是否触发异常。
    panic_on_build: bool,
    // 控制既有节点协调阶段是否触发异常。
    panic_on_snapshot: bool,
}

// 为异常门禁提供最小运行时组件实现。
impl WidgetComponent for PanicSibling {
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

    // 声明测试组件不需要布局或渲染能力。
    fn capabilities(&self) -> WidgetCapabilities {
        // 返回空能力集合。
        WidgetCapabilities::new()
    }

    // 在新节点真实建树入口按配置触发异常。
    fn build(&self) -> Vec<Box<dyn WidgetComponent>> {
        // 让失败用例停在第二个兄弟节点的建树阶段。
        assert!(!self.panic_on_build, "测试兄弟节点建树异常");
        // 正常路径不生成内部子节点。
        Vec::new()
    }

    // 在既有节点协调读取新版快照时按配置触发异常。
    fn snapshot_fields(&self) -> SnapshotFields {
        // 让失败用例停在第二个兄弟节点的原位协调阶段。
        assert!(!self.panic_on_snapshot, "测试兄弟节点快照异常");
        // 正常路径使用未知快照保持测试组件的保守协调语义。
        SnapshotFields::Unknown
    }
}

// 在指定宿主与业务键的动态命名空间中捕获一个私有状态节点。
fn capture_state_node(
    // 接收提供唯一组件状态存储的宿主树。
    tree: &WidgetTree,
    // 接收拥有延迟工厂实例的运行时宿主节点。
    owner: ComponentId,
    // 接收在工厂执行前确定的稳定业务键。
    stable_key: &str,
    // 回传本次捕获得到的私有状态句柄以供行为断言。
    captured_state: &mut Option<State<i32>>,
    // 返回携带状态作用域标记与运行时捕获输出的声明节点。
) -> ViewNode {
    // 复制业务键供动态命名空间拥有。
    let namespace_key = stable_key.to_owned();
    // 复制业务键供声明节点的 keyed reconcile 使用。
    let node_key = stable_key.to_owned();
    // 在宿主、槽位和业务键共同限定的命名空间中执行工厂。
    ViewAdapter::capture_dynamic_root(tree, owner, "test-item", namespace_key, || {
        // 申请本次内联组件调用在动态命名空间中的稳定作用域。
        let scope = uix_component_scope("dynamic-capture-test", 1);
        // 取得或初始化该动态实例的私有状态字段。
        let state = uix_component_state(&scope, 1, || 0_i32);
        // 把状态句柄回传给测试以观测跨捕获值语义。
        *captured_state = Some(state);
        // 构造可按业务键原位协调的最小动态子节点。
        ViewNode::leaf(Label::new(node_key.clone()))
            // 让运行时子树重排时按同一业务键寻找既有节点。
            .key(node_key)
            // 让真实挂载节点参与组件私有状态作用域保留与清理。
            .uix_component_scope(scope, 0)
    })
}

// 取得测试捕获必须回传的私有状态句柄。
fn captured_state(
    // 接收可能尚未由工厂写入的状态槽。
    state: Option<State<i32>>,
    // 返回已确认存在的响应式状态句柄。
) -> State<i32> {
    // 动态工厂在同步捕获返回前必须已经写入状态句柄。
    state.expect("动态捕获必须同步返回私有状态句柄")
}

// 验证业务键重排复用、不同键隔离以及真实移除后的重新初始化。
#[test]
// 执行动态命名空间状态完整生命周期回归。
fn dynamic_capture_namespace_preserves_reordered_keys_and_releases_removed_key() {
    // 建立可承载动态延迟 View 的稳定宿主树。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Container::new()));
    // 读取动态工厂实例共同所属的实际宿主节点。
    let owner = tree.root_id().expect("动态捕获测试必须拥有宿主根节点");
    // 准备接收业务键 A 首次捕获的私有状态。
    let mut first_a = None;
    // 捕获业务键 A 的首次声明节点。
    let node_a = capture_state_node(&tree, owner, "a", &mut first_a);
    // 准备接收业务键 B 首次捕获的私有状态。
    let mut first_b = None;
    // 捕获业务键 B 的首次声明节点。
    let node_b = capture_state_node(&tree, owner, "b", &mut first_b);
    // 同轮挂载两个稳定业务实例以提交各自状态回执。
    ViewAdapter::reconcile_dynamic_children(&mut tree, owner, vec![node_a, node_b]);
    // 取得业务键 A 已挂载的私有状态。
    let first_a = captured_state(first_a);
    // 取得业务键 B 已挂载的私有状态。
    let first_b = captured_state(first_b);
    // 写入业务键 A 独立的新值。
    first_a.set(7);
    // 写入业务键 B 独立的新值。
    first_b.set(9);

    // 准备接收重排后业务键 B 的复用状态。
    let mut reordered_b = None;
    // 先捕获业务键 B 以模拟声明顺序交换。
    let node_b = capture_state_node(&tree, owner, "b", &mut reordered_b);
    // 准备接收重排后业务键 A 的复用状态。
    let mut reordered_a = None;
    // 再捕获业务键 A 以完成相反顺序。
    let node_a = capture_state_node(&tree, owner, "a", &mut reordered_a);
    // 在相反顺序下协调同两个业务实例。
    ViewAdapter::reconcile_dynamic_children(&mut tree, owner, vec![node_b, node_a]);
    // 同一业务键 A 必须保留其原值而不跟随声明位置。
    assert_eq!(captured_state(reordered_a).get(), 7);
    // 同一业务键 B 必须保留其原值而不与 A 串槽。
    assert_eq!(captured_state(reordered_b).get(), 9);

    // 准备接收仅保留业务键 B 时的复用状态。
    let mut retained_b = None;
    // 捕获仍应存活的业务键 B。
    let node_b = capture_state_node(&tree, owner, "b", &mut retained_b);
    // 从实际动态子树中移除业务键 A。
    ViewAdapter::reconcile_dynamic_children(&mut tree, owner, vec![node_b]);
    // 未移除的业务键 B 必须继续保留既有值。
    assert_eq!(captured_state(retained_b).get(), 9);

    // 准备接收业务键 A 真实移除后再次进入的状态。
    let mut remounted_a = None;
    // 重新捕获已被真实移除的业务键 A。
    let node_a = capture_state_node(&tree, owner, "a", &mut remounted_a);
    // 真实移除后的业务键 A 必须重新执行字段初始化。
    assert_eq!(captured_state(remounted_a).get(), 0);
    // 再次挂载 A 以让新状态回执进入宿主树事务。
    ViewAdapter::reconcile_dynamic_children(&mut tree, owner, vec![node_a]);
}

// 验证目标树拒绝接纳另一状态存储产生的动态捕获回执。
#[test]
// 执行错误 store 回执的即时回滚回归。
fn dynamic_capture_foreign_store_receipt_rolls_back_instead_of_leaking() {
    // 建立实际接收动态子树的目标宿主树。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Container::new()));
    // 读取目标树的实际宿主节点身份。
    let owner = tree.root_id().expect("错误 store 测试必须拥有宿主根节点");
    // 建立与目标树无关的一次性状态存储。
    let foreign_store = crate::ui::component_state::ComponentStateStore::new();
    // 克隆外来存储以供首次错误捕获使用。
    let captured_foreign_store = foreign_store.clone();
    // 记录首次错误捕获返回的私有状态句柄。
    let mut first_state = None;
    // 使用目标 owner 与外来 store 直接构造不一致的底层捕获结果。
    let (mut foreign, foreign_receipt) =
        crate::ui::component_state::with_component_state_capture_in_namespace(
            // 故意传入不属于目标树的 store 以验证事务纵深防护。
            captured_foreign_store,
            // 构造使用目标 owner 但归属外来 store 的命名空间。
            crate::ui::component_state::ComponentStateCaptureNamespace::new(
                // 使用目标树中确实存在的宿主身份。
                owner,
                // 使用稳定测试槽位。
                "foreign-store",
                // 使用固定业务键。
                "only",
            ),
            // 在外来 store 中建立一个可回滚私有字段。
            || {
                // 创建稳定组件调用作用域。
                let scope = uix_component_scope("foreign-store-test", 1);
                // 在外来 store 中插入本次字段状态。
                let state = uix_component_state(&scope, 1, || 3_i32);
                // 回传首次状态句柄以证明工厂确实执行。
                first_state = Some(state);
                // 返回承载该作用域的动态声明节点。
                ViewNode::leaf(Label::new("foreign"))
                    // 使用固定运行时协调键。
                    .key("only")
                    // 挂载相同私有状态作用域标记。
                    .uix_component_scope(scope, 0)
            },
        );
    // 把外来状态 journal 附到声明节点以模拟错误接线输入。
    foreign.push_component_state_receipt(foreign_receipt);
    // 把错误 store 捕获结果交给目标树事务。
    ViewAdapter::reconcile_dynamic_children(&mut tree, owner, vec![foreign]);
    // 首次外来捕获必须确实完成字段初始化。
    assert_eq!(captured_state(first_state).get(), 3);

    // 记录后续同命名空间是否重新执行初始化。
    let initializer_runs = std::cell::Cell::new(0_u32);
    // 在相同外来 store 和命名空间中再次捕获该字段。
    let (_second, second_receipt) =
        crate::ui::component_state::with_component_state_capture_in_namespace(
            // 继续使用首次捕获的外来 store。
            foreign_store,
            // 重建完全相同的动态实例命名空间。
            crate::ui::component_state::ComponentStateCaptureNamespace::new(
                // 保持宿主身份不变。
                owner,
                // 保持静态槽位不变。
                "foreign-store",
                // 保持稳定业务键不变。
                "only",
            ),
            // 再次申请相同私有状态字段。
            || {
                // 创建与首次捕获相同的静态组件调用作用域。
                let scope = uix_component_scope("foreign-store-test", 1);
                // 若错误回执已回滚，该初始化必须重新执行。
                uix_component_state(&scope, 1, || {
                    // 记录未泄漏旧槽时发生的重新初始化。
                    initializer_runs.set(initializer_runs.get().saturating_add(1));
                    // 返回可区分首次值的新初始值。
                    8_i32
                })
            },
        );
    // 丢弃尚未挂载的第二次回执以恢复测试外来 store。
    drop(second_receipt);
    // 目标树不得接纳外来回执，因此相同字段必须重新初始化一次。
    assert_eq!(initializer_runs.get(), 1);
}

// 验证动态节点动画源在原位协调中不重复注册，并在真实移除后释放。
#[test]
// 执行节点专属动画源的挂载、复用与移除回归。
fn dynamic_capture_node_animation_registration_reuses_and_releases() {
    // 建立一条保持活动状态的测试动画源。
    let animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 建立不读取该动画源的稳定宿主树。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Container::new()));
    // 读取动态动画节点所属的实际宿主身份。
    let owner = tree.root_id().expect("动态动画测试必须拥有宿主根节点");
    // 克隆动画句柄供首次动态捕获读取。
    let first_animation = animation.clone();
    // 在节点专属动态命名空间中捕获动画源。
    let first = ViewAdapter::capture_dynamic_root(
        // 让入口从已验证 owner 的宿主树取得唯一状态存储。
        &tree,
        // 使用实际宿主节点隔离动态实例。
        owner,
        // 使用固定槽位区分测试动画工厂。
        "test-animation",
        // 使用稳定业务键标识当前动态实例。
        "only",
        // 同步执行读取动画源的最小工厂。
        || {
            // 读取当前值以把动画源登记到本次 View 捕获输出。
            let _ = first_animation.value();
            // 返回带稳定 reconcile 键的动态节点。
            ViewNode::leaf(Label::new("animated child")).key("only")
        },
    );
    // 挂载动态节点并把其动画源交给树级节点 owner。
    ViewAdapter::reconcile_dynamic_children(&mut tree, owner, vec![first]);
    // 首次挂载后树中必须只有一条活动动画注册。
    assert_eq!(tree.animated_source_registrations().len(), 1);

    // 克隆相同动画句柄供原位协调再次读取。
    let second_animation = animation.clone();
    // 捕获相同宿主、槽位与业务键的下一轮声明。
    let second = ViewAdapter::capture_dynamic_root(
        // 继续从同一宿主树取得状态存储。
        &tree,
        // 保持动态工厂 owner 不变。
        owner,
        // 保持动态工厂槽位不变。
        "test-animation",
        // 保持稳定业务键不变。
        "only",
        // 同步执行第二次动画读取。
        || {
            // 再次读取同一动画源以验证注册替换去重。
            let _ = second_animation.value();
            // 返回可原位复用的同类型同键节点。
            ViewNode::leaf(Label::new("animated child")).key("only")
        },
    );
    // 原位协调相同动态节点。
    ViewAdapter::reconcile_dynamic_children(&mut tree, owner, vec![second]);
    // 原位协调不得为同一 work id 重复建立树级注册。
    assert_eq!(tree.animated_source_registrations().len(), 1);
    // 真实移除动态节点以释放节点 owner。
    ViewAdapter::reconcile_dynamic_children(&mut tree, owner, Vec::new());
    // 最后一个节点 owner 移除后不得遗留活动动画注册。
    assert!(tree.animated_source_registrations().is_empty());
}

// 验证根与动态节点共享动画源时按 owner 分区并在最后 owner 消失后解绑。
#[test]
// 执行共享动画源的根节点所有权分区回归。
fn dynamic_capture_shared_animation_keeps_root_owner_after_child_remove() {
    // 建立由根与动态节点共同读取的活动动画源。
    let animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 克隆动画句柄供静态根捕获读取。
    let root_animation = animation.clone();
    // 捕获拥有根动画 owner 的初始声明树。
    let root = ViewAdapter::capture_root(|| {
        // 读取动画值以登记根 owner。
        let _ = root_animation.value();
        // 返回可承载动态子节点的稳定宿主。
        ViewNode::leaf(Container::new())
    });
    // 建立已经拥有根动画注册的运行时树。
    let mut tree = ViewAdapter::build_nodes(root);
    // 读取动态子节点所属的实际根身份。
    let owner = tree.root_id().expect("共享动画测试必须拥有宿主根节点");
    // 根捕获完成后必须存在一条活动动画注册。
    assert_eq!(tree.animated_source_registrations().len(), 1);

    // 克隆相同动画句柄供动态节点读取。
    let child_animation = animation.clone();
    // 捕获共享同一 work id 的动态节点。
    let child = ViewAdapter::capture_dynamic_root(
        // 让入口使用当前宿主树的唯一状态存储。
        &tree,
        // 把动态实例归属到实际根节点。
        owner,
        // 使用固定动态槽位。
        "shared-animation",
        // 使用固定动态业务键。
        "child",
        // 同步执行共享动画读取。
        || {
            // 读取同一动画源以增加节点 owner 而非重复绑定树。
            let _ = child_animation.value();
            // 返回稳定动态节点。
            ViewNode::leaf(Label::new("shared child")).key("child")
        },
    );
    // 挂载共享根动画源的动态节点。
    ViewAdapter::reconcile_dynamic_children(&mut tree, owner, vec![child]);
    // 同一 work id 的两个 owner 仍只应形成一条调度注册。
    assert_eq!(tree.animated_source_registrations().len(), 1);
    // 移除动态节点但保留根声明 owner。
    ViewAdapter::reconcile_dynamic_children(&mut tree, owner, Vec::new());
    // 节点 owner 消失后根 owner 必须继续保留动画注册。
    assert_eq!(tree.animated_source_registrations().len(), 1);
    // 用不再读取动画源的新根声明替换根 owner。
    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(Container::new()));
    // 最后一个根 owner 消失后动画源必须从树级注册表移除。
    assert!(tree.animated_source_registrations().is_empty());
}

// 验证窗口树关闭会释放仍挂载动态节点拥有的动画源。
#[test]
// 执行动态动画源 shutdown 生命周期回归。
fn dynamic_capture_shutdown_releases_mounted_node_animation() {
    // 建立关闭前保持活动的动画源。
    let animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 建立可承载动态动画节点的宿主树。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Container::new()));
    // 读取动态实例实际所属的宿主节点。
    let owner = tree.root_id().expect("shutdown 动画测试必须拥有宿主根节点");
    // 克隆动画句柄供动态工厂读取。
    let captured_animation = animation.clone();
    // 捕获关闭前仍会保持挂载的动态节点。
    let child = ViewAdapter::capture_dynamic_root(
        // 让入口使用宿主树唯一状态存储。
        &tree,
        // 绑定到实际宿主节点。
        owner,
        // 使用固定延迟工厂槽位。
        "shutdown-animation",
        // 使用固定动态业务键。
        "child",
        // 同步执行动画读取工厂。
        || {
            // 读取动画值以建立节点 owner。
            let _ = captured_animation.value();
            // 返回关闭前保持挂载的动态节点。
            ViewNode::leaf(Label::new("shutdown child")).key("child")
        },
    );
    // 挂载携带动画源的动态节点。
    ViewAdapter::reconcile_dynamic_children(&mut tree, owner, vec![child]);
    // 关闭前必须存在活动动画注册。
    assert_eq!(tree.animated_source_registrations().len(), 1);
    // 关闭整棵窗口树并释放全部 owner 分区。
    tree.shutdown();
    // shutdown 后不得保留任何动态动画注册。
    assert!(tree.animated_source_registrations().is_empty());
}

// 验证后续兄弟节点建树异常不会半提交前一个新节点的动画源。
#[test]
// 执行新兄弟节点动画登记的事务回滚回归。
fn dynamic_capture_animation_build_panic_discards_preceding_source_update() {
    // 建立将在失败事务中首次出现的活动动画源。
    let animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 建立初始没有动画登记的稳定宿主树。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Container::new()));
    // 读取两个动态兄弟节点共同所属的宿主身份。
    let owner = tree.root_id().expect("动画异常测试必须拥有宿主根节点");
    // 克隆动画句柄供第一个兄弟节点的动态捕获读取。
    let captured_animation = animation.clone();
    // 捕获会在事务中先成功建立的新动画节点。
    let animated_child = ViewAdapter::capture_dynamic_root(
        // 让入口使用宿主树唯一的组件状态存储。
        &tree,
        // 把动态实例归属到实际根节点。
        owner,
        // 使用固定延迟工厂槽位。
        "panic-build-animation",
        // 使用稳定业务键标识第一个兄弟节点。
        "animated",
        // 同步执行动画读取工厂。
        || {
            // 读取动画值以生成尚未提交的节点动画源输出。
            let _ = captured_animation.value();
            // 返回可按业务键协调的第一个兄弟节点。
            ViewNode::leaf(Label::new("animated")).key("animated")
        },
    );
    // 构造会在随后建树时触发异常的第二个兄弟节点。
    let panicking_child = ViewNode::leaf(PanicSibling {
        // 确保异常发生在新节点 build 阶段。
        panic_on_build: true,
        // 本用例不进入既有节点快照路径。
        panic_on_snapshot: false,
    })
    // 使用独立业务键确保适配器尝试新建第二个节点。
    .key("panic");
    // 捕获第二个兄弟节点失败时向上传播的原始异常。
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 在同一树事务内依次处理成功动画节点与失败兄弟节点。
        ViewAdapter::reconcile_dynamic_children(
            // 协调当前测试宿主树。
            &mut tree,
            // 使用两个兄弟节点的共同父节点。
            owner,
            // 保持失败节点位于动画节点之后。
            vec![animated_child, panicking_child],
        );
    }));
    // 测试组件的建树异常必须继续向上传播。
    assert!(result.is_err());
    // 失败事务不得留下第一个兄弟节点的半提交动画登记。
    assert!(tree.animated_source_registrations().is_empty());
}

// 验证既有节点的新动画源遇到后续兄弟协调异常时保留旧动画登记。
#[test]
// 执行原位动画替换的事务回滚回归。
fn dynamic_capture_animation_patch_panic_preserves_previous_source() {
    // 建立首次成功挂载并应在失败后继续保留的旧动画源。
    let old_animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 记录旧动画源的稳定工作身份供失败后精确断言。
    let old_source_id = old_animation.group_source_id();
    // 建立初始没有动画登记的稳定宿主树。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Container::new()));
    // 读取两个动态兄弟节点共同所属的宿主身份。
    let owner = tree.root_id().expect("动画替换异常测试必须拥有宿主根节点");
    // 克隆旧动画句柄供首次动态捕获读取。
    let captured_old_animation = old_animation.clone();
    // 捕获首次将成功挂载的旧动画节点。
    let old_animated_child = ViewAdapter::capture_dynamic_root(
        // 让入口使用宿主树唯一的组件状态存储。
        &tree,
        // 把动态实例归属到实际根节点。
        owner,
        // 使用固定延迟工厂槽位。
        "panic-patch-animation",
        // 使用稳定业务键标识动画兄弟节点。
        "animated",
        // 同步执行旧动画读取工厂。
        || {
            // 读取旧动画值以形成节点动画源输出。
            let _ = captured_old_animation.value();
            // 返回首次可挂载的稳定动画节点。
            ViewNode::leaf(Label::new("old animated")).key("animated")
        },
    );
    // 构造首次协调中不会失败的第二个兄弟节点。
    let stable_sibling = ViewNode::leaf(PanicSibling {
        // 首次挂载必须允许正常建树。
        panic_on_build: false,
        // 首次挂载不会读取既有节点快照。
        panic_on_snapshot: false,
    })
    // 使用固定键供下一轮原位协调复用同一运行时节点。
    .key("panic");
    // 首次成功挂载旧动画节点和稳定兄弟节点。
    ViewAdapter::reconcile_dynamic_children(
        // 协调当前测试宿主树。
        &mut tree,
        // 使用两个兄弟节点的共同父节点。
        owner,
        // 同轮提交旧动画登记与第二个稳定节点。
        vec![old_animated_child, stable_sibling],
    );
    // 读取首次成功事务后的动画登记快照。
    let initial_registrations = tree.animated_source_registrations();
    // 首次挂载后必须只有旧动画源一条登记。
    assert_eq!(initial_registrations.len(), 1);
    // 首次登记必须属于旧动画源工作身份。
    assert_eq!(initial_registrations[0].0, old_source_id);

    // 建立只应在下一轮事务成功时替换旧来源的新动画源。
    let new_animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 记录新动画源身份以证明失败后没有被树接纳。
    let new_source_id = new_animation.group_source_id();
    // 克隆新动画句柄供第二轮动态捕获读取。
    let captured_new_animation = new_animation.clone();
    // 捕获与旧节点同键同类型的新动画声明。
    let new_animated_child = ViewAdapter::capture_dynamic_root(
        // 继续使用当前宿主树唯一的组件状态存储。
        &tree,
        // 保持动态实例宿主不变。
        owner,
        // 保持延迟工厂槽位不变。
        "panic-patch-animation",
        // 保持动画节点业务键不变。
        "animated",
        // 同步执行新动画读取工厂。
        || {
            // 读取新动画值以形成待提交的替换请求。
            let _ = captured_new_animation.value();
            // 返回可原位复用的同型同键节点。
            ViewNode::leaf(Label::new("new animated")).key("animated")
        },
    );
    // 构造会在既有第二个兄弟节点快照阶段失败的新声明。
    let panicking_sibling = ViewNode::leaf(PanicSibling {
        // 原位协调路径不得先在 build 阶段失败。
        panic_on_build: false,
        // 让第二个兄弟节点在快照读取阶段触发异常。
        panic_on_snapshot: true,
    })
    // 保持键不变以强制进入同类型原位协调路径。
    .key("panic");
    // 捕获第二个兄弟节点失败时向上传播的原始异常。
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 在同一树事务内先暂存新动画替换，再协调失败兄弟节点。
        ViewAdapter::reconcile_dynamic_children(
            // 协调当前测试宿主树。
            &mut tree,
            // 使用两个既有兄弟节点的共同父节点。
            owner,
            // 保持失败节点位于待替换动画节点之后。
            vec![new_animated_child, panicking_sibling],
        );
    }));
    // 测试组件的快照异常必须继续向上传播。
    assert!(result.is_err());
    // 读取失败事务后的实际动画登记快照。
    let registrations_after_panic = tree.animated_source_registrations();
    // 失败事务不得增加或删除动画登记。
    assert_eq!(registrations_after_panic.len(), 1);
    // 失败事务后必须继续保留旧动画源。
    assert_eq!(registrations_after_panic[0].0, old_source_id);
    // 失败事务后不得接纳新动画源。
    assert_ne!(registrations_after_panic[0].0, new_source_id);
}

// 验证捕获后已经失效的宿主身份不能产生孤立节点或遗留运行时输出。
#[test]
// 执行旧 generation 动态交付拒绝与回执回滚回归。
fn dynamic_capture_rejects_stale_owner_before_mounting_outputs() {
    // 建立将在旧宿主命名空间中首次初始化的状态值。
    let mut captured_slot = None;
    // 建立将在旧宿主动态捕获中读取的活动动画源。
    let animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 建立最初可接收动态子树的容器宿主。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Container::new()));
    // 记录捕获时仍有效的旧宿主身份。
    let old_owner = tree.root_id().expect("旧宿主交付测试必须拥有初始根节点");
    // 克隆动画句柄供延迟工厂读取。
    let captured_animation = animation.clone();
    // 在旧宿主仍有效时生成完整动态捕获结果。
    let stale_child = ViewAdapter::capture_dynamic_root(
        // 让入口从初始宿主树取得唯一状态存储。
        &tree,
        // 记录即将因完整换根而失效的 owner generation。
        old_owner,
        // 使用固定延迟工厂槽位。
        "stale-owner",
        // 使用固定稳定业务键。
        "child",
        // 同步执行同时产生 State 与 AnimatedSource 的工厂。
        || {
            // 创建稳定组件调用作用域。
            let scope = uix_component_scope("stale-owner-test", 1);
            // 在尚未提交的旧宿主命名空间中建立私有状态。
            let state = uix_component_state(&scope, 1, || 4_i32);
            // 回传状态句柄以证明捕获已经执行。
            captured_slot = Some(state);
            // 读取动画值以形成尚未交付的动画源输出。
            let _ = captured_animation.value();
            // 返回带稳定键和状态作用域标记的声明节点。
            ViewNode::leaf(Label::new("stale child"))
                // 使用稳定键参与后续动态协调。
                .key("child")
                // 让状态作用域随实际节点生命周期交接。
                .uix_component_scope(scope, 0)
        },
    );
    // 捕获阶段必须已经返回旧命名空间的状态句柄。
    assert_eq!(captured_state(captured_slot).get(), 4);
    // 用不同组件类型完整替换根并使旧 owner generation 失效。
    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(Label::new("new root")));
    // 完整换根后旧宿主身份必须不可寻址。
    assert!(tree.get(old_owner).is_none());
    // 记录当前仍有效的新根身份。
    let new_owner = tree.root_id().expect("完整换根后必须拥有新根节点");
    // 捕获把旧动态结果交付给失效 parent 时的稳定拒绝。
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 尝试向旧 generation 交付此前合法捕获的动态节点。
        ViewAdapter::reconcile_dynamic_children(
            // 使用已经完成换根的当前宿主树。
            &mut tree,
            // 故意传入已经失效的旧宿主身份。
            old_owner,
            // 交付仍携带未提交状态与动画输出的旧捕获节点。
            vec![stale_child],
        );
    }));
    // 失效宿主交付必须在结构变化前明确拒绝。
    assert!(result.is_err());
    // 新根不得获得无法遍历的孤立动态子节点。
    assert!(tree
        // 读取当前有效根节点。
        .get(new_owner)
        // 有效根节点必须仍可寻址。
        .expect("旧宿主交付失败后新根必须保留")
        // 检查其直接子节点集合。
        .children()
        // 新根不应被失败交付污染。
        .is_empty());
    // 失败交付不得把旧捕获动画源登记到当前树。
    assert!(tree.animated_source_registrations().is_empty());

    // 记录失败交付后相同旧命名空间是否重新执行初始化。
    let initializer_runs = std::cell::Cell::new(0_u32);
    // 直接复查同一树存储中旧命名空间的临时槽是否已经回滚。
    let (_state, receipt) = crate::ui::component_state::with_component_state_capture_in_namespace(
        // 继续使用完成换根后的同一树状态存储。
        tree.component_state_store(),
        // 重建与失败交付完全相同的旧宿主动态身份。
        crate::ui::component_state::ComponentStateCaptureNamespace::new(
            // 保持旧 generation owner 不变以检查原槽。
            old_owner,
            // 保持延迟工厂槽位不变。
            "stale-owner",
            // 保持稳定业务键不变。
            "child",
        ),
        // 再次申请同一静态组件字段。
        || {
            // 创建与旧捕获相同的静态调用作用域。
            let scope = uix_component_scope("stale-owner-test", 1);
            // 若旧回执已回滚，初始化必须重新执行一次。
            uix_component_state(&scope, 1, || {
                // 记录旧临时槽已经被正确释放。
                initializer_runs.set(initializer_runs.get().saturating_add(1));
                // 返回可区分旧值的新初始值。
                9_i32
            })
        },
    );
    // 丢弃复查捕获回执以恢复测试存储。
    drop(receipt);
    // 失效宿主的旧回执必须已经回滚而非泄漏。
    assert_eq!(initializer_runs.get(), 1);
}

// 验证内层已经发布结构后发生 panic 会让共享外层事务永久 fail-stop。
#[test]
// 执行嵌套动画取消、回滚与 fail-stop 准入回归。
fn dynamic_capture_nested_animation_publish_panic_fail_stops_outer_transaction() {
    // 建立将由外层事务先暂存的动画源 A。
    let animation_a = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 建立将在内层失败事务中暂存的动画源 C。
    let animation_c = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 建立最初没有动态子节点的稳定宿主树。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Container::new()));
    // 读取所有嵌套动态协调共同所属的根身份。
    let owner = tree.root_id().expect("嵌套动画事务测试必须拥有宿主根节点");
    // 克隆动画 A 供外层事务内第一次动态捕获读取。
    let captured_animation_a = animation_a.clone();
    // 在进入外层事务前构造带动画 A 的声明节点。
    let child_a = ViewAdapter::capture_dynamic_root(
        // 让入口使用宿主树唯一状态存储。
        &tree,
        // 把延迟实例归属到实际根节点。
        owner,
        // 使用固定嵌套测试槽位。
        "nested-animation",
        // 使用将在内层被同键不同类型替换的业务键。
        "same",
        // 同步执行动画 A 的读取工厂。
        || {
            // 读取动画 A 以形成外层待提交来源。
            let _ = captured_animation_a.value();
            // 返回首次挂载的 Label 节点。
            ViewNode::leaf(Label::new("child a")).key("same")
        },
    );
    // 开启一层显式外层事务以保留动画 A 的待提交队列项。
    tree.with_component_state_transaction(Vec::new(), |tree| {
        // 在嵌套成功事务中挂载动画 A，但暂不提交到中央注册表。
        ViewAdapter::reconcile_dynamic_children(
            // 协调外层事务拥有的当前树。
            tree,
            // 使用稳定宿主根节点。
            owner,
            // 仅挂载动画 A 节点。
            vec![child_a],
        );
        // 外层事务尚未完成时中央注册表必须保持不变。
        assert!(tree.animated_source_registrations().is_empty());

        // 克隆动画 C 供失败的内层替换捕获读取。
        let captured_animation_c = animation_c.clone();
        // 构造与 A 同键但不同类型的节点以先真实销毁 A。
        let child_c = ViewAdapter::capture_dynamic_root(
            // 继续使用同一宿主树状态存储。
            tree,
            // 保持动态实例宿主不变。
            owner,
            // 保持嵌套测试槽位不变。
            "nested-animation",
            // 保持业务键不变以命中 A 的运行时节点。
            "same",
            // 同步执行动画 C 的读取工厂。
            || {
                // 读取动画 C 以形成内层待提交来源。
                let _ = captured_animation_c.value();
                // 使用 Container 类型迫使适配器先移除旧 Label 节点。
                ViewNode::leaf(Container::new()).key("same")
            },
        );
        // 构造会在 C 建立后触发异常的第二个兄弟节点。
        let panicking_child = ViewNode::leaf(PanicSibling {
            // 让异常发生在新节点 build 阶段。
            panic_on_build: true,
            // 本用例不进入既有节点快照路径。
            panic_on_snapshot: false,
        })
        // 使用独立键确保 C 先完成替换与待提交登记。
        .key("panic");
        // 捕获内层协调传播的原始测试异常。
        let inner_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // 先移除 A 并暂存 C，再由第二个兄弟节点触发回滚。
            ViewAdapter::reconcile_dynamic_children(
                // 协调仍处于外层事务中的当前树。
                tree,
                // 保持共同宿主根不变。
                owner,
                // 保持 C 位于失败兄弟节点之前。
                vec![child_c, panicking_child],
            );
        }));
        // 内层原始异常必须可被外层捕获而不发生 checkpoint 越界二次 panic。
        assert!(inner_result.is_err());
    });
    // A 已真实销毁且 C 所属内层事务失败，外层成功后不得登记任一来源。
    assert!(tree.animated_source_registrations().is_empty());
    // 任一内层真实发布都把最外层共享事务标记为不可恢复。
    assert!(!tree.accepts_external_work());
    // fail-stop 后的动态协调必须拒绝继续改写半完成树。
    assert!(!ViewAdapter::reconcile_dynamic_children(
        // 尝试复用已经进入故障停止状态的旧树。
        &mut tree,
        // 传入旧 owner 证明准入门先于结构协调执行。
        owner,
        // 空声明仍不得绕过 fail-stop 门禁。
        Vec::new(),
    ));
    // 所有者 teardown 必须可以幂等释放半完成树资源。
    tree.shutdown();
    // 重复关闭不得重新执行用户工作或恢复旧树。
    tree.shutdown();
    // 关闭后的旧树仍永久拒绝外部工作。
    assert!(!tree.accepts_external_work());
    // 只有全新 WidgetTree owner 才能恢复正常成功路径。
    let replacement = ViewAdapter::build_nodes(ViewNode::leaf(Container::new()));
    // 新 owner 必须以独立 Operational 状态开始。
    assert!(replacement.accepts_external_work());
}
