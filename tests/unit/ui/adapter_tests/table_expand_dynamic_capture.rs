// 导入父模块公开给适配器回归的声明树协调入口。
use super::ViewAdapter;
// 导入组件私有状态作用域与字段申请入口。
use crate::ui::component_state::{uix_component_scope, uix_component_state};
// 导入读取运行时节点子关系与稳定 key 的核心接口。
use crate::ui::component::widget::WidgetCore;
// 导入表格指针动作以直接驱动真实展开状态机。
use crate::ui::widgets::display::table::types::TablePointerAction;
// 导入声明节点与表格行快照类型。
use crate::ui::view::ViewNode;
// 导入真实容器、表格和最小展示组件。
use crate::ui::widgets::{Container, Label, Table, TableColumn, TableRow};
// 导入状态、Effect、动画、过渡和运行时树类型。
use crate::ui::{Animated, ComponentId, Easing, Effect, State, Transition, WidgetTree};
// 导入按行文案保存最近状态的映射。
use std::collections::HashMap;
// 导入 panic 开关、调用计数与共享内存顺序。
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
// 导入跨协调轮次共享观测对象所需的所有权类型。
use std::sync::{Arc, Mutex};

// 定义展开行工厂在每轮捕获后回传的私有状态观察表。
type CapturedStates = Arc<Mutex<HashMap<String, State<i32>>>>;

// 按行文案读取本轮 renderer 回传的组件私有状态句柄。
fn captured_state(
    // 接收跨 renderer 调用共享的状态观察表。
    states: &CapturedStates,
    // 接收需要读取的当前行文案。
    label: &str,
    // 返回该行业务实例最近捕获到的私有状态。
) -> State<i32> {
    // 锁定观察表仅覆盖状态句柄克隆。
    states
        // 进入共享状态记录。
        .lock()
        // 即使先前断言 panic 也恢复记录以保留精确诊断。
        .unwrap_or_else(|error| error.into_inner())
        // 按当前行文案读取最近一次工厂交接的状态。
        .get(label)
        // 在脱离锁后保留公开状态句柄。
        .cloned()
        // 已物化展开行必须已经执行真实 renderer。
        .expect("已物化 Table 展开行必须回传组件私有状态")
}

// 返回 Table 当前唯一活动展开行动态根的运行时身份。
fn expanded_child(
    // 接收真实运行时树。
    tree: &WidgetTree,
    // 接收仍在运行的 Table owner。
    table: ComponentId,
    // 返回唯一展开行根的组件身份。
) -> ComponentId {
    // 读取 Table owner 的直接子关系。
    *tree
        // Table owner 必须仍可寻址。
        .get(table)
        // 当前测试阶段 Table 不得提前离树。
        .expect("Table owner 必须存在")
        // 展开行由 Table 直接拥有。
        .children()
        // 同一时刻只允许一个活动展开行。
        .first()
        // 已展开 Table 必须已经物化动态根。
        .expect("Table 必须拥有展开行动态根")
}

// 构造与产品捕获命名空间完全一致的展开行稳定身份。
fn expanded_key(
    // 接收 Table 已确定的稳定行业务键。
    row_key: &str,
    // 返回框架写入动态根的权威协调 key。
) -> String {
    // 使用 UTF-8 字节长度与完整业务键避免拼接歧义。
    format!("table-expand:{}:{row_key}", row_key.len())
}

