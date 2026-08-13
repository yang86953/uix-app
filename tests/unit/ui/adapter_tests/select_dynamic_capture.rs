// 导入父模块公开给适配器回归的声明树协调入口。
use super::ViewAdapter;
// 导入组件私有状态作用域与字段申请入口。
use crate::ui::component_state::{uix_component_scope, uix_component_state};
// 导入读取运行时节点子关系与 key 的核心接口。
use crate::ui::component::widget::WidgetCore;
// 导入动态选项声明节点。
use crate::ui::view::ViewNode;
// 导入真实 Select 与最小选项组件。
use crate::ui::widgets::{Label, Select};
// 导入状态、Effect、动画与运行时树类型。
use crate::ui::{Animated, ComponentId, Easing, Effect, State, WidgetTree};
// 导入按显示文案保存最近状态的映射。
use std::collections::HashMap;
// 导入 renderer 失败开关与调用次数观察器。
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
// 导入跨协调轮次共享观察对象所需的所有权类型。
use std::sync::{Arc, Mutex};

// 定义 option renderer 在每轮捕获后回传的私有状态观察表。
type CapturedStates = Arc<Mutex<HashMap<String, State<i32>>>>;

// 按显示文案从本轮 renderer 观察表读取私有状态句柄。
fn captured_state(
    // 接收跨 renderer 调用共享的状态观察表。
    states: &CapturedStates,
    // 接收需要读取的选项显示文案。
    label: &str,
    // 返回该选项本轮捕获到的私有状态。
) -> State<i32> {
    // 锁定观察表仅覆盖状态句柄克隆。
    states
        // 进入共享状态记录。
        .lock()
        // 即使先前断言 panic 也恢复记录以保留精确诊断。
        .unwrap_or_else(|error| error.into_inner())
        // 按显示文案读取最近一次工厂交接的状态。
        .get(label)
        // 在脱离锁后保留公开状态句柄。
        .cloned()
        // 已物化选项必须已经执行真实 renderer。
        .expect("已物化 Select 选项必须回传组件私有状态")
}

// 在 Select 直接子节点中按绝对选项索引查找动态身份。
fn option_id(
    // 接收真实运行时树。
    tree: &WidgetTree,
    // 接收仍在运行的 Select owner。
    select: ComponentId,
    // 接收需要查找的绝对选项索引。
    index: usize,
    // 返回该动态选项当前的 ComponentId。
) -> ComponentId {
    // 生成与捕获命名空间一致的运行时 key。
    let key = format!("select-option:{index}");
    // 遍历 Select 的实际直接动态子节点。
    tree.get(select)
        // 当前测试阶段 Select owner 必须可寻址。
        .expect("Select owner 必须存在")
        // 读取实际动态子关系。
        .children()
        // 逐个检查稳定组件身份。
        .iter()
        // 复制轻量 ComponentId 以脱离父节点借用。
        .copied()
        // 使用运行时声明 key 进行精确匹配。
        .find(|child| tree.get(*child).and_then(|node| node.key()) == Some(key.as_str()))
        // 测试目标选项必须由 renderer 实际物化。
        .expect("Select 自定义选项必须存在")
}

