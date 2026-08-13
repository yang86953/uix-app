// 复用父模块私有 Anchor 类型以测试容器动态子树。
use super::Anchor;
// 导入真实声明协调与建树入口。
use crate::ui::adapter::ViewAdapter;
// 导入组件私有状态 scope 与槽位创建入口。
use crate::ui::component_state::{uix_component_scope, uix_component_state};
// 导入运行时节点读取所需的核心契约。
use crate::ui::component::widget::WidgetCore;
// 导入最小动态声明节点类型。
use crate::ui::view::ViewNode;
// 导入用于辨识容器工厂版本的最小组件。
use crate::ui::widgets::Label;
// 导入动态捕获、生命周期与异常测试所需类型。
use crate::ui::{
    Animated, ComponentId, Easing, Effect, SnapshotFields, State, Transition, WidgetCapabilities,
    WidgetComponent, WidgetTree,
};
// 导入手写发布异常组件所需的类型擦除接口。
use std::any::Any;
// 导入工厂与 Effect 调用次数记录。
use std::sync::atomic::{AtomicUsize, Ordering};
// 导入跨协调轮次共享的状态记录。
use std::sync::{Arc, Mutex};

// 读取 Anchor 的稳定运行时 owner。
fn anchor_root(
    // 接收已经成功挂载的运行时树。
    tree: &WidgetTree,
    // 返回当前唯一根身份。
) -> ComponentId {
    // Anchor 测试均只建立一个根组件。
    tree.root_id().expect("Anchor 必须拥有运行时根")
}

// 在 Anchor 的直接子节点中按产品固定 key 查找动态容器。
fn container_child(
    // 接收运行时树。
    tree: &WidgetTree,
    // 接收 Anchor owner 身份。
    root: ComponentId,
    // 返回当前动态容器身份。
) -> ComponentId {
    // 仅查找 Anchor 专属固定 key，避免误把 authored 子节点当作动态容器。
    tree.get(root)
        .expect("Anchor owner 必须存在")
        .children()
        .iter()
        .copied()
        .find(|child| {
            tree.get(*child).and_then(|node| node.key()) == Some(Anchor::CONTAINER_CHILD_KEY)
        })
        .expect("Anchor 动态容器必须存在")
}

// 从共享记录取得最近一次工厂捕获的私有状态句柄。
fn captured_state(
    // 接收由容器工厂写入的共享状态记录。
    states: &Arc<Mutex<Option<State<i32>>>>,
    // 返回可脱离记录锁使用的公开状态句柄。
) -> State<i32> {
    // 工厂完成后必须已经回传状态。
    states
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .as_ref()
        .cloned()
        .expect("容器工厂必须捕获私有 State")
}

// 只替换 live Anchor 的声明字段，避免父协调提前进入不可逆发布区。
fn sync_runtime_anchor(
    // 接收当前运行时树。
    tree: &mut WidgetTree,
    // 接收仍属于该树的 Anchor owner。
    root: ComponentId,
    // 接收将安装到 live owner 的新版声明。
    next: Anchor,
) {
    // 仅当前真实 Anchor 可以接纳测试指定的 factory。
    tree.get_mut(root)
        .expect("Anchor owner 必须存在")
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Anchor>()
        .expect("运行时 owner 必须是 Anchor")
        .sync_from(next);
}

