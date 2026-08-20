// 复用父模块的私有 Image 与加载状态，直接驱动受控错误分支。
use super::{Image, ImageLoadState};
// 导入错误状态更新所需的最小绘制矩形。
use crate::core::Rect;
// 导入真实声明树建树与原位协调入口。
use crate::ui::adapter::ViewAdapter;
// 导入组件私有状态作用域与状态槽创建入口。
use crate::ui::widget_state::{uix_widget_scope, uix_widget_state};
// 导入读取运行时节点组件与子节点所需的树节点契约。
use crate::ui::widget_runtime::widget::WidgetCore;
// 导入声明错误子树的最小节点类型。
use crate::ui::view::ViewNode;
// 导入错误子树使用的最小可视组件。
use crate::ui::widgets::Label;
// 导入动画、Effect、State、测试组件契约与树级身份观察类型。
use crate::ui::{
    // 导入动态错误子树需要交接的动画源。
    Animated,
    // 导入动画缓动函数。
    Easing,
    // 导入动态错误子树需要交接的 Effect。
    Effect,
    // 导入手写异常组件的保守快照字段。
    SnapshotFields,
    // 导入组件私有状态的公开句柄。
    State,
    // 导入发布阶段异常组件的核心契约。
    Widget,
    // 导入手写异常组件的空能力集合。
    WidgetCapabilities,
    // 导入稳定运行时组件身份。
    WidgetId,
    // 导入唯一拥有动态错误子树的运行时树。
    WidgetTree,
};
// 导入手写异常组件所需的类型擦除接口。
use std::any::Any;
// 导入共享测试观测所需的原子计数。
use std::sync::atomic::{AtomicUsize, Ordering};
// 导入错误工厂跨协调轮次共享的单线程安全记录容器。
use std::sync::{Arc, Mutex};

// 从真实树中读取图片根节点的固定运行时身份。
fn image_root(
    // 接收已经由 Image 建树入口创建的运行时树。
    tree: &WidgetTree,
    // 返回当前唯一根身份。
) -> WidgetId {
    // 图片根必须在测试读取前已经成功挂载。
    tree.root_id().expect("Image 必须拥有运行时根")
}

// 将已挂载 Image 切换为可控错误，避免依赖文件系统或图片解码能力。
fn set_controlled_error(
    // 接收当前运行时树以取得实际 Image owner。
    tree: &WidgetTree,
    // 接收本轮应进入错误视图分支的根身份。
    root: WidgetId,
    // 接收图片服务返回的稳定错误文本，组件状态命名空间固定使用 uix:image:error。
    error: &str,
) {
    // 读取根节点，测试中的 Image owner 必须仍然可寻址。
    let node = tree.get(root).expect("Image 根必须存在");
    // 从运行时类型擦除接口恢复私有 Image 组件。
    let image = node
        // 取得根节点当前组件引用。
        .widget()
        // 进入类型擦除只读接口。
        .as_any()
        // 恢复 Image 的私有加载状态入口。
        .downcast_ref::<Image>()
        // 根组件类型不得在本组回归中变化。
        .expect("根必须是 Image");
    // 直接设置真实错误状态，后续刷新仍走产品动态子树协调入口。
    image.set_load_state(
        // 错误文本同时模拟图片服务的确定性失败原因。
        ImageLoadState::Error(error.to_owned()),
        // 失效请求仍归属同一宿主树。
        tree,
        // 使用有效 frame，避免测试依赖尚未执行的布局阶段。
        Rect::new(0.0, 0.0, 100.0, 100.0),
    );
}