// 构造带完整 State、Effect 与 AnimatedSource 的真实选项 View。
fn option_view(
    // 接收当前选项显示文案。
    label: &str,
    // 接收工厂累计调用次数观察器。
    factory_calls: &Arc<AtomicUsize>,
    // 接收按显示文案回传 State 的观察表。
    states: &CapturedStates,
    // 接收 Effect 实际运行次数观察器。
    effect_runs: &Arc<AtomicUsize>,
    // 接收可切换的 renderer 失败开关。
    panic_now: &Arc<AtomicBool>,
    // 返回交给 Select renderer 的声明子树。
) -> ViewNode {
    // 记录本轮真实 renderer 调用，供重复协调断言。
    factory_calls.fetch_add(1, Ordering::Relaxed);
    // 在任何状态申请前执行可恢复的捕获阶段失败门禁。
    assert!(
        // 安全路径必须保持失败开关关闭。
        !panic_now.load(Ordering::Relaxed),
        // 使用稳定诊断标识直接捕获异常。
        "Select 选项捕获阶段测试异常"
    );
    // 建立选项私有字段的固定组件 scope。
    let scope = uix_component_scope("select-option-dynamic-capture-test", 1);
    // 在树签发的动态命名空间内取得或初始化本选项私有状态。
    let state = uix_component_state(&scope, 1, || 0_i32);
    // 将本轮状态按显示文案回传给测试。
    states
        // 锁定共享表仅覆盖当前选项的句柄写入。
        .lock()
        // 即使已有断言 panic 也恢复表以继续呈现精确失败。
        .unwrap_or_else(|error| error.into_inner())
        // 用当前文案覆盖本轮最新句柄。
        .insert(label.to_owned(), state.clone());
    // 在声明捕获边界读取私有状态，使节点接管可取消的协调租约。
    let _ = state.get();
    // 克隆本选项状态给 Effect，确保写入可调度节点拥有的副作用。
    let effect_state = state.clone();
    // 克隆 Effect 运行记录给长期闭包。
    let effect_runs = Arc::clone(effect_runs);
    // 建立归属选项节点的响应式 Effect。
    let _effect = Effect::new(move || {
        // 读取私有状态以建立该节点的实际依赖。
        let _ = effect_state.get();
        // 记录 Effect 首次与后续运行次数。
        effect_runs.fetch_add(1, Ordering::Relaxed);
    });
    // 建立需要由选项节点接管的动画源。
    let animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 在动态捕获边界内读取值，使动画来源进入节点所有权。
    let _ = animation.value();
    // 构造声明 scope 与本选项可识别文本的最小根。
    ViewNode::leaf(Label::new(label.to_owned()))
        // 让运行时节点在释放时归还该选项私有状态租约。
        .uix_component_scope(scope, 0)
}

// 构造真实 Select View，并按参数决定是否物化下拉选项。
fn select_view(
    // 接收是否打开下拉层。
    open: bool,
    // 接收 renderer 调用次数观察器。
    factory_calls: Arc<AtomicUsize>,
    // 接收私有状态观察表。
    states: CapturedStates,
    // 接收 Effect 观察器。
    effect_runs: Arc<AtomicUsize>,
    // 接收 renderer 失败开关。
    panic_now: Arc<AtomicBool>,
    // 返回可交给 ViewAdapter 的真实 Select 声明。
) -> ViewNode {
    // 建立两个具有稳定绝对索引的选项。
    let mut select = Select::new().options(["甲", "乙"]);
    // 只有打开状态才允许自定义选项进入物化范围。
    if open {
        // 使用真实公开入口打开 Select 下拉层。
        select.open();
    }
    // 捕获 Select 声明本身并登记真实 option renderer sidecar。
    ViewAdapter::capture_view(select.render_option(move |label| {
        // 构造携带完整动态运行时输出的选项 View。
        option_view(
            // 交付当前显示文案。
            label,
            // 交付共享调用记录。
            &factory_calls,
            // 交付共享状态观察表。
            &states,
            // 交付共享 Effect 观察器。
            &effect_runs,
            // 交付共享失败开关。
            &panic_now,
        )
    }))
}