// 构造带完整 State、Effect 与 AnimatedSource 的真实展开行 View。
fn expanded_view(
    // 接收当前行快照以读取可观察文案。
    row: &TableRow,
    // 接收工厂累计调用次数观察器。
    factory_calls: &Arc<AtomicUsize>,
    // 接收按行文案回传 State 的观察表。
    states: &CapturedStates,
    // 接收 Effect 实际运行次数观察器。
    effect_runs: &Arc<AtomicUsize>,
    // 接收 capture 阶段的可控 panic 开关。
    panic_now: &Arc<AtomicBool>,
    // 返回交给 Table renderer 的声明子树。
) -> ViewNode {
    // 记录本轮真实 renderer 调用。
    factory_calls.fetch_add(1, Ordering::Relaxed);
    // 在任何运行时输出发布前触发可控 capture 阶段异常。
    assert!(
        // 安全轮次继续构建完整展开行。
        !panic_now.load(Ordering::Relaxed),
        // 提供固定测试异常以验证事务回滚。
        "Table 展开行捕获阶段测试异常"
    );
    // 从当前行快照读取稳定测试文案。
    let label = row.first().cloned().unwrap_or_default();
    // 建立展开行私有字段的固定组件 scope。
    let scope = uix_component_scope("table-expand-dynamic-capture-test", 1);
    // 在树签发的动态命名空间内取得或初始化本行私有状态。
    let state = uix_component_state(&scope, 1, || 0_i32);
    // 将本轮状态按行文案回传给测试。
    states
        // 锁定共享表仅覆盖当前行句柄写入。
        .lock()
        // 恢复可能被预期 panic 污染的测试观察锁。
        .unwrap_or_else(|error| error.into_inner())
        // 覆盖该行本轮最新捕获到的状态句柄。
        .insert(label.clone(), state.clone());
    // 在声明捕获边界读取私有状态，使节点接管协调租约。
    let _ = state.get();
    // 克隆当前行状态给长期 Effect 闭包。
    let effect_state = state.clone();
    // 克隆 Effect 运行记录给长期闭包。
    let effect_runs = Arc::clone(effect_runs);
    // 建立归属当前展开行节点的响应式 Effect。
    let _effect = Effect::new(move || {
        // 读取私有状态以建立该节点的实际依赖。
        let _ = effect_state.get();
        // 记录 Effect 首次与后续运行次数。
        effect_runs.fetch_add(1, Ordering::Relaxed);
    });
    // 建立需要由展开行节点接管的动画来源。
    let animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 在动态捕获边界读取值，使动画来源进入节点所有权。
    let _ = animation.value();
    // 返回携带组件 scope 的最小展开行声明根。
    ViewNode::leaf(Label::new(label))
        // 让运行时节点在释放时归还该行私有状态租约。
        .uix_component_scope(scope, 0)
}

// 构造使用普通 Table 行索引稳定键的真实可展开表格声明。
fn table_view(
    // 接收当前 Table 行集合。
    rows: Vec<TableRow>,
    // 接收 renderer 调用次数观察器。
    factory_calls: Arc<AtomicUsize>,
    // 接收私有状态观察表。
    states: CapturedStates,
    // 接收 Effect 运行次数观察器。
    effect_runs: Arc<AtomicUsize>,
    // 接收 capture 阶段 panic 开关。
    panic_now: Arc<AtomicBool>,
    // 返回已经过根捕获的 Table 声明节点。
) -> ViewNode {
    // 构造带一列、当前行快照和展开行 renderer 的真实 Table。
    let table = Table::new()
        // 声明最小普通文本列。
        .columns(vec![TableColumn::new("内容", 120.0)])
        // 写入当前行集合并生成与索引一一对应的稳定 row_key。
        .rows(rows)
        // 注册真实延迟 View 工厂。
        .expandable(40.0, move |row| {
            // 在树级动态捕获边界中构造当前展开行。
            expanded_view(
                // 交付当前 Table 行快照。
                row,
                // 交付共享调用记录。
                &factory_calls,
                // 交付共享状态观察表。
                &states,
                // 交付共享 Effect 记录。
                &effect_runs,
                // 交付可控 panic 开关。
                &panic_now,
            )
        });
    // 通过真实 View build 与 capture 边界生成 Table 根声明。
    ViewAdapter::capture_view(table)
}

// 切换指定展开行并执行真实动态刷新入口。
fn toggle_expanded_row(
    // 接收需要更新的运行时树。
    tree: &mut WidgetTree,
    // 接收 Table owner 身份。
    table: ComponentId,
    // 接收需要切换的逻辑行索引。
    row: usize,
    // 返回动态刷新是否实际协调了子树。
) -> bool {
    // 取得运行时 Table 组件并提交真实指针动作。
    tree.get_mut(table)
        // 当前 Table owner 必须仍可寻址。
        .expect("Table owner 必须存在")
        // 取得具体组件可变引用。
        .component_mut()
        // 恢复动态组件的具体类型。
        .as_any_mut()
        // 声明根必须保持 Table 类型。
        .downcast_mut::<Table>()
        // 测试接线错误必须立即失败。
        .expect("展开行 owner 必须保持 Table 类型")
        // 复用产品交互状态机切换展开行。
        .commit_pointer_action(TablePointerAction::ToggleExpand(row));
    // 执行生产路径的动态展开行捕获与协调。
    tree.refresh_table_expand_component(table)
}