// 将已挂载 Image 清回空载状态，模拟错误子树已不再应存在的产品条件。
fn clear_controlled_error(
    // 接收当前运行时树以取得实际 Image owner。
    tree: &WidgetTree,
    // 接收本轮应取消错误视图分支的根身份。
    root: WidgetId,
) {
    // 读取根节点，测试中的 Image owner 必须仍然可寻址。
    let node = tree.get(root).expect("Image 根必须存在");
    // 从运行时类型擦除接口恢复私有 Image 组件。
    let image = node
        // 取得根节点当前组件引用。
        .widget()
        // 进入类型擦除只读接口。
        .as_any()
        // 恢复 Image 的私有加载状态入口。
        .downcast_ref::<Image>()
        // 根组件类型不得在本组回归中变化。
        .expect("根必须是 Image");
    // 直接清除真实错误状态，后续刷新必须重新初始化动态子树。
    image.set_load_state(
        // 空载状态不再声明错误子树。
        ImageLoadState::Empty,
        // 失效请求仍归属同一宿主树。
        tree,
        // 使用有效 frame，避免测试依赖尚未执行的布局阶段。
        Rect::new(0.0, 0.0, 100.0, 100.0),
    );
}

// 从错误工厂的共享记录取得最近一次捕获到的私有状态句柄。
fn captured_state(
    // 接收错误工厂写入状态句柄的共享槽位。
    states: &Arc<Mutex<Option<State<i32>>>>,
    // 返回可在协调后观察其私有槽身份与值的状态句柄。
) -> State<i32> {
    // 锁定记录只覆盖状态句柄克隆操作。
    states
        // 进入测试专用共享记录。
        .lock()
        // 先前断言 panic 不得妨碍后续诊断。
        .unwrap_or_else(|error| error.into_inner())
        // 取得本轮延迟工厂已经写入的状态句柄。
        .as_ref()
        // 脱离互斥锁后继续使用公开 State 句柄。
        .cloned()
        // 首次动态错误子树物化必须已经执行工厂。
        .expect("错误工厂必须捕获私有 State")
}

// 构造会在延迟错误工厂中捕获 State、Effect 与动画源的 Image 声明。
fn captured_error_image(
    // 接收记录工厂实际执行次数的原子计数。
    factory_calls: Arc<AtomicUsize>,
    // 接收回传组件私有状态句柄的共享槽位。
    states: Arc<Mutex<Option<State<i32>>>>,
    // 接收 Effect 订阅的独立可变依赖。
    effect_dependency: State<i32>,
    // 接收记录 Effect 实际执行次数的原子计数。
    effect_runs: Arc<AtomicUsize>,
    // 接收需要由动态子树所有者注册的动画源。
    animation: Animated<f32>,
    // 返回尚未物化错误子树的 Image 声明。
) -> Image {
    // 声明固定尺寸以使测试不依赖根 bootstrap 布局。
    Image::new(100.0, 100.0)
        // 注册受控错误工厂，实际调用只能发生在运行时 owner 已验证后。
        .on_error(move |_error| -> ViewNode {
            // 每次实际执行都记录，检测重复刷新是否错误重建子树。
            factory_calls.fetch_add(1, Ordering::Relaxed);
            // 克隆 Effect 的依赖，使订阅闭包拥有独立句柄。
            let effect_dependency = effect_dependency.clone();
            // 克隆运行计数器供 Effect 生命周期长期持有。
            let effect_runs = Arc::clone(&effect_runs);
            // 创建读取依赖的 Effect，验证动态捕获会转交完整副作用输出。
            let _ = Effect::new(move || {
                // 读取依赖以建立可随子树移除释放的订阅。
                let _ = effect_dependency.get();
                // 记录首次及后续实际调度次数。
                effect_runs.fetch_add(1, Ordering::Relaxed);
            });
            // 读取动画值以登记本错误子树专属的动画源。
            let _ = animation.value();
            // 为静态错误子组件声明稳定 scope marker。
            let scope = uix_widget_scope("image-dynamic-error-test", 1);
            // 在动态 owner、错误槽位与固定 uix:image:error 命名空间内取得私有状态。
            let state = uix_widget_state(&scope, 1, || 0_i32);
            // 锁定共享槽位只覆盖本轮最新句柄写入。
            *states
                // 进入测试专用状态记录。
                .lock()
                // 先前断言 panic 不得污染本轮测试记录。
                .unwrap_or_else(|error| error.into_inner()) = Some(state);
            // 返回不自设根 key 的错误节点，产品 Image 负责注入固定子节点身份。
            ViewNode::leaf(Label::new("error"))
                // 让已挂载错误子树承载组件私有状态 scope。
                .uix_widget_scope(scope, 0)
        })
}