// 构造携带完整动态捕获输出与可选 leave 的 Anchor 容器工厂。
fn captured_anchor(
    // 接收用于区分新旧工厂输出的显示文本。
    label: &'static str,
    // 接收工厂调用次数记录。
    factory_calls: Arc<AtomicUsize>,
    // 接收状态句柄回传记录。
    states: Arc<Mutex<Option<State<i32>>>>,
    // 接收 Effect 依赖。
    effect_dependency: State<i32>,
    // 接收 Effect 运行次数记录。
    effect_runs: Arc<AtomicUsize>,
    // 接收动态容器必须拥有的动画源。
    animation: Animated<f32>,
    // 接收可选的非零离场过渡。
    leave: Option<Transition>,
    // 返回尚未建树的 Anchor 声明。
) -> Anchor {
    // 克隆记录以让可重复执行的工厂闭包独占。
    let renderer_calls = Arc::clone(&factory_calls);
    // 克隆状态记录以让每轮捕获更新最新句柄。
    let renderer_states = Arc::clone(&states);
    // 克隆 Effect 计数器以让订阅闭包长期拥有。
    let renderer_effect_runs = Arc::clone(&effect_runs);
    // 返回启用动态容器的 Anchor。
    Anchor::new(Vec::new()).container(move || {
        // 每次真实容器物化或父协调重捕获都必须可观察。
        renderer_calls.fetch_add(1, Ordering::Relaxed);
        // 为固定动态容器声明稳定组件私有 scope。
        let scope = uix_component_scope("anchor-container-dynamic-capture-test", 1);
        // 在 owner、容器槽与固定 key 限定的命名空间中获取状态。
        let state = uix_component_state(&scope, 1, || 0_i32);
        // 读取私有状态以登记当前动态容器的结构性协调依赖。
        let _ = state.get();
        // 记录本轮捕获到的状态句柄。
        *renderer_states
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(state);
        // 克隆依赖以让 Effect 保持独立生命周期。
        let effect_dependency = effect_dependency.clone();
        // 克隆计数器以让 Effect 长期记录调度。
        let effect_runs = Arc::clone(&renderer_effect_runs);
        // 创建必须随动态容器释放或替换的 Effect。
        let _ = Effect::new(move || {
            // 读取依赖以建立树级调度关系。
            let _ = effect_dependency.get();
            // 记录首次与后续实际运行。
            effect_runs.fetch_add(1, Ordering::Relaxed);
        });
        // 读取动画值以让动态捕获移交动画源所有权。
        let _ = animation.value();
        // 建立承载私有 scope 的最小容器节点。
        let node = ViewNode::leaf(Label::new(label)).uix_component_scope(scope, 0);
        // 仅在需要留场测试时为根节点增加非零过渡。
        match leave {
            // 长离场使测试能够观察 pending-removal 阶段。
            Some(leave) => node.leave(leave),
            // 其余场景维持立即移除语义。
            None => node,
        }
    })
}

// 定义在动态发布区触发异常的最小组件。
struct PanicOnBuild;

// 为异常测试提供运行时组件最低契约。
impl WidgetComponent for PanicOnBuild {
    // 返回当前组件的只读类型擦除引用。
    fn as_any(&self) -> &dyn Any {
        // 当前测试组件无需额外状态。
        self
    }

    // 返回当前组件的可变类型擦除引用。
    fn as_any_mut(&mut self) -> &mut dyn Any {
        // 当前测试组件无需额外状态。
        self
    }

    // 把装箱组件所有权交给运行时。
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        // 直接上转型当前组件。
        self
    }

    // 声明异常组件没有布局、绘制或事件能力。
    fn capabilities(&self) -> WidgetCapabilities {
        // 返回空能力集合。
        WidgetCapabilities::new()
    }

    // 在已进入不可逆发布区后触发稳定异常。
    fn build(&self) -> Vec<Box<dyn WidgetComponent>> {
        // 用稳定消息区分预发布 factory panic。
        panic!("Anchor 容器发布阶段 build 异常")
    }

    // 返回保守快照以满足协调契约。
    fn snapshot_fields(&self) -> SnapshotFields {
        // 测试组件没有产品字段。
        SnapshotFields::Unknown
    }
}

// 验证首次建树通过真实 Anchor owner 接纳 State、Effect、动画与 receipt。
#[test]
// 重复刷新不得重建已物化的固定动态容器。
fn anchor_container_dynamic_capture_materializes_complete_outputs() {
    // 建立工厂调用记录。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 建立私有状态回传槽位。
    let states = Arc::new(Mutex::new(None));
    // 建立可独立触发的 Effect 依赖。
    let effect_dependency = State::new(0_i32);
    // 建立 Effect 调度记录。
    let effect_runs = Arc::new(AtomicUsize::new(0));
    // 建立必须归动态容器拥有的动画源。
    let animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 通过真实建树入口挂载启用 container 的 Anchor。
    let mut tree = ViewAdapter::build(ViewNode::leaf(captured_anchor(
        "initial",
        Arc::clone(&factory_calls),
        Arc::clone(&states),
        effect_dependency.clone(),
        Arc::clone(&effect_runs),
        animation,
        None,
    )));
    // 保存稳定 owner 身份。
    let root = anchor_root(&tree);
    // 初建必须恰好执行一次动态容器工厂。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 1);
    // 初建必须实际发布一个带固定 key 的动态容器。
    let child = container_child(&tree, root);
    // 动态容器必须没有取代 Anchor 根身份。
    assert_eq!(tree.root_id(), Some(root));
    // receipt 成功提交后状态从产品初值开始。
    assert_eq!(captured_state(&states).get(), 0);
    // 首次提交必须将动画源登记到树级所有权。
    assert_eq!(tree.animated_source_registrations().len(), 1);
    // 首次提交必须运行动态容器创建的 Effect。
    assert_eq!(effect_runs.load(Ordering::Relaxed), 1);
    // 写入状态以确认 receipt 的结构性绑定确实归当前树所有。
    captured_state(&states).set(7);
    // 状态更新必须请求 owner 树协调。
    assert!(tree.take_reconcile_requested());
    // 对已物化容器的直接刷新不得再次调用工厂。
    assert!(!tree.refresh_anchor_container_component(root));
    // 空刷新必须保持初建的一次调用，不能创建后丢弃临时动态输出。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 1);
    // 固定动态子节点身份必须保持不变。
    assert_eq!(container_child(&tree, root), child);
    // 已提交 State 不得因空刷新重置。
    assert_eq!(captured_state(&states).get(), 7);
    // 修改 Effect 依赖以确认订阅已移交给动态容器。
    effect_dependency.set(1);
    // 树必须发现该动态容器拥有的待执行 Effect。
    assert!(tree.tick_effects());
    // Effect 必须恰好响应一次依赖变化。
    assert_eq!(effect_runs.load(Ordering::Relaxed), 2);
}