// 验证每个可见选项完整接管运行时输出，普通父协调保留独立状态身份。
#[test]
// 执行双选项隔离、重复协调与 Effect 调度回归。
fn select_option_dynamic_capture_preserves_independent_state_across_reconcile() {
    // 建立真实 renderer 调用次数记录。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 建立本轮私有 State 观察表。
    let states = Arc::new(Mutex::new(HashMap::new()));
    // 建立节点 Effect 运行次数记录。
    let effect_runs = Arc::new(AtomicUsize::new(0));
    // 建立默认关闭的 renderer 失败开关。
    let panic_now = Arc::new(AtomicBool::new(false));
    // 用真实打开的 Select View 完成首次建树。
    let mut tree = ViewAdapter::build_nodes(select_view(
        // 首轮打开下拉层以物化两个选项。
        true,
        // 交付调用记录。
        Arc::clone(&factory_calls),
        // 交付状态观察表。
        Arc::clone(&states),
        // 交付 Effect 观察器。
        Arc::clone(&effect_runs),
        // 交付安全失败开关。
        Arc::clone(&panic_now),
    ));
    // 读取唯一 Select owner。
    let select = tree.root_id().expect("Select 必须拥有根节点");
    // 两个可见选项必须各执行一次真实 renderer。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 2);
    // 两个动态选项必须各自登记一个节点动画源。
    assert_eq!(tree.animated_source_registrations().len(), 2);
    // 保存两个选项首次物化的真实节点身份。
    let first_a = option_id(&tree, select, 0);
    // 保存第二个选项首次物化的真实节点身份。
    let first_b = option_id(&tree, select, 1);
    // 为首个选项写入非初值状态。
    captured_state(&states, "甲").set(17);
    // 次选项必须保持自己的初始值。
    assert_eq!(captured_state(&states, "乙").get(), 0);
    // 活跃选项私有 State 必须请求所属树执行协调。
    assert!(tree.take_reconcile_requested());
    // 只有首个选项 Effect 应进入待执行状态。
    assert!(tree.tick_effects());

    // 以等价声明重新协调同一 Select owner。
    ViewAdapter::reconcile_nodes(
        // 在同一真实树内执行父声明刷新。
        &mut tree,
        // 重新捕获等价的打开 Select 声明。
        select_view(
            // 保持下拉层打开。
            true,
            // 使用同一调用记录。
            Arc::clone(&factory_calls),
            // 使用同一状态观察表。
            Arc::clone(&states),
            // 使用同一 Effect 观察器。
            Arc::clone(&effect_runs),
            // 保持安全 renderer。
            Arc::clone(&panic_now),
        ),
    );
    // 同一绝对索引必须保留首项非初值状态。
    assert_eq!(captured_state(&states, "甲").get(), 17);
    // 次项状态必须继续独立且不接收首项数值。
    assert_eq!(captured_state(&states, "乙").get(), 0);
    // 两个稳定绝对索引必须分别复用原有运行时节点。
    assert_eq!(option_id(&tree, select, 0), first_a);
    // 第二个稳定绝对索引也必须复用原节点。
    assert_eq!(option_id(&tree, select, 1), first_b);
    // 父刷新必须重新执行两个声明 renderer 以交接最新输出。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 4);
    // 动画注册必须仍恰好属于两个活动选项。
    assert_eq!(tree.animated_source_registrations().len(), 2);
    // 再次写入首项状态以验证新版 Effect 与协调租约已接管。
    captured_state(&states, "甲").set(18);
    // 新版 State 租约仍必须指向同一所属树。
    assert!(tree.take_reconcile_requested());
    // 新版 Effect 必须可由树级调度执行。
    assert!(tree.tick_effects());
}