// 定义在动态子节点真实 build 阶段触发稳定异常的最小组件。
struct PanicOnBuild;

// 为发布后异常回归实现不带布局、绘制或事件能力的组件契约。
impl Widget for PanicOnBuild {
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

    // 把装箱测试组件交给运行时所有者。
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        // 返回当前测试组件所有权。
        self
    }

    // 声明异常组件不提供额外运行时能力。
    fn capabilities(&self) -> WidgetCapabilities {
        // 返回空能力集合。
        WidgetCapabilities::new()
    }

    // 在动态事务已经进入发布区后触发用户组件 build 异常。
    fn build(&self) -> Vec<Box<dyn Widget>> {
        // 使用稳定消息便于失败时定位确切注入点。
        panic!("Image 错误子树发布阶段 build 异常")
    }

    // 提供保守快照以满足运行时协调契约。
    fn snapshot_fields(&self) -> SnapshotFields {
        // 测试组件没有可比较的产品状态。
        SnapshotFields::Unknown
    }
}

// 验证首次受控错误会将延迟工厂的完整输出原子物化到真实 Image 子树。
#[test]
// 同时覆盖重复刷新不重建既有错误子树及其私有资源。
fn image_error_dynamic_capture_materializes_and_reuses_complete_outputs() {
    // 建立错误工厂调用记录。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 建立私有状态句柄记录。
    let states = Arc::new(Mutex::new(None));
    // 建立可单独触发的 Effect 依赖。
    let effect_dependency = State::new(0_i32);
    // 建立 Effect 调度次数记录。
    let effect_runs = Arc::new(AtomicUsize::new(0));
    // 建立将被错误子树捕获的动画源。
    let animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 通过真实建树入口挂载尚未错误的 Image。
    let mut tree = ViewAdapter::build(ViewNode::leaf(captured_error_image(
        // 交给 Image 长期拥有工厂调用记录。
        Arc::clone(&factory_calls),
        // 交给 Image 长期拥有私有状态回传槽位。
        Arc::clone(&states),
        // 交给 Image 错误工厂创建 Effect。
        effect_dependency.clone(),
        // 交给 Image 错误工厂记录 Effect 调度。
        Arc::clone(&effect_runs),
        // 交给 Image 错误工厂登记动画源。
        animation,
    )));
    // 保存稳定 Image owner 身份。
    let root = image_root(&tree);
    // 在同一 Image 实例上进入确定性错误状态。
    set_controlled_error(&tree, root, "controlled error");
    // 首次刷新必须物化受控错误子树。
    assert!(tree.refresh_image_error_widget(root));
    // 错误工厂必须恰好执行一次。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 1);
    // Image 必须拥有唯一错误子节点。
    assert_eq!(
        tree.get(root).expect("Image 根必须存在").children().len(),
        1
    );
    // 错误节点必须使用产品约定的稳定 key。
    assert_eq!(
        tree.get(
            // 读取 Image 当前唯一错误子节点身份。
            tree.get(root).expect("Image 根必须存在").children()[0],
        )
        // 错误子节点必须仍可寻址。
        .expect("错误子节点必须存在")
        // 读取产品注入的协调 key。
        .key(),
        // 错误状态对应固定动态槽位。
        Some("uix:image:error")
    );
    // 捕获的私有状态初值必须已经被动态 owner 接纳。
    assert_eq!(captured_state(&states).get(), 0);
    // 动态错误子树必须注册其捕获动画源。
    assert_eq!(tree.animated_source_registrations().len(), 1);
    // Effect 必须在首次成功发布后执行一次。
    assert_eq!(effect_runs.load(Ordering::Relaxed), 1);
    // 写入私有状态以验证重复刷新不会丢失已提交槽。
    captured_state(&states).set(7);
    // 重复刷新不得重建已经物化的错误子树。
    assert!(!tree.refresh_image_error_widget(root));
    // 工厂调用次数不得因空刷新增加。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 1);
    // 同一私有状态槽必须继续保留已写入值。
    assert_eq!(captured_state(&states).get(), 7);
    // 修改 Effect 依赖以验证该订阅仍归属实际错误子节点。
    effect_dependency.set(1);
    // 树级调度必须发现错误子树的待执行 Effect。
    assert!(tree.tick_effects());
    // Effect 必须恰好响应一次依赖变化。
    assert_eq!(effect_runs.load(Ordering::Relaxed), 2);
}