// 验证初建与父协调都不会把 authored 子节点误当作动态容器删除。
#[test]
// 普通 authored key 必须与 Anchor 固定容器 key 长期共存。
fn anchor_container_dynamic_capture_preserves_authored_children() {
    // 建立首版 factory 调用记录。
    let first_calls = Arc::new(AtomicUsize::new(0));
    // 通过真实 ViewNode children 构造一个 authored 子节点。
    let mut tree = ViewAdapter::build(ViewNode::new(
        // Anchor 只拥有其固定 key 动态容器。
        Anchor::new(Vec::new()).container({
            // 克隆首版记录供 factory 持有。
            let first_calls = Arc::clone(&first_calls);
            // 返回首版动态容器 factory。
            move || {
                // 记录初建物化。
                first_calls.fetch_add(1, Ordering::Relaxed);
                // 返回与 authored child 类型相同但身份不同的节点。
                ViewNode::leaf(Label::new("dynamic-first"))
            }
        }),
        // authored child 由调用方声明，不能被动态容器所有者接管。
        vec![ViewNode::leaf(Label::new("authored")).key("authored-anchor-child")],
    ));
    // 保存 Anchor owner。
    let root = anchor_root(&tree);
    // 保存初建 authored child identity。
    let authored = tree
        .get(root)
        .expect("Anchor owner 必须存在")
        .children()
        .iter()
        .copied()
        .find(|child| tree.get(*child).and_then(|node| node.key()) == Some("authored-anchor-child"))
        .expect("authored Anchor child 必须存在");
    // 保存初建动态容器 identity。
    let dynamic = container_child(&tree, root);
    // 初建不得丢失 authored child。
    assert!(tree.get(authored).is_some());
    // 初建必须同时物化固定动态容器。
    assert!(tree.get(dynamic).is_some());
    // 初建 factory 必须只调用一次。
    assert_eq!(first_calls.load(Ordering::Relaxed), 1);
    // 建立父协调的新 factory 调用记录。
    let next_calls = Arc::new(AtomicUsize::new(0));
    // 用同一 authored child 声明协调同一 Anchor owner。
    ViewAdapter::reconcile(
        &mut tree,
        ViewNode::new(
            // 新 factory 必须参与同一动态容器 identity 的协调。
            Anchor::new(Vec::new()).container({
                // 克隆新版记录供 factory 持有。
                let next_calls = Arc::clone(&next_calls);
                // 返回新版动态容器 factory。
                move || {
                    // 记录父协调对最新版 factory 的调用。
                    next_calls.fetch_add(1, Ordering::Relaxed);
                    // 返回新版显示节点。
                    ViewNode::leaf(Label::new("dynamic-next"))
                }
            }),
            // 保持调用方的 authored child 声明。
            vec![ViewNode::leaf(Label::new("authored")).key("authored-anchor-child")],
        ),
    );
    // 父协调不得删除或替换 authored child。
    assert_eq!(
        tree.get(root)
            .expect("Anchor owner 必须存在")
            .children()
            .iter()
            .copied()
            .find(|child| {
                tree.get(*child).and_then(|node| node.key()) == Some("authored-anchor-child")
            }),
        Some(authored)
    );
    // 父协调也必须保留固定动态容器 identity。
    assert_eq!(container_child(&tree, root), dynamic);
    // 新 factory 必须只执行一次。
    assert_eq!(next_calls.load(Ordering::Relaxed), 1);
}

