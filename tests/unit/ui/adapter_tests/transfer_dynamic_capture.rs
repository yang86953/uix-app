// 导入父级适配器测试公开的声明树协调入口。
use super::super::ViewAdapter;
// 导入组件私有状态作用域与字段申请入口。
use crate::ui::component_state::{uix_component_scope, uix_component_state};
// 导入读取运行时节点子关系与 key 的核心接口。
use crate::ui::component::widget::WidgetCore;
// 导入动态条目声明根类型。
use crate::ui::view::ViewNode;
// 导入真实 Transfer、条目与最小文本组件。
use crate::ui::widgets::{Label, Transfer, TransferItem};
// 导入状态、副作用、动画、事件与运行时树类型。
use crate::ui::{
    Animated, ComponentId, Easing, Effect, EventResult, KeyCode, KeyMod, State, SystemEvent,
    WidgetTree,
};
// 导入按业务 key 保存最近状态的映射。
use std::collections::HashMap;
// 导入 renderer 失败开关与调用次数观察器。
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
// 导入跨协调轮次共享观察对象所需的所有权类型。
use std::sync::{Arc, Mutex};

// 定义 item renderer 在每轮捕获后回传的私有状态观察表。
type CapturedStates = Arc<Mutex<HashMap<String, State<i32>>>>;

// 按业务 key 从本轮 renderer 观察表读取私有状态句柄。
fn captured_state(
    // 接收跨 renderer 调用共享的状态观察表。
    states: &CapturedStates,
    // 接收需要读取的 TransferItem 业务 key。
    key: &str,
    // 返回该条目最近一次捕获到的私有状态。
) -> State<i32> {
    // 锁定观察表仅覆盖状态句柄克隆。
    states
        // 进入共享状态记录。
        .lock()
        // 即使先前断言 panic 也恢复记录以保留精确诊断。
        .unwrap_or_else(|error| error.into_inner())
        // 按业务 key 读取最近一次工厂交接的状态。
        .get(key)
        // 在脱离锁后保留公开状态句柄。
        .cloned()
        // 已物化条目必须已经执行真实 renderer。
        .expect("已物化 Transfer 条目必须回传组件私有状态")
}

// 在 Transfer 直接子节点中按 pane 与业务 key 查找动态身份。
fn item_id(
    // 接收真实运行时树。
    tree: &WidgetTree,
    // 接收仍在运行的 Transfer owner。
    transfer: ComponentId,
    // 接收框架身份中的 pane 名称。
    pane: &str,
    // 接收需要查找的条目业务 key。
    key: &str,
    // 返回该动态条目当前的 ComponentId。
) -> ComponentId {
    // 生成与捕获命名空间一致的运行时结构 key。
    let runtime_key = format!("transfer:{pane}:{key}");
    // 遍历 Transfer 的实际直接动态子节点。
    tree.get(transfer)
        // 当前测试阶段 Transfer owner 必须可寻址。
        .expect("Transfer owner 必须存在")
        // 读取实际动态子关系。
        .children()
        // 逐个检查稳定组件身份。
        .iter()
        // 复制轻量 ComponentId 以脱离父节点借用。
        .copied()
        // 使用运行时声明 key 进行精确匹配。
        .find(|child| tree.get(*child).and_then(|node| node.key()) == Some(runtime_key.as_str()))
        // 测试目标条目必须由 renderer 实际物化。
        .expect("Transfer 自定义条目必须存在")
}