// 验证同一稳定行在父声明刷新后复用私有状态和动态根身份。
#[test]
// 执行首次物化、同 key 重捕获、Effect 与 shutdown 所有权回归。
fn table_expand_dynamic_capture_reuses_same_row_and_complete_runtime_outputs() {
    // 建立两个可展开行快照。
    let rows = vec![vec!["Alpha".to_owned()], vec!["Beta".to_owned()]];
    // 建立 renderer 调用记录。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 建立私有状态观察表。
    let states = Arc::new(Mutex::new(HashMap::new()));
    // 建立 Effect 运行记录。
    let effect_runs = Arc::new(AtomicUsize::new(0));
    // 建立关闭的 panic 开关。
    let panic_now = Arc::new(AtomicBool::new(false));
    // 首次建树时没有展开行，renderer 不应提前执行。
    let mut tree = ViewAdapter::build_nodes(table_view(
        // 交付初始行快照。
        rows.clone(),
        // 交付共享调用记录。
        Arc::clone(&factory_calls),
        // 交付共享状态表。
        Arc::clone(&states),
        // 交付共享 Effect 记录。
        Arc::clone(&effect_runs),
        // 交付共享 panic 开关。
        Arc::clone(&panic_now),
    ));
    // 初始折叠状态不得调用延迟工厂。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 0);
    // 读取稳定 Table owner。
    let table = tree.root_id().expect("Table 必须拥有根节点");
    // 展开首行并物化真实动态根。
    assert!(toggle_expanded_row(&mut tree, table, 0));
    // 首次展开必须且只能调用一次 renderer。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 1);
    // 保存首个动态根身份供同 key 协调复用断言。
    let first_child = expanded_child(&tree, table);
    // 动态根 key 必须与索引 row_key 的捕获身份一致。
    assert_eq!(
        // 读取实际声明协调 key。
        tree.get(first_child).and_then(|node| node.key()),
        // 普通 Table 首行使用隐式稳定 row_key 零。
        Some(expanded_key("0").as_str())
    );
    // 首次捕获必须交接唯一动画来源。
    assert_eq!(tree.animated_source_registrations().len(), 1);
    // 写入非初值以验证同 key 重捕获不会重置状态。
    captured_state(&states, "Alpha").set(41);
    // 活动 State 必须请求所属树协调。
    assert!(tree.take_reconcile_requested());
    // 依赖该 State 的 Effect 必须由树实际调度。
    assert!(tree.tick_effects());
    // 记录首次状态句柄供 shutdown 后释放断言。
    let first_state = captured_state(&states, "Alpha");

    // 用同一业务行快照重新协调父 Table 声明。
    ViewAdapter::reconcile_nodes(
        // 在同一 WidgetTree 中保留 Table owner。
        &mut tree,
        // 交付会重置 materialized 标记的同内容声明。
        table_view(
            // 保持行索引和内容不变。
            rows,
            // 复用 renderer 调用记录。
            Arc::clone(&factory_calls),
            // 复用状态观察表。
            Arc::clone(&states),
            // 复用 Effect 记录。
            Arc::clone(&effect_runs),
            // 保持安全 capture 路径。
            Arc::clone(&panic_now),
        ),
    );
    // 父声明刷新必须重新执行一次展开行工厂。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 2);
    // 同一稳定 row_key 必须复用原动态根身份。
    assert_eq!(expanded_child(&tree, table), first_child);
    // 同一动态命名空间必须继续读取已写入值。
    assert_eq!(captured_state(&states, "Alpha").get(), 41);
    // 重捕获不得泄漏或重复登记动画来源。
    assert_eq!(tree.animated_source_registrations().len(), 1);

    // 关闭树必须统一释放展开行的 State、Effect 与动画来源。
    tree.shutdown();
    // shutdown 后不允许遗留动画来源。
    assert!(tree.animated_source_registrations().is_empty());
    // 写入关闭前状态不得再请求原树协调。
    first_state.set(42);
    // 已关闭树不得收到旧 State 的协调请求。
    assert!(!tree.take_reconcile_requested());
    // 已关闭树不得保留旧展开行 Effect 工作。
    assert!(!tree.has_pending_effects());
}