// 验证同源父级 reconcile 继续复用 Image 错误子树身份与私有状态。
#[test]
// 父声明更新可重建声明输出，但不得更换错误子节点身份或覆盖动态状态槽。
fn image_error_dynamic_capture_survives_same_source_parent_reconcile() {
    // 建立错误工厂调用记录。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 建立私有状态句柄记录。
    let states = Arc::new(Mutex::new(None));
    // 建立 Error 子树 Effect 的依赖。
    let effect_dependency = State::new(0_i32);
    // 建立 Effect 调度记录。
    let effect_runs = Arc::new(AtomicUsize::new(0));
    // 建立 Error 子树动画源。
    let animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 用首版 Image 建树。
    let mut tree = ViewAdapter::build(ViewNode::leaf(captured_error_image(
        // 共享工厂调用记录。
        Arc::clone(&factory_calls),
        // 共享私有状态回传槽位。
        Arc::clone(&states),
        // 共享 Effect 依赖。
        effect_dependency.clone(),
        // 共享 Effect 调度记录。
        Arc::clone(&effect_runs),
        // 首版使用独立动画源。
        animation.clone(),
    )));
    // 读取 Image 的稳定 owner。
    let root = image_root(&tree);
    // 进入确定性错误状态。
    set_controlled_error(&tree, root, "controlled error");
    // 物化唯一错误子树。
    assert!(tree.refresh_image_error_widget(root));
    // 保存首次错误子树的运行时身份。
    let child = tree.get(root).expect("Image 根必须存在").children()[0];
    // 在私有状态槽中写入可区分值。
    captured_state(&states).set(11);
    // 用同一逻辑来源的新 Image 声明原位协调父组件。
    ViewAdapter::reconcile(
        // 协调既有运行时树。
        &mut tree,
        // 新版错误工厂保持相同动态组件 scope 与产品语义。
        ViewNode::leaf(captured_error_image(
            // 共享工厂记录以检测错误重建。
            Arc::clone(&factory_calls),
            // 共享状态记录以观察槽复用。
            Arc::clone(&states),
            // 共享 Effect 依赖。
            effect_dependency,
            // 共享 Effect 调度记录。
            Arc::clone(&effect_runs),
            // 新版声明创建等价动画源。
            animation,
        )),
    );
    // 根组件应继续使用原运行时 owner。
    assert_eq!(image_root(&tree), root);
    // 已物化错误子树必须继续使用原 WidgetId。
    assert_eq!(
        tree.get(root).expect("Image 根必须存在").children(),
        &[child]
    );
    // 父 reconcile 必须重新捕获声明输出以参与事务，但不得分配新状态实例。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 2);
    // 动态私有状态必须跨同源父级协调继续保留。
    assert_eq!(captured_state(&states).get(), 11);
}