// 验证同一 Anchor owner 的父协调采用最新 factory 输出而不更换动态实例身份。
#[test]
// 旧 Effect 与动画必须被新版输出替换，State 必须继续复用。
fn anchor_container_dynamic_capture_parent_reconcile_reuses_id_and_state() {
    // 建立首版状态记录。
    let first_states = Arc::new(Mutex::new(None));
    // 建立首版 Effect 依赖。
    let first_dependency = State::new(0_i32);
    // 建立首版 Effect 记录。
    let first_runs = Arc::new(AtomicUsize::new(0));
    // 建立首版动画源。
    let first_animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 建立首版工厂调用记录。
    let first_calls = Arc::new(AtomicUsize::new(0));
    // 挂载首版 Anchor。
    let mut tree = ViewAdapter::build(ViewNode::leaf(captured_anchor(
        "first",
        Arc::clone(&first_calls),
        Arc::clone(&first_states),
        first_dependency.clone(),
        Arc::clone(&first_runs),
        first_animation,
        None,
    )));
    // 保存稳定 owner 与容器身份。
    let root = anchor_root(&tree);
    // 保存固定动态容器 identity。
    let child = container_child(&tree, root);
    // 写入必须跨新版 factory 输出保留的状态。
    captured_state(&first_states).set(23);
    // 保存首版动画工作身份。
    let first_animation_id = tree.animated_source_registrations()[0].0;
    // 建立新版状态记录。
    let next_states = Arc::new(Mutex::new(None));
    // 建立新版 Effect 依赖。
    let next_dependency = State::new(0_i32);
    // 建立新版 Effect 记录。
    let next_runs = Arc::new(AtomicUsize::new(0));
    // 建立新版动画源。
    let next_animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 建立新版工厂调用记录。
    let next_calls = Arc::new(AtomicUsize::new(0));
    // 用新版 factory 原位协调同一 Anchor owner。
    ViewAdapter::reconcile(
        &mut tree,
        ViewNode::leaf(captured_anchor(
            "next",
            Arc::clone(&next_calls),
            Arc::clone(&next_states),
            next_dependency.clone(),
            Arc::clone(&next_runs),
            next_animation,
            None,
        )),
    );
    // 同类型同 owner 协调不得替换 Anchor 根。
    assert_eq!(anchor_root(&tree), root);
    // 固定 key 必须复用原动态容器 ComponentId。
    assert_eq!(container_child(&tree, root), child);
    // 新版 factory 必须真实执行一次。
    assert_eq!(next_calls.load(Ordering::Relaxed), 1);
    // 新版 factory 必须读取同一已提交状态槽。
    assert_eq!(captured_state(&next_states).get(), 23);
    // 新版 Effect 必须完成首次运行。
    assert_eq!(next_runs.load(Ordering::Relaxed), 1);
    // 旧 Effect 依赖不得再由当前动态容器持有。
    first_dependency.set(1);
    // 旧 Effect 替换后不得形成树级待执行工作。
    assert!(!tree.has_pending_effects());
    // 新 Effect 依赖仍必须保持动态容器所有权。
    next_dependency.set(1);
    // 新 Effect 更新必须进入树级待执行集合。
    assert!(tree.tick_effects());
    // 新 Effect 必须恰好响应一次更新。
    assert_eq!(next_runs.load(Ordering::Relaxed), 2);
    // 树级动画表只能保留新版动态容器输出。
    assert_eq!(tree.animated_source_registrations().len(), 1);
    // 新动画源必须替换首版工作身份。
    assert_ne!(
        tree.animated_source_registrations()[0].0,
        first_animation_id
    );
}