// 验证关闭、重新打开与换根会释放旧选项资源并重新初始化。
#[test]
// 执行真实移除、重新物化、跨树隔离与 shutdown 回归。
fn select_option_dynamic_capture_releases_removed_and_cross_tree_state() {
    // 建立第一棵树的 renderer 观察对象。
    let calls_a = Arc::new(AtomicUsize::new(0));
    // 建立第一棵树的私有状态表。
    let states_a = Arc::new(Mutex::new(HashMap::new()));
    // 建立第一棵树的 Effect 观察器。
    let effects_a = Arc::new(AtomicUsize::new(0));
    // 建立第一棵树的安全失败开关。
    let panic_a = Arc::new(AtomicBool::new(false));
    // 首次物化第一棵 Select 树。
    let mut tree_a = ViewAdapter::build_nodes(select_view(
        // 打开第一棵树的下拉层。
        true,
        // 交付第一棵树调用记录。
        Arc::clone(&calls_a),
        // 交付第一棵树状态表。
        Arc::clone(&states_a),
        // 交付第一棵树 Effect 观察器。
        Arc::clone(&effects_a),
        // 交付第一棵树失败开关。
        Arc::clone(&panic_a),
    ));
    // 保存第一棵树的 Select owner。
    let select_a = tree_a.root_id().expect("第一棵 Select 必须存在");
    // 保存关闭前首个选项节点身份。
    let removed = option_id(&tree_a, select_a, 0);
    // 保存关闭后仍可用于验证租约释放的旧状态句柄。
    let removed_state = captured_state(&states_a, "甲");
    // 写入非初值以排除重新物化假阳性。
    removed_state.set(31);
    // 清除本轮状态写入产生的树级协调请求。
    assert!(tree_a.take_reconcile_requested());
    // 清除本轮状态写入产生的 Effect 调度。
    assert!(tree_a.tick_effects());

    // 协调为关闭状态，触发全部自定义选项真实移除。
    ViewAdapter::reconcile_nodes(
        // 在第一棵树内执行关闭声明。
        &mut tree_a,
        // 交付使用同一 renderer 观察器的关闭 Select。
        select_view(
            // 关闭下拉层使物化索引集合为空。
            false,
            // 复用第一棵树调用记录。
            Arc::clone(&calls_a),
            // 复用第一棵树状态表。
            Arc::clone(&states_a),
            // 复用第一棵树 Effect 观察器。
            Arc::clone(&effects_a),
            // 保持安全 renderer。
            Arc::clone(&panic_a),
        ),
    );
    // 已关闭的旧选项不得继续存在。
    assert!(tree_a.get(removed).is_none());
    // 全部选项移除后不得遗留动画来源。
    assert!(tree_a.animated_source_registrations().is_empty());
    // 写入旧 State 不得重新请求已经释放的树协调。
    removed_state.set(32);
    // 旧状态租约必须已经同步释放。
    assert!(!tree_a.take_reconcile_requested());
    // 旧 Effect 所有权必须已经同步释放。
    assert!(!tree_a.has_pending_effects());

    // 重新打开同一 Select owner，验证真实移除后按初值重建。
    ViewAdapter::reconcile_nodes(
        // 在第一棵树内重新协调。
        &mut tree_a,
        // 交付重新打开的 Select 声明。
        select_view(
            // 再次打开下拉层。
            true,
            // 复用第一棵树调用记录。
            Arc::clone(&calls_a),
            // 复用第一棵树状态表。
            Arc::clone(&states_a),
            // 复用第一棵树 Effect 观察器。
            Arc::clone(&effects_a),
            // 保持安全 renderer。
            Arc::clone(&panic_a),
        ),
    );
    // 已真实释放后重建必须回到初始化值。
    assert_eq!(captured_state(&states_a, "甲").get(), 0);
    // 重建选项不得复用已经真实删除的 ComponentId。
    assert_ne!(option_id(&tree_a, select_a, 0), removed);

    // 建立第二棵窗口树的独立观察对象。
    let states_b = Arc::new(Mutex::new(HashMap::new()));
    // 建立第二棵树并使用相同选项声明。
    let mut tree_b = ViewAdapter::build_nodes(select_view(
        // 同样打开第二棵树的下拉层。
        true,
        // 第二棵树使用独立调用记录。
        Arc::new(AtomicUsize::new(0)),
        // 第二棵树使用独立状态表。
        Arc::clone(&states_b),
        // 第二棵树使用独立 Effect 观察器。
        Arc::new(AtomicUsize::new(0)),
        // 第二棵树使用独立失败开关。
        Arc::new(AtomicBool::new(false)),
    ));
    // 修改第一棵树重新物化后的首项状态。
    captured_state(&states_a, "甲").set(77);
    // 第二棵树相同索引必须保持自己的初值。
    assert_eq!(captured_state(&states_b, "甲").get(), 0);
    // 关闭第一棵树并释放其全部动态资源。
    tree_a.shutdown();
    // 第一棵树关闭后不得遗留动画来源。
    assert!(tree_a.animated_source_registrations().is_empty());
    // 第二棵树必须不受第一棵树关闭影响。
    assert_eq!(captured_state(&states_b, "甲").get(), 0);
    // 受控关闭第二棵树完成独立生命周期。
    tree_b.shutdown();
}