// 构造带完整 State、Effect 与 AnimatedSource 的真实条目 View。
fn item_view(
    // 接收当前条目快照。
    item: &TransferItem,
    // 接收工厂累计调用次数观察器。
    factory_calls: &Arc<AtomicUsize>,
    // 接收按业务 key 回传 State 的观察表。
    states: &CapturedStates,
    // 接收 Effect 实际运行次数观察器。
    effect_runs: &Arc<AtomicUsize>,
    // 接收可切换的 renderer 失败开关。
    panic_now: &Arc<AtomicBool>,
    // 返回交给 Transfer renderer 的声明子树。
) -> ViewNode {
    // 记录本轮真实 renderer 调用，供重复协调断言。
    factory_calls.fetch_add(1, Ordering::Relaxed);
    // 在任何状态申请前执行可恢复的捕获阶段失败门禁。
    assert!(
        // 安全路径必须保持失败开关关闭。
        !panic_now.load(Ordering::Relaxed),
        // 使用稳定诊断标识直接捕获异常。
        "Transfer 条目捕获阶段测试异常"
    );
    // 建立条目私有字段的固定组件 scope。
    let scope = uix_component_scope("transfer-item-dynamic-capture-test", 1);
    // 在树签发的动态命名空间内取得或初始化本条目私有状态。
    let state = uix_component_state(&scope, 1, || 0_i32);
    // 将本轮状态按业务 key 回传给测试。
    states
        // 锁定共享表仅覆盖当前条目的句柄写入。
        .lock()
        // 即使已有断言 panic 也恢复表以继续呈现精确失败。
        .unwrap_or_else(|error| error.into_inner())
        // 用业务 key 覆盖本轮最新句柄。
        .insert(item.key.clone(), state.clone());
    // 在声明捕获边界读取私有状态，使节点接管可取消的协调租约。
    let _ = state.get();
    // 克隆本条目状态给 Effect，确保写入可调度节点拥有的副作用。
    let effect_state = state.clone();
    // 克隆 Effect 运行记录给长期闭包。
    let effect_runs = Arc::clone(effect_runs);
    // 建立归属条目节点的响应式 Effect。
    let _effect = Effect::new(move || {
        // 读取私有状态以建立该节点的实际依赖。
        let _ = effect_state.get();
        // 记录 Effect 首次与后续运行次数。
        effect_runs.fetch_add(1, Ordering::Relaxed);
    });
    // 建立需要由条目节点接管的动画源。
    let animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 在动态捕获边界内读取值，使动画来源进入节点所有权。
    let _ = animation.value();
    // 构造声明 scope 与当前条目标题的最小根。
    ViewNode::leaf(Label::new(item.title.clone()))
        // 让运行时节点在释放时归还该条目私有状态租约。
        .uix_component_scope(scope, 0)
}

// 构造真实 Transfer View，并按参数决定首项选择与 renderer 配置。
fn transfer_view(
    // 接收源列表首项是否预先选中。
    first_selected: bool,
    // 接收是否安装自定义条目 renderer。
    with_renderer: bool,
    // 接收 renderer 调用次数观察器。
    factory_calls: Arc<AtomicUsize>,
    // 接收私有状态观察表。
    states: CapturedStates,
    // 接收 Effect 观察器。
    effect_runs: Arc<AtomicUsize>,
    // 接收 renderer 失败开关。
    panic_now: Arc<AtomicBool>,
    // 返回可交给 ViewAdapter 的真实 Transfer 声明。
) -> ViewNode {
    // 建立具有稳定业务 key 的首个源条目。
    let mut first = TransferItem::new("a", "甲");
    // 按测试场景设置首项选择状态。
    first.selected = first_selected;
    // 建立含两个独立业务身份的 Transfer 声明。
    let transfer = Transfer::new()
        // 让 source pane 同时物化两个条目。
        .source(vec![first, TransferItem::new("b", "乙")]);
    // 按测试场景选择默认文本或自定义动态 renderer。
    let transfer = if with_renderer {
        // 安装会产生完整动态运行时输出的真实公开 renderer。
        transfer.render_item(move |item| {
            // 构造当前条目的树级动态声明。
            item_view(
                // 交付当前条目快照。
                item,
                // 交付共享调用记录。
                &factory_calls,
                // 交付共享状态观察表。
                &states,
                // 交付共享 Effect 观察器。
                &effect_runs,
                // 交付共享失败开关。
                &panic_now,
            )
        })
    } else {
        // 不安装 renderer 以验证旧动态条目释放。
        transfer
    };
    // 捕获 Transfer 声明本身，动态条目由 live WidgetTree 另行捕获。
    ViewAdapter::capture_root(|| ViewNode::leaf(transfer))
}