// 验证关闭 container、真实 root replacement 与 shutdown 都释放工厂持有资源。
#[test]
// 动态容器不得在 owner 生命周期结束后保留 State、Effect 或闭包资源。
fn anchor_container_dynamic_capture_releases_on_disable_replace_and_shutdown() {
    // 建立关闭 container 场景的资源。
    let disable_resource = Arc::new(());
    // 保存资源弱引用以观察工厂 sidecar 释放。
    let weak_disable_resource = Arc::downgrade(&disable_resource);
    // 克隆资源给首版 container factory。
    let renderer_disable_resource = Arc::clone(&disable_resource);
    // 挂载启用 container 的 Anchor。
    let mut disable_tree = ViewAdapter::build(ViewNode::leaf(Anchor::new(Vec::new()).container(
        move || {
            // 证明当前 factory 实际持有资源。
            let _ = Arc::strong_count(&renderer_disable_resource);
            // 返回最小动态容器。
            ViewNode::leaf(Label::new("disable"))
        },
    )));
    // 保存当前动态容器身份。
    let disable_root = anchor_root(&disable_tree);
    // 确保初建确实物化了动态容器。
    let disable_child = container_child(&disable_tree, disable_root);
    // 只留下 Anchor factory 对资源的持有。
    drop(disable_resource);
    // 关闭前工厂 sidecar 必须仍持有资源。
    assert!(weak_disable_resource.upgrade().is_some());
    // 用未启用 container 的同源 Anchor 进行协调。
    ViewAdapter::reconcile(&mut disable_tree, ViewNode::leaf(Anchor::new(Vec::new())));
    // 禁用 container 必须真实移除旧动态容器。
    assert!(disable_tree.get(disable_child).is_none());
    // 禁用后 factory sidecar 必须立即释放。
    assert!(weak_disable_resource.upgrade().is_none());
    // 建立 root replacement 场景的资源。
    let replace_resource = Arc::new(());
    // 保存替换场景资源弱引用。
    let weak_replace_resource = Arc::downgrade(&replace_resource);
    // 克隆资源给待替换 Anchor factory。
    let renderer_replace_resource = Arc::clone(&replace_resource);
    // 建立待被非 Anchor 根替换的树。
    let mut replace_tree = ViewAdapter::build(ViewNode::leaf(Anchor::new(Vec::new()).container(
        move || {
            // 证明 factory 持有资源。
            let _ = Arc::strong_count(&renderer_replace_resource);
            // 返回最小动态容器。
            ViewNode::leaf(Label::new("replace"))
        },
    )));
    // 丢弃测试侧资源强引用。
    drop(replace_resource);
    // 替换前资源必须存活。
    assert!(weak_replace_resource.upgrade().is_some());
    // 用不同根组件触发真实 root replacement。
    ViewAdapter::reconcile(&mut replace_tree, ViewNode::leaf(Label::new("replacement")));
    // 替换旧 Anchor 后 factory sidecar 必须释放。
    assert!(weak_replace_resource.upgrade().is_none());
    // 建立 shutdown 场景的资源。
    let shutdown_resource = Arc::new(());
    // 保存关闭场景资源弱引用。
    let weak_shutdown_resource = Arc::downgrade(&shutdown_resource);
    // 克隆资源给待关闭 Anchor factory。
    let renderer_shutdown_resource = Arc::clone(&shutdown_resource);
    // 挂载正常运行的 Anchor。
    let mut shutdown_tree = ViewAdapter::build(ViewNode::leaf(Anchor::new(Vec::new()).container(
        move || {
            // 证明 factory 持有资源。
            let _ = Arc::strong_count(&renderer_shutdown_resource);
            // 返回最小动态容器。
            ViewNode::leaf(Label::new("shutdown"))
        },
    )));
    // 丢弃测试侧资源强引用。
    drop(shutdown_resource);
    // shutdown 前 factory sidecar 必须仍持有资源。
    assert!(weak_shutdown_resource.upgrade().is_some());
    // 正常关闭完整 owner 树。
    shutdown_tree.shutdown();
    // shutdown 必须释放 Anchor 及其 container factory。
    assert!(weak_shutdown_resource.upgrade().is_none());
}

// 验证固定 key 容器在非零 leave 期间保留资源并在重入时取消离场。
#[test]
// 重入必须复用同一 ComponentId 与已提交 State。
fn anchor_container_dynamic_capture_leave_reentry_reuses_id_and_state() {
    // 建立状态记录。
    let states = Arc::new(Mutex::new(None));
    // 建立 Effect 依赖。
    let effect_dependency = State::new(0_i32);
    // 建立 Effect 记录。
    let effect_runs = Arc::new(AtomicUsize::new(0));
    // 建立动态容器动画源。
    let animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 建立可观察工厂调用记录。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 挂载带长离场动画的 Anchor。
    let mut tree = ViewAdapter::build(ViewNode::leaf(captured_anchor(
        "leaving",
        Arc::clone(&factory_calls),
        Arc::clone(&states),
        effect_dependency.clone(),
        Arc::clone(&effect_runs),
        animation,
        Some(Transition::fade_out(10.0)),
    )));
    // 保存稳定 owner 与动态容器身份。
    let root = anchor_root(&tree);
    // 保存 fixed-key 容器 identity。
    let child = container_child(&tree, root);
    // 写入重入后必须保留的状态。
    captured_state(&states).set(23);
    // 用禁用 container 的新声明启动容器 leave。
    ViewAdapter::reconcile(&mut tree, ViewNode::leaf(Anchor::new(Vec::new())));
    // 容器在非零 leave 期间必须仍保留为直接子节点。
    assert_eq!(container_child(&tree, root), child);
    // 容器必须进入 pending-removal。
    assert!(tree.get(child).expect("离场容器必须存在").pending_removal());
    // pending leave 必须继续保留动画工作所有权。
    assert_eq!(tree.animated_source_registrations().len(), 1);
    // 重建同固定 key 的 Anchor container 声明。
    ViewAdapter::reconcile(
        &mut tree,
        ViewNode::leaf(captured_anchor(
            "reentered",
            Arc::clone(&factory_calls),
            Arc::clone(&states),
            effect_dependency,
            Arc::clone(&effect_runs),
            Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear),
            Some(Transition::fade_out(10.0)),
        )),
    );
    // 同固定 key 重入不得重新分配动态容器身份。
    assert_eq!(container_child(&tree, root), child);
    // 重入必须取消该节点的 pending-removal。
    assert!(!tree.get(child).expect("重入容器必须存在").pending_removal());
    // 重入不得丢失已提交的私有状态。
    assert_eq!(captured_state(&states).get(), 23);
    // 初建与重入协调各执行一次 factory，不能额外刷新重建。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 2);
}