// 验证同一 Image handler 从 Error 切回 Empty 时真实移除动态错误子树。
#[test]
// 再次进入 Error 必须重建初值状态且不复用已释放的动态实例。
fn image_error_dynamic_capture_clear_and_rebuild_resets_private_state() {
    // 建立错误工厂调用记录。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 建立私有状态记录但不在释放断言中持有状态句柄。
    let states = Arc::new(Mutex::new(None));
    // 建立 Effect 依赖。
    let effect_dependency = State::new(0_i32);
    // 建立 Effect 调度记录。
    let effect_runs = Arc::new(AtomicUsize::new(0));
    // 建立错误子树动画源。
    let animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 用带 custom 错误工厂的 Image 建树。
    let mut tree = ViewAdapter::build(ViewNode::leaf(captured_error_image(
        // 共享工厂调用记录。
        Arc::clone(&factory_calls),
        // 共享私有状态记录。
        Arc::clone(&states),
        // 共享 Effect 依赖。
        effect_dependency.clone(),
        // 共享 Effect 调度记录。
        Arc::clone(&effect_runs),
        // 交给首版错误子树捕获的动画源。
        animation,
    )));
    // 读取当前 Image owner。
    let root = image_root(&tree);
    // 进入确定性错误状态。
    set_controlled_error(&tree, root, "controlled error");
    // 首次刷新物化 custom 错误子树。
    assert!(tree.refresh_image_error_widget(root));
    // 错误子树必须已经注册一个 Effect。
    assert_eq!(effect_runs.load(Ordering::Relaxed), 1);
    // 首次错误子树必须已经发布其动画源所有权。
    assert_eq!(tree.animated_source_registrations().len(), 1);
    // 把已提交私有状态写成可区分值，验证真实移除后不会错误复用。
    captured_state(&states).set(23);
    // 移除前当前错误实例必须确实持有二十三。
    assert_eq!(captured_state(&states).get(), 23);
    // 同一 handler 直接离开 Error 状态，触发运行时解除路径。
    clear_controlled_error(&tree, root);
    // 空载刷新必须报告实际移除了已物化错误子树。
    assert!(tree.refresh_image_error_widget(root));
    // 已解除的错误子树必须从实际运行时 children 中移除。
    assert!(
        tree.get(root)
            .expect("Image 根必须存在")
            .children()
            .is_empty()
    );
    // 真实移除必须同步清空错误子树的动画源注册。
    assert!(tree.animated_source_registrations().is_empty());
    // 清除先前状态写入造成的协调请求，隔离移除后 Effect 租约断言。
    let _ = tree.take_reconcile_requested();
    // 修改旧 Effect 依赖。
    effect_dependency.set(1);
    // 被真实移除的 Effect 不得再进入树级调度。
    assert!(!tree.has_pending_effects());
    // 丢弃测试记录持有的旧状态句柄，避免它人为延长已移除实例的观察生命周期。
    *states
        // 锁定记录只覆盖旧句柄释放。
        .lock()
        // 先前断言 panic 不得阻碍释放断言继续执行。
        .unwrap_or_else(|error| error.into_inner()) = None;
    // 同一 handler 再次进入相同受控错误，必须建立新的动态实例。
    set_controlled_error(&tree, root, "controlled error");
    // 清除后再次错误必须重新物化子树。
    assert!(tree.refresh_image_error_widget(root));
    // 错误工厂必须在第二次真实错误时再次执行。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 2);
    // 真实移除后的重新错误必须从私有状态初值开始。
    assert_eq!(captured_state(&states).get(), 0);
    // 第二个错误实例必须重新发布自己的动画源注册。
    assert_eq!(tree.animated_source_registrations().len(), 1);
    // 第二个错误实例会创建并首次运行新的 Effect。
    assert_eq!(effect_runs.load(Ordering::Relaxed), 2);
}