// 验证切换行业务身份会隔离状态，并在真实移除后重新初始化。
#[test]
// 执行行切换、旧租约释放、同索引重新进入与折叠释放回归。
fn table_expand_dynamic_capture_isolates_row_identity_and_releases_removed_row() {
    // 建立两个相邻行快照。
    let rows = vec![vec!["Alpha".to_owned()], vec!["Beta".to_owned()]];
    // 建立 renderer 调用记录。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 建立私有状态观察表。
    let states = Arc::new(Mutex::new(HashMap::new()));
    // 建立 Effect 运行记录。
    let effect_runs = Arc::new(AtomicUsize::new(0));
    // 建立关闭的 panic 开关。
    let panic_now = Arc::new(AtomicBool::new(false));
    // 建立真实可展开 Table 树。
    let mut tree = ViewAdapter::build_nodes(table_view(
        // 交付当前行快照。
        rows,
        // 交付共享调用记录。
        Arc::clone(&factory_calls),
        // 交付共享状态表。
        Arc::clone(&states),
        // 交付共享 Effect 记录。
        Arc::clone(&effect_runs),
        // 交付安全 capture 开关。
        panic_now,
    ));
    // 读取稳定 Table owner。
    let table = tree.root_id().expect("Table 必须拥有根节点");
    // 首先展开索引零行。
    assert!(toggle_expanded_row(&mut tree, table, 0));
    // 保存索引零行状态并写入非初值。
    let alpha_state = captured_state(&states, "Alpha");
    // 写入用于识别错误串槽的值。
    alpha_state.set(71);
    // 清除活动 State 的协调请求。
    assert!(tree.take_reconcile_requested());
    // 清除活动 Effect 的待处理工作。
    assert!(tree.tick_effects());

    // 切换到索引一行并替换动态根。
    assert!(toggle_expanded_row(&mut tree, table, 1));
    // 索引一必须使用独立动态命名空间初始化。
    assert_eq!(captured_state(&states, "Beta").get(), 0);
    // 当前根 key 必须使用索引一业务身份。
    assert_eq!(
        // 读取当前唯一展开行动态根 key。
        tree.get(expanded_child(&tree, table))
            // 索引一根必须仍可寻址。
            .expect("索引一展开行必须存在")
            // 读取其权威协调 key。
            .key(),
        // 普通 Table 次行使用隐式稳定 row_key 一。
        Some(expanded_key("1").as_str())
    );
    // 清除切换过程中可能产生的树级请求。
    let _ = tree.take_reconcile_requested();
    // 旧索引零状态在真实移除后不得再请求树协调。
    alpha_state.set(72);
    // 已释放旧 State 不得影响当前 Table 树。
    assert!(!tree.take_reconcile_requested());
    // 已释放旧 Effect 不得留下待处理工作。
    assert!(!tree.has_pending_effects());

    // 再次切换回索引零行。
    assert!(toggle_expanded_row(&mut tree, table, 0));
    // 已真实释放的索引零私有状态必须重新初始化。
    assert_eq!(captured_state(&states, "Alpha").get(), 0);
    // 再次点击当前行会折叠并真实移除动态根。
    assert!(toggle_expanded_row(&mut tree, table, 0));
    // 折叠后 Table 不得保留直接动态 child。
    assert!(
        tree
            // 读取仍在运行的 Table owner。
            .get(table)
            // Table owner 必须继续存在。
            .expect("折叠后 Table 必须存在")
            // 检查展开行直接子关系已清空。
            .children()
            // 没有活动或墓碑展开行才算完成释放。
            .is_empty()
    );
    // 折叠后不得遗留展开行动画来源。
    assert!(tree.animated_source_registrations().is_empty());
}