// 验证 stale、非 Anchor、owner leave 与 fail-stop 都在调用 factory 前拒绝刷新。
#[test]
// 拒绝路径不得重新激活无效 owner 或泄漏半发布动态输出。
fn anchor_container_dynamic_capture_rejects_invalid_owners_and_fail_stop() {
    // 建立非 Anchor 根树。
    let mut non_anchor_tree = ViewAdapter::build(ViewNode::leaf(Label::new("plain")));
    // 读取非 Anchor 根身份。
    let non_anchor_root = non_anchor_tree.root_id().expect("Label 必须拥有根");
    // 非 Anchor owner 必须安全拒绝。
    assert!(!non_anchor_tree.refresh_anchor_container_component(non_anchor_root));
    // 建立 stale owner 工厂调用记录。
    let stale_calls = Arc::new(AtomicUsize::new(0));
    // 挂载可观察的 Anchor。
    let mut stale_tree = ViewAdapter::build(ViewNode::leaf(Anchor::new(Vec::new()).container({
        // 克隆计数器供 factory 持有。
        let stale_calls = Arc::clone(&stale_calls);
        // 返回可重复调用的 factory。
        move || {
            // 记录任何错误的迟到调用。
            stale_calls.fetch_add(1, Ordering::Relaxed);
            // 返回最小容器。
            ViewNode::leaf(Label::new("stale"))
        }
    })));
    // 保存即将失效的 owner identity。
    let stale_root = anchor_root(&stale_tree);
    // 初建只允许 factory 执行一次。
    assert_eq!(stale_calls.load(Ordering::Relaxed), 1);
    // 真实移除 owner 使该 identity 失效。
    stale_tree.remove(stale_root);
    // stale owner 必须拒绝刷新。
    assert!(!stale_tree.refresh_anchor_container_component(stale_root));
    // stale 拒绝不得调用 factory。
    assert_eq!(stale_calls.load(Ordering::Relaxed), 1);
    // 建立 owner leave 场景的工厂调用记录。
    let leave_calls = Arc::new(AtomicUsize::new(0));
    // 建立自身具有长 leave 的 Anchor owner。
    let mut leave_tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        // 克隆计数器供 factory 持有。
        let leave_calls = Arc::clone(&leave_calls);
        // 为 owner 加入非零离场配置。
        ViewNode::leaf(Anchor::new(Vec::new()).container(move || {
            // 记录 factory 调用。
            leave_calls.fetch_add(1, Ordering::Relaxed);
            // 返回最小容器。
            ViewNode::leaf(Label::new("owner-leave"))
        }))
        .leave(Transition::fade_out(10.0))
    }));
    // 保存即将进入 leave 的 owner。
    let leave_root = anchor_root(&leave_tree);
    // 初建必须只调用一次。
    assert_eq!(leave_calls.load(Ordering::Relaxed), 1);
    // 让 Anchor owner 本身进入 pending-removal。
    assert!(leave_tree.start_leave_transition(leave_root));
    // 离场 owner 必须拒绝刷新。
    assert!(!leave_tree.refresh_anchor_container_component(leave_root));
    // 离场拒绝不得调用 factory。
    assert_eq!(leave_calls.load(Ordering::Relaxed), 1);
    // 建立正常 Anchor 以制造可观察的发布后异常。
    let mut fail_tree = ViewAdapter::build(ViewNode::leaf(
        Anchor::new(Vec::new()).container(|| ViewNode::leaf(Label::new("safe"))),
    ));
    // 保存 fail-stop 前的 owner identity。
    let fail_root = anchor_root(&fail_tree);
    // 建立发布失败 factory 调用记录。
    let fail_calls = Arc::new(AtomicUsize::new(0));
    // 捕获父协调期间的发布后 build panic。
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 新 factory 返回会在发布区 build 的异常组件。
        ViewAdapter::reconcile(
            &mut fail_tree,
            ViewNode::leaf(Anchor::new(Vec::new()).container({
                // 克隆计数器供 factory 持有。
                let fail_calls = Arc::clone(&fail_calls);
                // 返回可重复调用的异常 factory。
                move || {
                    // 记录本轮真正进入的 factory。
                    fail_calls.fetch_add(1, Ordering::Relaxed);
                    // 返回发布阶段异常组件。
                    ViewNode::leaf(PanicOnBuild)
                }
            })),
        );
    }));
    // 发布后异常必须向调用方传播。
    assert!(result.is_err());
    // 已进入发布区的异常必须永久 fail-stop 整棵树。
    assert!(fail_tree.is_fail_stopped());
    // 首次失败工厂必须只执行一次。
    assert_eq!(fail_calls.load(Ordering::Relaxed), 1);
    // fail-stop 后刷新必须拒绝。
    assert!(!fail_tree.refresh_anchor_container_component(fail_root));
    // fail-stop 拒绝不得再次调用 factory。
    assert_eq!(fail_calls.load(Ordering::Relaxed), 1);
}