// 验证错误工厂在发布前 panic 会回滚临时状态并重新开放原 Image owner。
#[test]
// 后续安全工厂必须能在同一固定错误子树命名空间中从初值重新物化。
fn image_error_factory_pre_publish_panic_rolls_back_and_recovers_same_owner() {
    // 建立失败与恢复工厂的总调用次数记录。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 保存发布前失败工厂创建的 provisional State 句柄。
    let failed_states = Arc::new(Mutex::new(None));
    // 建立失败工厂会读取但不得发布的动画源。
    let failed_animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 克隆调用记录供失败工厂拥有。
    let panicking_calls = Arc::clone(&factory_calls);
    // 克隆状态记录供失败工厂回传 provisional 句柄。
    let panicking_states = Arc::clone(&failed_states);
    // 构造会在返回声明节点前触发异常的 Image。
    let image = Image::new(100.0, 100.0)
        // 注册只在运行时 owner 已验证后执行的失败工厂。
        .on_error(move |_error| -> ViewNode {
            // 记录失败工厂已经实际进入捕获边界。
            panicking_calls.fetch_add(1, Ordering::Relaxed);
            // 读取动画以验证捕获守卫会在 panic 时丢弃临时输出。
            let _ = failed_animation.value();
            // 为失败与恢复工厂声明同一静态组件 scope。
            let scope = uix_widget_scope("image-error-pre-publish-panic-test", 1);
            // 在固定错误子树命名空间中创建 provisional 私有状态。
            let state = uix_widget_state(&scope, 1, || 0_i32);
            // 写入非初值以区分错误保留与正确回滚。
            state.set(23);
            // 保存失败状态句柄供异常返回后直接观察。
            *panicking_states
                // 锁定共享记录只覆盖本次句柄写入。
                .lock()
                // 测试线程中的锁 poison 继续恢复内部值供诊断。
                .unwrap_or_else(|error| error.into_inner()) = Some(state);
            // 在返回 ViewNode 之前触发异常，因此运行时发布尚未开始。
            panic!("Image 错误工厂发布前异常")
        });
    // 先挂载正常且尚未错误的 Image owner。
    let mut tree = ViewAdapter::build(ViewNode::leaf(image));
    // 保存发布前失败仍必须继续拥有的根身份。
    let root = image_root(&tree);
    // 进入确定性错误状态以触发失败工厂。
    set_controlled_error(&tree, root, "controlled error");
    // 捕获错误工厂向上传播的发布前异常。
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 首次错误刷新会在 capture 内执行失败工厂。
        let _ = tree.refresh_image_error_widget(root);
    }));
    // 工厂异常必须继续传播给调用方。
    assert!(result.is_err());
    // 发布前异常不得把仍完整的树切换为 fail-stop。
    assert!(!tree.is_fail_stopped());
    // 树必须恢复 Operational 并继续接受外部协调。
    assert!(tree.accepts_external_work());
    // 同一 Image owner 必须继续作为公开根存在。
    assert_eq!(tree.root_id(), Some(root));
    // 发布前异常不得留下任何错误子节点。
    assert!(
        tree.get(root)
            .expect("Image 根必须存在")
            .children()
            .is_empty()
    );
    // 异常捕获的动画源不得进入树级注册表。
    assert!(tree.animated_source_registrations().is_empty());
    // 失败工厂必须只执行一次。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 1);
    // provisional State 句柄自身保留其局部写入，便于证明后续取得的是新槽。
    assert_eq!(captured_state(&failed_states).get(), 23);
    // 建立安全工厂回传的新状态句柄记录。
    let recovered_states = Arc::new(Mutex::new(None));
    // 克隆总调用记录供安全工厂继续累加。
    let recovered_calls = Arc::clone(&factory_calls);
    // 克隆恢复状态记录供安全工厂写入。
    let safe_states = Arc::clone(&recovered_states);
    // 用安全工厂原位协调同一 Image owner。
    ViewAdapter::reconcile(
        // 继续使用发布前失败后恢复的树。
        &mut tree,
        // 新声明替换失败工厂但保留 Image 类型与来源身份。
        ViewNode::leaf(Image::new(100.0, 100.0).on_error(move |_error| {
            // 记录安全工厂实际执行。
            recovered_calls.fetch_add(1, Ordering::Relaxed);
            // 使用与失败工厂完全相同的组件 scope。
            let scope = uix_widget_scope("image-error-pre-publish-panic-test", 1);
            // 正确回滚后同一动态命名空间必须重新创建初值状态。
            let state = uix_widget_state(&scope, 1, || 0_i32);
            // 保存恢复工厂取得的状态句柄。
            *safe_states
                // 锁定共享记录只覆盖本次句柄写入。
                .lock()
                // 测试线程中的锁 poison 继续恢复内部值供诊断。
                .unwrap_or_else(|error| error.into_inner()) = Some(state);
            // 返回可成功发布的最小错误子节点。
            ViewNode::leaf(Label::new("recovered"))
                // 让真实节点声明同一私有状态 scope。
                .uix_widget_scope(scope, 0)
        })),
    );
    // 恢复协调必须继续复用原 Image owner。
    assert_eq!(tree.root_id(), Some(root));
    // 安全工厂必须成功物化唯一错误子节点。
    assert_eq!(
        tree.get(root).expect("Image 根必须存在").children().len(),
        1
    );
    // 两个工厂各执行一次。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 2);
    // 安全工厂必须取得新建初值而非失败捕获写入的二十三。
    assert_eq!(captured_state(&recovered_states).get(), 0);
}