// 验证 capture panic 可回滚，且离场或关闭 owner 不再调用应用工厂。
#[test]
// 执行预发布 panic 恢复、pending leave 与 shutdown 准入门禁回归。
fn table_expand_dynamic_capture_rolls_back_panics_and_rejects_inactive_owner() {
    // 建立单行快照。
    let rows = vec![vec!["Only".to_owned()]];
    // 建立 renderer 调用记录。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 建立私有状态观察表。
    let states = Arc::new(Mutex::new(HashMap::new()));
    // 建立 Effect 运行记录。
    let effect_runs = Arc::new(AtomicUsize::new(0));
    // 首轮开启 capture 阶段 panic。
    let panic_now = Arc::new(AtomicBool::new(true));
    // 建立尚未展开的安全 Table 根。
    let mut tree = ViewAdapter::build_nodes(table_view(
        // 交付单行快照。
        rows.clone(),
        // 交付共享调用记录。
        Arc::clone(&factory_calls),
        // 交付共享状态表。
        Arc::clone(&states),
        // 交付共享 Effect 记录。
        Arc::clone(&effect_runs),
        // 交付已开启的 panic 开关。
        Arc::clone(&panic_now),
    ));
    // 读取稳定 Table owner。
    let table = tree.root_id().expect("Table 必须拥有根节点");
    // 先提交展开动作，使下一步直接进入动态 capture。
    tree.get_mut(table)
        // Table owner 必须可寻址。
        .expect("Table owner 必须存在")
        // 取得真实组件可变引用。
        .component_mut()
        // 恢复动态组件类型。
        .as_any_mut()
        // 声明必须保持 Table。
        .downcast_mut::<Table>()
        // 测试接线错误必须立即失败。
        .expect("展开行 owner 必须保持 Table 类型")
        // 提交索引零展开动作。
        .commit_pointer_action(TablePointerAction::ToggleExpand(0));
    // 捕获用户 renderer 在首个协调发布前发生的异常。
    let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 直接刷新应把异常限制在 capture 阶段。
        tree.refresh_table_expand_component(table)
    }));
    // 测试异常必须确实传播给调用方。
    assert!(panic_result.is_err());
    // 预发布异常后 Table owner 必须继续可用。
    assert!(tree.get(table).is_some());
    // 回滚后不得挂载半成品动态根。
    assert!(
        tree
            // 读取仍可用的 Table owner。
            .get(table)
            // Table owner 必须存在。
            .expect("panic 后 Table 必须存在")
            // 检查直接展开行子关系。
            .children()
            // 没有发布任何动态根才算回滚成功。
            .is_empty()
    );
    // 回滚后不得泄漏动画来源。
    assert!(tree.animated_source_registrations().is_empty());
    // 关闭 panic 开关以验证同一 owner 可恢复。
    panic_now.store(false, Ordering::Relaxed);
    // materialized 标记未提交，下一次刷新必须安全成功。
    assert!(tree.refresh_table_expand_component(table));
    // 安全恢复必须物化唯一动态根。
    assert_eq!(tree.get(table).expect("Table 必须存在").children().len(), 1);

    // 构造一个由稳定容器承载、可整体进入 leave 的 Table child。
    let leaving_node = table_view(
        // 交付同一单行快照。
        rows,
        // 复用调用记录。
        Arc::clone(&factory_calls),
        // 复用状态观察表。
        states,
        // 复用 Effect 记录。
        effect_runs,
        // 保持安全 capture 路径。
        Arc::clone(&panic_now),
    )
    // 让整个 Table owner 在父级移除时保留 pending leave 墓碑。
    .leave(Transition::fade_out(10.0));
    // 把可离场 Table 挂到稳定容器根。
    let mut leaving_tree = ViewAdapter::build_nodes(ViewNode::new(
        // 使用普通容器作为长期父 owner。
        Container::new(),
        // 交付唯一 Table child。
        vec![leaving_node],
    ));
    // 读取容器根。
    let container = leaving_tree.root_id().expect("容器根必须存在");
    // 读取唯一 Table child 身份。
    let leaving_table = *leaving_tree
        // 容器根必须可寻址。
        .get(container)
        // 根声明必须保持容器。
        .expect("容器根必须存在")
        // 读取直接子关系。
        .children()
        // 读取唯一 Table child。
        .first()
        // Table child 必须在首次建树挂载。
        .expect("容器必须拥有 Table child");
    // 移除声明 child 并保留其 leave 墓碑。
    ViewAdapter::reconcile_nodes(
        // 在同一树中协调父容器。
        &mut leaving_tree,
        // 保持容器根类型但移除 Table child。
        ViewNode::leaf(Container::new()),
    );
    // Table owner 必须仍存在且处于 pending leave。
    assert!(
        leaving_tree
            // 读取离场 Table 墓碑。
            .get(leaving_table)
            // 长 leave 期间节点必须可寻址。
            .expect("离场 Table 必须存在")
            // 验证 owner 已进入不可重新捕获阶段。
            .pending_removal()
    );
    // 记录离场刷新前的用户工厂调用次数。
    let calls_before_leave_refresh = factory_calls.load(Ordering::Relaxed);
    // leaving owner 绝不能签发动态捕获能力。
    assert!(!leaving_tree.refresh_table_expand_component(leaving_table));
    // 准入拒绝不得调用任何应用 renderer。
    assert_eq!(
        // 读取离场刷新后的调用次数。
        factory_calls.load(Ordering::Relaxed),
        // 调用次数必须保持完全不变。
        calls_before_leave_refresh
    );

    // 记录关闭树前的 renderer 调用次数。
    let calls_before_shutdown = factory_calls.load(Ordering::Relaxed);
    // 关闭原安全恢复树并释放其动态资源。
    tree.shutdown();
    // 关闭树不得再次执行展开行刷新。
    assert!(!tree.refresh_table_expand_component(table));
    // shutdown 准入拒绝不得调用应用 renderer。
    assert_eq!(
        // 读取 shutdown 后调用次数。
        factory_calls.load(Ordering::Relaxed),
        // 调用次数必须保持关闭前值。
        calls_before_shutdown
    );
}