// 验证初建 authored 子节点不能占用 Anchor 动态容器的框架保留 key。
#[test]
// 冲突必须在执行容器工厂前被拒绝，避免用户节点接管动态状态身份。
fn anchor_container_dynamic_capture_rejects_authored_reserved_key_on_build() {
    // 建立用于证明冲突路径没有执行应用工厂的计数器。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 克隆计数器供容器工厂持有。
    let renderer_calls = Arc::clone(&factory_calls);
    // 捕获初建声明占用框架保留 key 的稳定诊断。
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 构造同时声明动态容器与冲突 authored 子节点的 Anchor 根。
        let root = ViewNode::new(
            // 容器工厂若被错误执行会留下可观察调用次数。
            Anchor::new(Vec::new()).container(move || {
                // 记录任何越过 authored key 预检的错误执行。
                renderer_calls.fetch_add(1, Ordering::Relaxed);
                // 返回最小容器声明。
                ViewNode::leaf(Label::new("container"))
            }),
            // authored 子节点故意占用框架固定动态身份。
            vec![ViewNode::leaf(Label::new("conflict")).key(Anchor::CONTAINER_CHILD_KEY)],
        );
        // 真实建树入口必须拒绝冲突声明。
        let _ = ViewAdapter::build(root);
    }));
    // 冲突 key 必须被明确拒绝。
    assert!(result.is_err());
    // 预检必须发生在任何容器工厂调用之前。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 0);
}

// 验证 factory 自设根 key 在发布前被拒绝，不能覆盖产品固定动态身份。
#[test]
// 拒绝后既有容器必须仍可服务且树保持可重试的 Operational 状态。
fn anchor_container_dynamic_capture_rejects_factory_owned_root_key() {
    // 挂载未启用 container 的 Anchor，确保 factory 只由直接刷新触发。
    let mut tree = ViewAdapter::build(ViewNode::leaf(Anchor::new(Vec::new())));
    // 保存稳定 owner。
    let root = anchor_root(&tree);
    // 建立非法 factory 调用记录。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 仅替换 live Anchor factory，不触发父协调的发布边界。
    sync_runtime_anchor(
        &mut tree,
        root,
        Anchor::new(Vec::new()).container({
            // 克隆计数器供 factory 持有。
            let factory_calls = Arc::clone(&factory_calls);
            // 返回非法 root key factory。
            move || {
                // 记录 factory 确实只进入一次。
                factory_calls.fetch_add(1, Ordering::Relaxed);
                // 用户根 key 不得覆盖 Anchor 固定容器 key。
                ViewNode::leaf(Label::new("invalid")).key("factory-owned-key")
            }
        }),
    );
    // 捕获 factory root key 违反固定动态身份约束的异常。
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 直接刷新让 key 校验发生在动态事务发布前。
        let _ = tree.refresh_anchor_container_component(root);
    }));
    // 非法 key 必须被明确拒绝。
    assert!(result.is_err());
    // key 检查发生在不可逆发布前，树不得 fail-stop。
    assert!(!tree.is_fail_stopped());
    // 预发布拒绝后树必须仍接受后续外部工作。
    assert!(tree.accepts_external_work());
    // 非法 factory 只能执行一次。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 1);
    // 非法 key 不得留下半发布动态容器。
    assert!(tree
        .get(root)
        .expect("Anchor owner 必须存在")
        .children()
        .is_empty());
    // 安装安全 factory 证明同一 owner 仍可重试。
    sync_runtime_anchor(
        &mut tree,
        root,
        Anchor::new(Vec::new()).container(|| ViewNode::leaf(Label::new("recovered"))),
    );
    // 安全 factory 必须能直接完成首次发布。
    assert!(tree.refresh_anchor_container_component(root));
    // 恢复后必须拥有唯一固定动态容器。
    let _ = container_child(&tree, root);
}