// 验证错误子节点进入发布区后的 build panic 会永久停止半完成 WidgetTree。
#[test]
// pending State、Effect 与动画不得发布，后续刷新被拒绝且 shutdown 释放工厂资源。
fn image_error_child_post_publish_build_panic_fail_stops_and_shutdown_releases() {
    // 建立错误工厂调用次数记录。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 建立发布失败工厂回传的 provisional State 记录。
    let states = Arc::new(Mutex::new(None));
    // 建立发布失败 Effect 的依赖。
    let effect_dependency = State::new(0_i32);
    // 建立 Effect 首次与后续运行次数记录。
    let effect_runs = Arc::new(AtomicUsize::new(0));
    // 建立发布失败工厂读取的动画源。
    let animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 建立仅由测试与 Image 错误工厂共享的关闭资源。
    let shutdown_resource = Arc::new(());
    // 保存弱引用以观察 fail-stop teardown 是否释放工厂捕获。
    let weak_shutdown_resource = Arc::downgrade(&shutdown_resource);
    // 克隆调用记录供错误工厂拥有。
    let renderer_calls = Arc::clone(&factory_calls);
    // 克隆状态记录供错误工厂回传 provisional 句柄。
    let renderer_states = Arc::clone(&states);
    // 克隆 Effect 依赖供错误工厂创建订阅。
    let renderer_effect_dependency = effect_dependency.clone();
    // 克隆 Effect 运行记录供错误工厂长期闭包拥有。
    let renderer_effect_runs = Arc::clone(&effect_runs);
    // 克隆关闭资源供 Image 的错误工厂 sidecar 持有。
    let renderer_shutdown_resource = Arc::clone(&shutdown_resource);
    // 构造会返回 PanicOnBuild 动态子节点的 Image。
    let image = Image::new(100.0, 100.0)
        // 注册完整捕获输出后才在节点 build 阶段失败的工厂。
        .on_error(move |_error| {
            // 记录工厂实际进入一次。
            renderer_calls.fetch_add(1, Ordering::Relaxed);
            // 读取强引用计数，确保 sidecar 真实捕获关闭资源。
            let _ = Arc::strong_count(&renderer_shutdown_resource);
            // 为发布失败的错误子组件声明稳定 scope。
            let scope = uix_widget_scope("image-error-post-publish-panic-test", 1);
            // 创建本轮发布候选私有状态。
            let state = uix_widget_state(&scope, 1, || 0_i32);
            // 读取状态以让 provisional bind 进入捕获输出。
            let _ = state.get();
            // 保存状态句柄供失败后行为观察。
            *renderer_states
                // 锁定共享记录只覆盖本次句柄写入。
                .lock()
                // 测试线程中的锁 poison 继续恢复内部值供诊断。
                .unwrap_or_else(|error| error.into_inner()) = Some(state);
            // 克隆依赖供 Effect 闭包长期持有。
            let effect_dependency = renderer_effect_dependency.clone();
            // 克隆运行记录供 Effect 闭包长期持有。
            let effect_runs = Arc::clone(&renderer_effect_runs);
            // 创建会被声明节点携带到发布事务的 Effect。
            let _ = Effect::new(move || {
                // 读取依赖以建立可观察订阅。
                let _ = effect_dependency.get();
                // 记录 Effect 创建时的首次同步运行。
                effect_runs.fetch_add(1, Ordering::Relaxed);
            });
            // 读取动画以建立待交接的动态动画输出。
            let _ = animation.value();
            // 返回会在真实 build_child_node 阶段触发异常的组件。
            ViewNode::leaf(PanicOnBuild)
                // 让发布候选节点承载同一私有状态 scope。
                .uix_widget_scope(scope, 0)
        });
    // 先挂载尚未错误的正常 Image owner。
    let mut tree = ViewAdapter::build(ViewNode::leaf(image));
    // 保存进入发布失败前的真实 owner 身份。
    let root = image_root(&tree);
    // 丢弃测试侧强引用，使弱引用只观察 Image 工厂 sidecar。
    drop(shutdown_resource);
    // 工厂尚由运行时 Image 持有，关闭资源必须仍然存活。
    assert!(weak_shutdown_resource.upgrade().is_some());
    // 进入确定性错误状态以触发发布失败工厂。
    set_controlled_error(&tree, root, "controlled error");
    // 捕获动态子节点真实 build 阶段传播的异常。
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // append_dynamic_child 会在发布事务内调用 PanicOnBuild。
        let _ = tree.refresh_image_error_widget(root);
    }));
    // 发布阶段 build 异常必须继续传播给调用方。
    assert!(result.is_err());
    // 发布已经开始，树必须切换为永久 fail-stop。
    assert!(tree.is_fail_stopped());
    // fail-stop 树不得继续接受外部工作。
    assert!(!tree.accepts_external_work());
    // 公开根接口不得暴露半发布结构。
    assert!(tree.root_id().is_none());
    // 公开节点访问不得泄漏旧 Image owner。
    assert!(tree.get(root).is_none());
    // 错误工厂必须只执行一次。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 1);
    // 发布失败的动画源不得进入可驱动注册表。
    assert!(tree.animated_source_registrations().is_empty());
    // Effect 只允许创建时同步运行一次，不能进入树级 owner。
    assert_eq!(effect_runs.load(Ordering::Relaxed), 1);
    // 修改发布失败 Effect 的旧依赖。
    effect_dependency.set(1);
    // fail-stop 树不得暴露任何 pending Effect。
    assert!(!tree.has_pending_effects());
    // 显式 tick 同样不得执行未发布 Effect。
    assert!(!tree.tick_effects());
    // Effect 运行次数必须保持在创建时的一次。
    assert_eq!(effect_runs.load(Ordering::Relaxed), 1);
    // 后续迟到刷新必须在调用工厂前被拒绝。
    assert!(!tree.refresh_image_error_widget(root));
    // 被拒绝刷新不得再次执行应用工厂。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 1);
    // shutdown 前 Image 工厂 sidecar 仍持有关闭资源。
    assert!(weak_shutdown_resource.upgrade().is_some());
    // 所有者关闭 fail-stop 树并释放半发布与 sidecar 资源。
    tree.shutdown();
    // shutdown 后工厂捕获资源必须完成释放。
    assert!(weak_shutdown_resource.upgrade().is_none());
}