// 验证每个条目完整接管运行时输出，普通父协调保留独立状态身份。
#[test]
// 执行双条目隔离、重复协调与 Effect 调度回归。
fn transfer_item_dynamic_capture_preserves_independent_state_across_reconcile() {
    // 建立真实 renderer 调用次数记录。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 建立本轮私有 State 观察表。
    let states = Arc::new(Mutex::new(HashMap::new()));
    // 建立节点 Effect 运行次数记录。
    let effect_runs = Arc::new(AtomicUsize::new(0));
    // 建立默认关闭的 renderer 失败开关。
    let panic_now = Arc::new(AtomicBool::new(false));
    // 用真实 Transfer View 完成首次建树。
    let mut tree = ViewAdapter::build_nodes(transfer_view(
        // 首项保持未选择状态。
        false,
        // 安装自定义 renderer。
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
    // 读取唯一 Transfer owner。
    let transfer = tree.root_id().expect("Transfer 必须拥有根节点");
    // 两个源条目必须各执行一次真实 renderer。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 2);
    // 两个动态条目必须各自登记一个节点动画源。
    assert_eq!(tree.animated_source_registrations().len(), 2);
    // 保存两个条目首次物化的真实节点身份。
    let first_a = item_id(&tree, transfer, "source", "a");
    // 保存第二个条目首次物化的真实节点身份。
    let first_b = item_id(&tree, transfer, "source", "b");
    // 为首个条目写入非初值状态。
    captured_state(&states, "a").set(17);
    // 次条目必须保持自己的初始值。
    assert_eq!(captured_state(&states, "b").get(), 0);
    // 活跃条目私有 State 必须请求所属树执行协调。
    assert!(tree.take_reconcile_requested());
    // 只有首个条目 Effect 应进入待执行状态。
    assert!(tree.tick_effects());
    // 以等价声明重新协调同一 Transfer owner。
    ViewAdapter::reconcile_nodes(
        // 在同一真实树内执行父声明刷新。
        &mut tree,
        // 重新捕获等价 Transfer 声明。
        transfer_view(
            // 保持首项未选择。
            false,
            // 保持自定义 renderer。
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
    // 同一 source 业务 key 必须保留首项非初值状态。
    assert_eq!(captured_state(&states, "a").get(), 17);
    // 次项状态必须继续独立且不接收首项数值。
    assert_eq!(captured_state(&states, "b").get(), 0);
    // 两个稳定身份必须分别复用原有运行时节点。
    assert_eq!(item_id(&tree, transfer, "source", "a"), first_a);
    // 第二个稳定身份也必须复用原节点。
    assert_eq!(item_id(&tree, transfer, "source", "b"), first_b);
    // 父刷新必须重新执行两个声明 renderer 以交接最新输出。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 4);
    // 动画注册必须仍恰好属于两个活动条目。
    assert_eq!(tree.animated_source_registrations().len(), 2);
    // 受控关闭释放本测试树的动态资源。
    tree.shutdown();
}

// 验证跨 pane 移动与 renderer 移除会释放旧条目资源。
#[test]
// 执行身份切换、工厂移除与 shutdown 生命周期回归。
fn transfer_item_dynamic_capture_releases_moved_and_removed_state() {
    // 建立 renderer 调用次数记录。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 建立私有 State 观察表。
    let states = Arc::new(Mutex::new(HashMap::new()));
    // 建立 Effect 运行记录。
    let effect_runs = Arc::new(AtomicUsize::new(0));
    // 建立安全 renderer 失败开关。
    let panic_now = Arc::new(AtomicBool::new(false));
    // 首次物化首项已选择的 Transfer 树。
    let mut tree = ViewAdapter::build_nodes(transfer_view(
        // 预选首项以让 Enter 执行 source 到 target 移动。
        true,
        // 安装自定义 renderer。
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
    // 保存真实 Transfer owner。
    let transfer = tree.root_id().expect("Transfer 必须拥有根节点");
    // 保存移动前 source:a 的实际节点身份。
    let source_a = item_id(&tree, transfer, "source", "a");
    // 保存移动前仍可用于验证租约释放的旧状态句柄。
    let source_state = captured_state(&states, "a");
    // 写入非初值以排除重新物化假阳性。
    source_state.set(31);
    // 清除旧状态写入产生的协调请求。
    assert!(tree.take_reconcile_requested());
    // 清除旧状态写入产生的 Effect 调度。
    assert!(tree.tick_effects());
    // 通过真实键盘事件把已选首项移动到 target pane。
    let result = tree.dispatch_to(
        // 把事件直接交付当前 Transfer owner。
        transfer,
        // Enter 沿组件既有交互契约移动已选条目。
        &SystemEvent::KeyDown {
            // 使用真实移动快捷键。
            key: KeyCode::Enter,
            // 本场景不携带修饰键。
            mods: KeyMod::NONE,
        },
    );
    // 真实移动事件必须由 Transfer 处理。
    assert_eq!(result, EventResult::Handled);
    // 旧 source 身份必须完成真实移除。
    assert!(tree.get(source_a).is_none());
    // 同一业务条目进入 target 后必须取得新节点身份。
    let target_a = item_id(&tree, transfer, "target", "a");
    // 跨 pane 移动不得复用旧 source ComponentId。
    assert_ne!(target_a, source_a);
    // 新 pane 命名空间必须从条目私有状态初值开始。
    assert_eq!(captured_state(&states, "a").get(), 0);
    // 修改已释放 source 状态不得再请求所属树协调。
    source_state.set(32);
    // 旧状态租约必须在真实移除时同步释放。
    assert!(!tree.take_reconcile_requested());
    // 协调为无自定义 renderer，释放全部动态条目。
    ViewAdapter::reconcile_nodes(
        // 在同一真实树内更新 Transfer 声明。
        &mut tree,
        // 移除 renderer 而不改变公开组件类型。
        transfer_view(
            // 声明首项选择不影响 live Transfer 运行态。
            false,
            // 关闭自定义 renderer。
            false,
            // 保留调用记录对象。
            Arc::clone(&factory_calls),
            // 保留状态观察表。
            Arc::clone(&states),
            // 保留 Effect 观察器。
            Arc::clone(&effect_runs),
            // 保留安全失败开关。
            Arc::clone(&panic_now),
        ),
    );
    // renderer 移除后 Transfer 不再拥有任何 View 子节点。
    assert!(tree
        .get(transfer)
        .expect("Transfer 必须仍存在")
        .children()
        .is_empty());
    // 全部动态条目移除后不得遗留动画来源。
    assert!(tree.animated_source_registrations().is_empty());
    // 保存最近一次 target:a 状态句柄用于关闭前释放验证。
    let removed_state = captured_state(&states, "a");
    // 修改已移除状态不得重新请求树协调。
    removed_state.set(41);
    // 动态条目租约必须已经释放。
    assert!(!tree.take_reconcile_requested());
    // 受控关闭不得重新执行已移除 renderer。
    tree.shutdown();
}

// 验证直接刷新 renderer panic 可回滚，陈旧 owner 不会重新执行应用代码。
#[test]
// 执行捕获阶段异常恢复与 stale generation 准入回归。
fn transfer_item_dynamic_capture_recovers_pre_publish_panic_and_rejects_stale_owner() {
    // 建立 renderer 调用记录。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 建立私有状态观察表。
    let states = Arc::new(Mutex::new(HashMap::new()));
    // 建立 Effect 观察器。
    let effect_runs = Arc::new(AtomicUsize::new(0));
    // 建立可切换的 renderer 失败开关。
    let panic_now = Arc::new(AtomicBool::new(false));
    // 先用安全 renderer 建立真实 Transfer 树。
    let mut tree = ViewAdapter::build_nodes(transfer_view(
        // 首项保持未选择。
        false,
        // 安装自定义 renderer。
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
    // 保存后续会变为陈旧 generation 的 Transfer owner。
    let stale_owner = tree.root_id().expect("Transfer 必须拥有根节点");
    // 保存 panic 前首个条目节点身份。
    let stable_child = item_id(&tree, stale_owner, "source", "a");
    // 只为本次直接捕获打开失败开关。
    panic_now.store(true, Ordering::Relaxed);
    // 捕获在任何动态结构发布前发生的 renderer panic。
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 直接刷新必须先完整捕获，再进入动态协调事务。
        tree.refresh_transfer_item_component(stale_owner);
    }));
    // 捕获阶段 panic 必须向调用方传播。
    assert!(result.is_err());
    // 发布前失败后旧树必须继续允许外部工作。
    assert!(tree.accepts_external_work());
    // 旧动态条目节点必须保持原身份且没有半发布替换。
    assert_eq!(item_id(&tree, stale_owner, "source", "a"), stable_child);
    // 关闭失败开关，使同一刷新可以安全恢复。
    panic_now.store(false, Ordering::Relaxed);
    // 安全重试必须完成捕获，结构是否变化不影响本断言。
    let _ = tree.refresh_transfer_item_component(stale_owner);
    // 恢复后首项私有状态仍保持可读。
    assert_eq!(captured_state(&states, "a").get(), 0);
    // 完整换根使原 Transfer ComponentId generation 失效。
    tree.set_root(Box::new(Label::new("replacement")));
    // 记录 stale 刷新前的 renderer 调用次数。
    let calls_before_stale = factory_calls.load(Ordering::Relaxed);
    // 陈旧 owner 的迟到刷新必须安全拒绝。
    assert!(!tree.refresh_transfer_item_component(stale_owner));
    // stale 拒绝不得再次进入应用 renderer。
    assert_eq!(factory_calls.load(Ordering::Relaxed), calls_before_stale);
    // 受控关闭替换后的树必须保持可调用。
    tree.shutdown();
}