// 验证直接刷新 renderer panic 可回滚，陈旧 owner 不会重新执行应用代码。
#[test]
// 执行捕获阶段异常恢复与 stale generation 准入回归。
fn select_option_dynamic_capture_recovers_pre_publish_panic_and_rejects_stale_owner() {
    // 建立 renderer 调用记录。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 建立私有状态观察表。
    let states = Arc::new(Mutex::new(HashMap::new()));
    // 建立 Effect 观察器。
    let effect_runs = Arc::new(AtomicUsize::new(0));
    // 建立可切换的 renderer 失败开关。
    let panic_now = Arc::new(AtomicBool::new(false));
    // 先用安全 renderer 建立真实 Select 树。
    let mut tree = ViewAdapter::build_nodes(select_view(
        // 打开下拉层以建立现有动态子树。
        true,
        // 交付调用记录。
        Arc::clone(&factory_calls),
        // 交付状态观察表。
        Arc::clone(&states),
        // 交付 Effect 观察器。
        Arc::clone(&effect_runs),
        // 交付可切换失败开关。
        Arc::clone(&panic_now),
    ));
    // 保存后续会变为陈旧 generation 的 Select owner。
    let stale_owner = tree.root_id().expect("Select 必须拥有根节点");
    // 保存 panic 前的首个选项节点身份。
    let stable_child = option_id(&tree, stale_owner, 0);
    // 强制下一次直接刷新重新执行全部选项 renderer。
    tree.invalidate_select_option_component(stale_owner);
    // 只为本次直接捕获打开失败开关。
    panic_now.store(true, Ordering::Relaxed);
    // 捕获在任何动态结构发布前发生的 renderer panic。
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 直接刷新必须先完整捕获，再进入动态协调事务。
        tree.refresh_select_option_component(stale_owner);
    }));
    // 捕获阶段 panic 必须向调用方传播。
    assert!(result.is_err());
    // 发布前失败后旧树必须继续允许外部工作。
    assert!(tree.accepts_external_work());
    // 旧动态选项节点必须保持原身份且没有半发布替换。
    assert_eq!(option_id(&tree, stale_owner, 0), stable_child);
    // 关闭失败开关，使同一刷新可以安全恢复。
    panic_now.store(false, Ordering::Relaxed);
    // 安全重试必须完成捕获，结构是否变化不影响本断言。
    let _ = tree.refresh_select_option_component(stale_owner);
    // 恢复后首项私有状态仍保持可读。
    assert_eq!(captured_state(&states, "甲").get(), 0);

    // 完整换根使原 Select ComponentId generation 失效。
    tree.set_root(Box::new(Label::new("replacement")));
    // 记录 stale 刷新前的 renderer 调用次数。
    let calls_before_stale = factory_calls.load(Ordering::Relaxed);
    // 陈旧 owner 的迟到刷新必须安全拒绝。
    assert!(!tree.refresh_select_option_component(stale_owner));
    // stale 拒绝不得再次进入应用 renderer。
    assert_eq!(factory_calls.load(Ordering::Relaxed), calls_before_stale);
    // 受控关闭替换后的树必须保持可调用。
    tree.shutdown();
}