// 验证非 Image owner 与关闭后陈旧 Image owner 都会安全拒绝动态刷新。
#[test]
// 迟到调用不得执行应用错误工厂或重新激活停止树。
fn image_error_dynamic_capture_rejects_non_image_and_shutdown_owners() {
    // 用普通 Label 建立非 Image 根树。
    let mut non_image_tree = ViewAdapter::build(ViewNode::leaf(Label::new("plain")));
    // 读取普通根身份。
    let non_image_root = non_image_tree.root_id().expect("Label 必须拥有根");
    // 非 Image 节点必须安全返回 false。
    assert!(!non_image_tree.refresh_image_error_widget(non_image_root));
    // 建立带错误工厂的独立 Image 树。
    let mut image_tree = ViewAdapter::build(
        // 工厂即使存在也不得在 shutdown 后执行。
        ViewNode::leaf(
            // 把组件声明包装为适配器可消费的 View 根。
            Image::new(100.0, 100.0).on_error(|_| ViewNode::leaf(Label::new("error"))),
        ),
    );
    // 保存关闭前 Image owner 身份作为陈旧调用目标。
    let image_root = image_root(&image_tree);
    // 关闭树使所有旧 WidgetId 与动态 renderer 所有权失效。
    image_tree.shutdown();
    // shutdown 后的陈旧 Image owner 必须安全返回 false。
    assert!(!image_tree.refresh_image_error_widget(image_root));
}