// 验证直接 factory panic 回滚临时捕获，随后可用同一 owner 成功重试。
#[test]
// 验证父协调发布后 panic 则进入 fail-stop，二者不得混淆。
fn anchor_container_dynamic_capture_pre_publish_panic_rolls_back_and_retries() {
    // 先挂载未启用 container 的 Anchor，避免初建立即调用失败 factory。
    let mut tree = ViewAdapter::build(ViewNode::leaf(Anchor::new(Vec::new())));
    // 保存可重试的稳定 owner。
    let root = anchor_root(&tree);
    // 建立失败 factory 调用记录。
    let failed_calls = Arc::new(AtomicUsize::new(0));
    // 建立 provisional State 回传记录。
    let failed_states = Arc::new(Mutex::new(None));
    // 仅安装会直接 panic 的 factory，不触发父协调发布。
    sync_runtime_anchor(
        &mut tree,
        root,
        Anchor::new(Vec::new()).container({
            // 克隆调用记录供 factory 持有。
            let failed_calls = Arc::clone(&failed_calls);
            // 克隆状态记录供 factory 回传 provisional 槽。
            let failed_states = Arc::clone(&failed_states);
            // 返回发布前异常 factory。
            move || -> ViewNode {
                // 记录 factory 确实进入捕获边界。
                failed_calls.fetch_add(1, Ordering::Relaxed);
                // 声明与恢复工厂相同的私有 scope。
                let scope = uix_component_scope("anchor-container-pre-publish-panic-test", 1);
                // 创建仍未提交的 provisional State。
                let state = uix_component_state(&scope, 1, || 0_i32);
                // 写入可辨识值以检查恢复不会复用错误 journal。
                state.set(23);
                // 回传 provisional 句柄供异常后诊断。
                *failed_states
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(state);
                // 在返回声明节点前触发异常。
                panic!("Anchor 容器工厂发布前异常")
            }
        }),
    );
    // 捕获 factory 返回 ViewNode 前的异常。
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 直接刷新让 panic 发生在 append 发布前。
        let _ = tree.refresh_anchor_container_component(root);
    }));
    // factory panic 必须向调用方传播。
    assert!(result.is_err());
    // 尚未发布结构的异常不得 fail-stop 树。
    assert!(!tree.is_fail_stopped());
    // 预发布异常回滚后树必须仍接受后续外部工作。
    assert!(tree.accepts_external_work());
    // 原 Anchor owner 必须仍保持可用。
    assert_eq!(anchor_root(&tree), root);
    // 失败不得留下半发布动态容器。
    assert!(tree
        .get(root)
        .expect("Anchor owner 必须存在")
        .children()
        .is_empty());
    // 失败 factory 必须只执行一次。
    assert_eq!(failed_calls.load(Ordering::Relaxed), 1);
    // provisional 句柄仅保留局部写入，不能代表已提交 owner 状态。
    assert_eq!(captured_state(&failed_states).get(), 23);
    // 建立安全恢复 factory 的状态记录。
    let recovered_states = Arc::new(Mutex::new(None));
    // 建立安全 factory 调用记录。
    let recovered_calls = Arc::new(AtomicUsize::new(0));
    // 用相同 scope 的安全 factory 安装到同一 live owner。
    sync_runtime_anchor(
        &mut tree,
        root,
        Anchor::new(Vec::new()).container({
            // 克隆状态记录供安全 factory 回传。
            let recovered_states = Arc::clone(&recovered_states);
            // 克隆调用记录供安全 factory 递增。
            let recovered_calls = Arc::clone(&recovered_calls);
            // 返回可成功发布的 factory。
            move || {
                // 记录恢复 factory 实际执行。
                recovered_calls.fetch_add(1, Ordering::Relaxed);
                // 使用与失败工厂相同的动态组件 scope。
                let scope = uix_component_scope("anchor-container-pre-publish-panic-test", 1);
                // 正确回滚后必须重新得到初值 State。
                let state = uix_component_state(&scope, 1, || 0_i32);
                // 保存恢复后的状态句柄。
                *recovered_states
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(state);
                // 返回承载同一 scope 的最小容器。
                ViewNode::leaf(Label::new("recovered")).uix_component_scope(scope, 0)
            }
        }),
    );
    // 安全 factory 必须通过直接刷新实际发布固定动态容器。
    assert!(tree.refresh_anchor_container_component(root));
    // 安全 factory 必须恰好执行一次。
    assert_eq!(recovered_calls.load(Ordering::Relaxed), 1);
    // 恢复必须取得新建初值而不是失败 factory 写入的二十三。
    assert_eq!(captured_state(&recovered_states).get(), 0);
}
