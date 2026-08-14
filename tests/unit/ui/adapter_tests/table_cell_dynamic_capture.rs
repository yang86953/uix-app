// 导入父模块公开给适配器回归的声明树协调入口。
use super::ViewAdapter;
// 导入组件私有状态作用域与字段申请入口。
use crate::ui::component_state::{uix_component_scope, uix_component_state};
// 导入读取运行时节点子关系、key 与离场状态的核心接口。
use crate::ui::component::widget::WidgetCore;
// 导入动态单元格声明节点。
use crate::ui::view::ViewNode;
// 导入真实 DataTable 构造、列配置和最小单元格组件。
use crate::ui::widgets::{DataTable, Label, Table, TableColumn};
// 导入状态、Effect、动画、过渡和运行时树类型。
use crate::ui::{Animated, ComponentId, Easing, Effect, State, Transition, WidgetTree};
// 导入按稳定业务 row_key 保存最近状态的映射。
use std::collections::HashMap;
// 导入工厂调用与 Effect 执行记录所需的原子类型。
use std::sync::atomic::{AtomicUsize, Ordering};
// 导入运行时切换安全与 panic renderer 的原子布尔值。
use std::sync::atomic::AtomicBool;
// 导入跨协调轮次共享观测对象所需的所有权类型。
use std::sync::{Arc, Mutex};

// 定义最小 typed DataTable 行数据，使 row_key、文本与可观察 renderer 分离。
#[derive(Clone)]
struct TestRow {
    // 保存经 Table::data 校验的稳定业务 row_key。
    key: String,
    // 保存 renderer 输出中可识别的稳定文本。
    label: String,
}

// 定义单元格工厂在每轮捕获后回传的私有状态观察表。
type CapturedStates = Arc<Mutex<HashMap<String, State<i32>>>>;

// 按 row_key 从本轮 renderer 观察表读取私有状态句柄。
fn captured_state(
    // 接收跨 renderer 调用共享的状态观察表。
    states: &CapturedStates,
    // 接收需要读取的稳定业务 row_key。
    key: &str,
    // 返回该 row_key 本轮捕获到的私有状态。
) -> State<i32> {
    // 锁定观察表仅覆盖状态句柄克隆。
    states
        // 进入共享状态记录。
        .lock()
        // 即使先前断言 panic 也恢复记录以保留精确诊断。
        .unwrap_or_else(|error| error.into_inner())
        // 按 row_key 读取最近一次工厂交接的状态。
        .get(key)
        // 在脱离锁后保留公开状态句柄。
        .cloned()
        // 已物化单元格必须已经执行真实 renderer。
        .expect("已物化 DataTable 单元格必须回传组件私有状态")
}

// 在 Table 直接子节点中按稳定运行时 key 查找对应单元格身份。
fn cell_id(
    // 接收真实运行时树。
    tree: &WidgetTree,
    // 接收仍在运行的 Table owner。
    table: ComponentId,
    // 接收需要精确查找的运行时单元格 key。
    key: &str,
    // 返回该动态单元格当前的 ComponentId。
) -> ComponentId {
    // 遍历 Table 的实际直接动态子节点。
    tree.get(table)
        // 当前测试阶段 Table owner 必须可寻址。
        .expect("DataTable owner 必须存在")
        // 读取包含 pending leave 墓碑的真实子关系。
        .children()
        // 逐个检查稳定组件身份。
        .iter()
        // 复制轻量 ComponentId 以脱离父节点借用。
        .copied()
        // 使用运行时声明 key 进行精确匹配。
        .find(|child| tree.get(*child).and_then(|node| node.key()) == Some(key))
        // 测试目标单元格必须由 renderer 实际物化。
        .expect("DataTable 单元格必须存在")
}

// 构造逻辑列零与完整 row_key 共同限定的无碰撞动态单元格 identity。
fn cell_key(
    // 接收完整稳定业务 row_key。
    row_key: &str,
    // 返回与 DataTable capture namespace 完全一致的运行时节点 key。
) -> String {
    // 使用逻辑列、UTF-8 字节长度和完整 row_key 构成可逆 identity。
    format!("table-cell:0:{}:{row_key}", row_key.len())
}

// 构造带完整 State、Effect、AnimatedSource 与可选 leave 的真实单元格 View。
fn cell_view(
    // 接收当前业务行以读取稳定 key 与显示文本。
    row: &TestRow,
    // 接收工厂累计调用次数观察器。
    factory_calls: &Arc<AtomicUsize>,
    // 接收按 row_key 回传 State 的观察表。
    states: &CapturedStates,
    // 接收 Effect 实际运行次数观察器。
    effect_runs: &Arc<AtomicUsize>,
    // 指示本单元格是否保留一个长 leave 过渡。
    leave: bool,
    // 返回交给 DataTable renderer 的声明子树。
) -> ViewNode {
    // 记录本轮真实 renderer 调用，供重复刷新门禁断言。
    factory_calls.fetch_add(1, Ordering::Relaxed);
    // 建立单元格私有字段的固定组件 scope。
    let scope = uix_component_scope("table-cell-dynamic-capture-test", 1);
    // 在树签发的动态命名空间内取得或初始化本行私有状态。
    let state = uix_component_state(&scope, 1, || 0_i32);
    // 将本轮状态按稳定业务 key 回传给测试。
    states
        // 锁定共享表仅覆盖当前行的句柄写入。
        .lock()
        // 即使已有断言 panic 也恢复表以继续呈现精确失败。
        .unwrap_or_else(|error| error.into_inner())
        // 用本行完整 row_key 覆盖本轮最新句柄。
        .insert(row.key.clone(), state.clone());
    // 在声明捕获边界读取私有状态，使节点接管可取消的协调租约。
    let _ = state.get();
    // 克隆本行状态给 Effect，确保写入可调度节点拥有的副作用。
    let effect_state = state.clone();
    // 克隆 Effect 运行记录给长期闭包。
    let effect_runs = Arc::clone(effect_runs);
    // 建立归属单元格节点的响应式 Effect。
    let _effect = Effect::new(move || {
        // 读取私有状态以建立该节点的实际依赖。
        let _ = effect_state.get();
        // 记录 Effect 首次与后续运行次数。
        effect_runs.fetch_add(1, Ordering::Relaxed);
    });
    // 建立需要由单元格节点接管的动画源。
    let animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 在动态捕获边界内读取值，使动画来源进入节点所有权。
    let _ = animation.value();
    // 构造声明 scope 与本行可识别文本的最小单元格根。
    let node = ViewNode::leaf(Label::new(row.label.clone()))
        // 让运行时节点在释放时归还该行私有状态租约。
        .uix_component_scope(scope, 0);
    // 仅在生命周期测试要求时让移除保留为 pending leave 墓碑。
    if leave {
        // 使用长离场过渡避免测试期间被 tick 自动完成。
        node.leave(Transition::fade_out(10.0))
    } else {
        // 普通单元格保持立即真实移除语义。
        node
    }
}

// 构造真实 DataTable View，使用唯一 row_key 与一列自定义 renderer。
fn table_view(
    // 接收当前 DataTable 行集合。
    rows: Vec<TestRow>,
    // 接收 renderer 调用次数观察器。
    factory_calls: Arc<AtomicUsize>,
    // 接收私有状态观察表。
    states: CapturedStates,
    // 接收 Effect 运行次数观察器。
    effect_runs: Arc<AtomicUsize>,
    // 指示 cell View 是否配置 leave 过渡。
    leave: bool,
    // 返回可交给 ViewAdapter::capture_view 的 DataTable 声明。
) -> DataTable<TestRow> {
    // 以业务 row_key 构造并验证真实 typed DataTable。
    let table = Table::data(rows, |row| row.key.clone())
        // 测试 fixture 的 row_key 必须满足产品唯一性契约。
        .expect("DataTable 测试行 key 必须唯一");
    // 声明唯一的 View 列，令动态 child 与 row_key 一一对应。
    table
        // 设置绑定文本与真实 renderer 的组合列。
        .columns(vec![
            // 构造稳定逻辑列零。
            TableColumn::new("内容", 120.0)
                // 让展示文本保持与行数据一致。
                .bind(|row: &TestRow| row.label.clone())
                // 使用真实 typed renderer 交接完整动态运行时输出。
                .render(move |row| {
                    // 构造携带测试观察点的真实单元格 View。
                    cell_view(row, &factory_calls, &states, &effect_runs, leave)
                }),
        ])
        // 关闭虚拟化以便所有 fixture 行都被当前测试窗口物化。
        .virtualized(false)
        // 给根固定高度使直接建树与后续协调采用稳定可见范围。
        .size(240.0, 160.0)
}

// 从 DataTable 构造实际可捕获的声明 ViewNode。
fn captured_table_view(
    // 接收已经配置完成的 typed DataTable。
    table: DataTable<TestRow>,
    // 返回由真实 View build 与 capture 边界生成的声明节点。
) -> ViewNode {
    // 通过测试公开 capture_view 保留 DataTable 自身声明副作用的完整捕获语义。
    ViewAdapter::capture_view(table)
}

// 验证首次物化完整接管 State、Effect、AnimatedSource，常规刷新不重建。
#[test]
// 执行首轮交接、重复协调与 effect 调度回归。
fn table_cell_dynamic_capture_materializes_complete_runtime_outputs_without_rebuild() {
    // 创建两行稳定业务数据。
    let rows = vec![
        // 首行使用含冒号的 key 覆盖身份编码边界。
        TestRow {
            key: "alpha:0".to_owned(),
            label: "Alpha".to_owned(),
        },
        // 次行使用普通 key 覆盖相邻槽隔离。
        TestRow {
            key: "beta".to_owned(),
            label: "Beta".to_owned(),
        },
    ];
    // 建立真实 renderer 调用次数记录。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 建立本轮私有 State 观察表。
    let states = Arc::new(Mutex::new(HashMap::new()));
    // 建立节点 Effect 运行次数记录。
    let effect_runs = Arc::new(AtomicUsize::new(0));
    // 用真实 DataTable View 完成首次建树。
    let mut tree = ViewAdapter::build_nodes(captured_table_view(table_view(
        // 交付稳定初始行。
        rows.clone(),
        // 交付调用记录。
        Arc::clone(&factory_calls),
        // 交付状态观察表。
        Arc::clone(&states),
        // 交付 Effect 观察器。
        Arc::clone(&effect_runs),
        // 首轮不需要 leave 过渡。
        false,
    )));
    // 读取唯一 Table owner。
    let table = tree.root_id().expect("DataTable 必须拥有根节点");
    // 两行一列必须各执行一次真实 renderer。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 2);
    // 两个动态单元格必须各自登记一个节点动画源。
    assert_eq!(tree.animated_source_registrations().len(), 2);
    // 保存首版单元格动画工作身份，父协调必须用新版声明来源替换。
    let first_animation_ids = tree
        // 读取中央动画注册快照。
        .animated_source_registrations()
        // 逐项只保留稳定工作身份。
        .into_iter()
        // 丢弃本测试不需要的 owner 诊断数据。
        .map(|(work_id, _)| work_id)
        // 收集为可直接比较的有序快照。
        .collect::<Vec<_>>();
    // 两个首次构造的 Effect 必须各运行一次。
    assert_eq!(effect_runs.load(Ordering::Relaxed), 2);
    // 写入首行私有状态以建立非初值重用前提。
    captured_state(&states, "alpha:0").set(23);
    // 活跃单元格私有 State 必须请求所属树执行协调。
    assert!(tree.take_reconcile_requested());
    // 活跃单元格 Effect 必须因状态写入进入待执行状态。
    assert!(tree.has_pending_effects());
    // 执行本轮单元格 Effect。
    assert!(tree.tick_effects());
    // 只有依赖首行 State 的 Effect 应在本轮额外运行一次。
    assert_eq!(effect_runs.load(Ordering::Relaxed), 3);
    // 记录首行当前节点身份以验证重复刷新不重建。
    let first_id = cell_id(&tree, table, &cell_key("alpha:0"));

    // 以等价数据重新协调同一 DataTable owner。
    ViewAdapter::reconcile_nodes(
        // 在同一真实树内执行父声明刷新。
        &mut tree,
        // 重新捕获等价 DataTable 声明。
        captured_table_view(table_view(
            // 保持相同行集合与顺序。
            rows,
            // 使用同一调用记录。
            Arc::clone(&factory_calls),
            // 使用同一状态观察表。
            Arc::clone(&states),
            // 使用同一 Effect 观察器。
            Arc::clone(&effect_runs),
            // 保持无 leave 配置。
            false,
        )),
    );
    // 父刷新会重新捕获声明，但同一 row_key 必须保留非初值状态。
    assert_eq!(captured_state(&states, "alpha:0").get(), 23);
    // 同一 row_key+logical column 必须复用原有运行时节点。
    assert_eq!(cell_id(&tree, table, &cell_key("alpha:0")), first_id);
    // 动画注册必须仍恰好属于两条活动单元格。
    assert_eq!(tree.animated_source_registrations().len(), 2);
    // 父声明刷新必须用本轮捕获的新动画来源替换旧输出。
    let next_animation_ids = tree
        // 读取协调后的中央动画注册快照。
        .animated_source_registrations()
        // 逐项只保留稳定工作身份。
        .into_iter()
        // 丢弃本测试不需要的 owner 诊断数据。
        .map(|(work_id, _)| work_id)
        // 收集为与首版同形的比较快照。
        .collect::<Vec<_>>();
    // 同节点复用不能误保留首版动画来源。
    assert_ne!(next_animation_ids, first_animation_ids);
    // 再次写入首行状态以验证新版 Effect 已接管依赖。
    captured_state(&states, "alpha:0").set(24);
    // 新版 State 租约仍必须指向同一所属树。
    assert!(tree.take_reconcile_requested());
    // 只有当前节点拥有的新版 Effect 应进入树级待执行集合。
    assert!(tree.tick_effects());
    // 首建两次、首次更新一次、新声明两次、再次更新一次应精确执行六次。
    assert_eq!(effect_runs.load(Ordering::Relaxed), 6);
}

// 验证父数据行重排时 State 与 ComponentId 按 row_key 复用且互不串槽。
#[test]
// 执行稳定业务 key 重排隔离回归。
fn table_cell_dynamic_capture_reorder_preserves_state_without_cross_row_leakage() {
    // 建立两行具有可识别稳定 key 的初始数据。
    let original = vec![
        // 首行覆盖带分隔符的业务 key。
        TestRow {
            key: "a:1".to_owned(),
            label: "A".to_owned(),
        },
        // 次行覆盖普通业务 key。
        TestRow {
            key: "b".to_owned(),
            label: "B".to_owned(),
        },
    ];
    // 建立 renderer 调用次数记录。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 建立私有状态观察表。
    let states = Arc::new(Mutex::new(HashMap::new()));
    // 建立 Effect 观察器。
    let effect_runs = Arc::new(AtomicUsize::new(0));
    // 构造初始真实树。
    let mut tree = ViewAdapter::build_nodes(captured_table_view(table_view(
        // 交付初始顺序。
        original.clone(),
        // 交付共享调用记录。
        Arc::clone(&factory_calls),
        // 交付共享状态表。
        Arc::clone(&states),
        // 交付共享 Effect 观察器。
        Arc::clone(&effect_runs),
        // 重排测试不需要 leave。
        false,
    )));
    // 读取稳定 Table owner。
    let table = tree.root_id().expect("DataTable 必须拥有根节点");
    // 保存两行首次物化的真实节点身份。
    let first_a = cell_id(&tree, table, &cell_key("a:1"));
    // 保存第二行首次物化的真实节点身份。
    let first_b = cell_id(&tree, table, &cell_key("b"));
    // 为首行写入非初值状态。
    captured_state(&states, "a:1").set(41);
    // 为次行写入不同非初值状态。
    captured_state(&states, "b").set(92);
    // 两个活动 State 至少请求一次共同所属树协调。
    assert!(tree.take_reconcile_requested());
    // 清除这次状态写入调度的 Effect，隔离重排后断言。
    assert!(tree.tick_effects());

    // 建立仅交换行顺序、不改变业务 key 的下一轮数据。
    let reordered = vec![
        // 原次行移动到首位。
        original[1].clone(),
        // 原首行移动到次位。
        original[0].clone(),
    ];
    // 通过父级声明协调提交重排。
    ViewAdapter::reconcile_nodes(
        // 在同一 WidgetTree 内执行重排。
        &mut tree,
        // 捕获换序后的真实 DataTable 声明。
        captured_table_view(table_view(
            // 交付已重排行集合。
            reordered,
            // 使用同一调用记录。
            Arc::clone(&factory_calls),
            // 使用同一状态观察表。
            Arc::clone(&states),
            // 使用同一 Effect 观察器。
            Arc::clone(&effect_runs),
            // 保持无 leave 配置。
            false,
        )),
    );
    // 移动后的 a 行必须保留原节点身份。
    assert_eq!(cell_id(&tree, table, &cell_key("a:1")), first_a);
    // 移动后的 b 行必须保留原节点身份。
    assert_eq!(cell_id(&tree, table, &cell_key("b")), first_b);
    // a 行的状态必须跟随自身 row_key 而非当前索引。
    assert_eq!(captured_state(&states, "a:1").get(), 41);
    // b 行的状态必须保持独立且不得接收 a 行数值。
    assert_eq!(captured_state(&states, "b").get(), 92);
}

// 验证移出物化范围后的 State、Effect 与动画来源会被真实释放并可重新初始化。
#[test]
// 执行行删除、root replacement 与 shutdown 的资源释放回归。
fn table_cell_dynamic_capture_releases_removed_rows_and_owner_teardown() {
    // 建立一行稳定数据。
    let rows = vec![TestRow {
        key: "gone".to_owned(),
        label: "Gone".to_owned(),
    }];
    // 建立 renderer 调用记录。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 建立私有状态观察表。
    let states = Arc::new(Mutex::new(HashMap::new()));
    // 建立 Effect 观察器。
    let effect_runs = Arc::new(AtomicUsize::new(0));
    // 用动态单元格建立真实树。
    let mut tree = ViewAdapter::build_nodes(captured_table_view(table_view(
        // 交付初始单行。
        rows.clone(),
        // 交付调用记录。
        Arc::clone(&factory_calls),
        // 交付状态表。
        Arc::clone(&states),
        // 交付 Effect 观察器。
        Arc::clone(&effect_runs),
        // 删除测试使用立即移除。
        false,
    )));
    // 读取当前 Table owner。
    let table = tree.root_id().expect("DataTable 必须拥有根节点");
    // 保存稍后必须失效的动态单元格身份。
    let removed = cell_id(&tree, table, &cell_key("gone"));
    // 记录非初值状态，防止重建假阳性。
    captured_state(&states, "gone").set(71);
    // 删除前活动 State 必须仍拥有树协调租约。
    assert!(tree.take_reconcile_requested());
    // 清除活跃节点的本轮 Effect 调度。
    assert!(tree.tick_effects());

    // 协调为零行 DataTable，触发唯一单元格的真实移除。
    ViewAdapter::reconcile_nodes(
        // 在同一 tree 内更新数据。
        &mut tree,
        // 零行声明必须移除旧动态单元格。
        captured_table_view(table_view(
            // 交付空行集合。
            Vec::new(),
            // 复用同一调用记录。
            Arc::clone(&factory_calls),
            // 复用状态观察表。
            Arc::clone(&states),
            // 复用 Effect 观察器。
            Arc::clone(&effect_runs),
            // 保持立即移除。
            false,
        )),
    );
    // 已删除节点不得继续存在。
    assert!(tree.get(removed).is_none());
    // 已删除节点的动画来源必须随节点所有权释放。
    assert!(tree.animated_source_registrations().is_empty());
    // 写入旧 State 不得重新调度已经释放的 Effect。
    captured_state(&states, "gone").set(72);
    // 已删除节点的旧 State 不得再请求原树协调。
    assert!(!tree.take_reconcile_requested());
    // 旧 State 已无节点 Effect 所有者，树不得出现 pending 工作。
    assert!(!tree.has_pending_effects());

    // 重新加入相同 row_key，验证已释放状态不会错误复活。
    ViewAdapter::reconcile_nodes(
        // 在同一 tree 内重新物化该业务行。
        &mut tree,
        // 重新交付原行集合。
        captured_table_view(table_view(
            // 交付原始单行。
            rows,
            // 复用调用记录。
            Arc::clone(&factory_calls),
            // 复用状态表。
            Arc::clone(&states),
            // 复用 Effect 观察器。
            Arc::clone(&effect_runs),
            // 保持立即移除配置。
            false,
        )),
    );
    // 已真实释放后重建必须回到初始化值，而不是旧值 71。
    assert_eq!(captured_state(&states, "gone").get(), 0);
    // 保存重建单元格的当前状态句柄以验证 root replacement 释放 Effect。
    let replacement_state = captured_state(&states, "gone");
    // 直接 root replacement 必须释放当前动态动画来源。
    tree.set_root(Box::new(Label::new("replacement")));
    // root replacement 后不允许遗留 DataTable 单元格动画注册。
    assert!(tree.animated_source_registrations().is_empty());
    // 写入已经离开树的重建状态。
    replacement_state.set(1);
    // root replacement 必须同步释放该 State 的树协调租约。
    assert!(!tree.take_reconcile_requested());
    // root replacement 必须同步释放该单元格 Effect 所有权。
    assert!(!tree.has_pending_effects());
    // shutdown 后继续保持无动态动画来源终态。
    tree.shutdown();
    // shutdown 必须幂等释放全部剩余资源。
    assert!(tree.animated_source_registrations().is_empty());
}

// 验证 pending leave 墓碑不会促使同 range 重复调用 factory，并允许同 key 重入复用。
#[test]
// 执行 leave tombstone active-count、重入取消与 State 复用回归。
fn table_cell_dynamic_capture_leave_tombstone_does_not_repeat_factory_and_reenters() {
    // 建立两行数据，其中第二行将在缩行时进入 leave。
    let rows = vec![
        // 保留第一行作为缩行后的活动单元格。
        TestRow {
            key: "keep".to_owned(),
            label: "Keep".to_owned(),
        },
        // 保留第二行作为离场和重入目标。
        TestRow {
            key: "return".to_owned(),
            label: "Return".to_owned(),
        },
    ];
    // 建立 renderer 调用记录。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 建立私有状态观察表。
    let states = Arc::new(Mutex::new(HashMap::new()));
    // 建立 Effect 观察器。
    let effect_runs = Arc::new(AtomicUsize::new(0));
    // 用带长 leave 的单元格建立真实树。
    let mut tree = ViewAdapter::build_nodes(captured_table_view(table_view(
        // 交付完整两行。
        rows.clone(),
        // 交付调用记录。
        Arc::clone(&factory_calls),
        // 交付状态表。
        Arc::clone(&states),
        // 交付 Effect 观察器。
        Arc::clone(&effect_runs),
        // 为每个单元格启用 leave。
        true,
    )));
    // 读取 Table owner。
    let table = tree.root_id().expect("DataTable 必须拥有根节点");
    // 保存将进入 leave 的单元格身份。
    let returning_id = cell_id(&tree, table, &cell_key("return"));
    // 写入非初值状态，避免重入时的假阳性。
    captured_state(&states, "return").set(23);
    // 离场前活动 State 必须请求所属树协调。
    assert!(tree.take_reconcile_requested());
    // 清除本轮状态写入引起的 Effect。
    assert!(tree.tick_effects());

    // 缩减为仅保留第一行，第二行必须成为 pending leave 墓碑。
    ViewAdapter::reconcile_nodes(
        // 在当前树内协调新的行集合。
        &mut tree,
        // 交付只含活动首行的声明。
        captured_table_view(table_view(
            // 保留第一行。
            vec![rows[0].clone()],
            // 复用调用记录。
            Arc::clone(&factory_calls),
            // 复用状态表。
            Arc::clone(&states),
            // 复用 Effect 观察器。
            Arc::clone(&effect_runs),
            // 保持 leave 配置。
            true,
        )),
    );
    // 离场单元格必须仍可寻址，直至 leave 真正完成。
    assert!(tree.get(returning_id).is_some());
    // 离场节点必须标记为 pending removal。
    assert!(
        tree.get(returning_id)
            .expect("离场单元格必须存在")
            .pending_removal()
    );
    // 记录缩行协调后的 renderer 调用数。
    let calls_after_shrink = factory_calls.load(Ordering::Relaxed);
    // 同一已物化 range 的刷新不得把 leave 墓碑误算为缺少活动子节点。
    assert!(!tree.refresh_table_cell_component(table));
    // pending leave 墓碑不能驱动重复 factory 调用。
    assert_eq!(factory_calls.load(Ordering::Relaxed), calls_after_shrink);

    // 在 leave 完成前恢复原两行，触发同 key 单元格重入。
    ViewAdapter::reconcile_nodes(
        // 在同一树内重新交付两行。
        &mut tree,
        // 使用原始 row_key 集合让旧离场节点匹配。
        captured_table_view(table_view(
            // 恢复完整行集合。
            rows,
            // 复用调用记录。
            Arc::clone(&factory_calls),
            // 复用状态表。
            Arc::clone(&states),
            // 复用 Effect 观察器。
            Arc::clone(&effect_runs),
            // 继续提供 leave 配置。
            true,
        )),
    );
    // 同一业务 row_key 重入必须复用原 pending 节点身份。
    assert_eq!(cell_id(&tree, table, &cell_key("return")), returning_id);
    // 重入必须取消 pending removal 而不是留下不可交互墓碑。
    assert!(
        !tree
            .get(returning_id)
            .expect("重入单元格必须存在")
            .pending_removal()
    );
    // 重入必须保留已经写入的私有 State。
    assert_eq!(captured_state(&states, "return").get(), 23);
}

// 验证非 Table、陈旧 owner 与 leaving Table owner 都不会调用应用 renderer。
#[test]
// 执行动态捕获 owner 类型、generation 与离场准入门禁回归。
fn table_cell_dynamic_capture_rejects_non_table_stale_and_leaving_owner() {
    // 建立普通标签树覆盖非 Table owner。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Label::new("ordinary")));
    // 保存标签 owner 身份。
    let stale_owner = tree.root_id().expect("普通测试树必须拥有根节点");
    // 非 Table 节点不得触发 renderer 或动态协调。
    assert!(!tree.refresh_table_cell_component(stale_owner));
    // 完整换根使原 ComponentId generation 失效。
    tree.set_root(Box::new(Label::new("replacement")));
    // 陈旧 owner 的迟到刷新必须安全拒绝。
    assert!(!tree.refresh_table_cell_component(stale_owner));

    // 建立单行动态表所需的共享 renderer 观测器。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 建立该表的私有状态观察表。
    let states = Arc::new(Mutex::new(HashMap::new()));
    // 建立该表的 Effect 观察器。
    let effect_runs = Arc::new(AtomicUsize::new(0));
    // 构造一个由容器承载、可整体进入 leave 的 DataTable child。
    let table_node = captured_table_view(table_view(
        // 交付唯一动态行。
        vec![TestRow {
            key: "leaving".to_owned(),
            label: "Leaving".to_owned(),
        }],
        // 交付调用记录。
        Arc::clone(&factory_calls),
        // 交付状态表。
        Arc::clone(&states),
        // 交付 Effect 观察器。
        Arc::clone(&effect_runs),
        // 单元格本身不需要 leave。
        false,
    ))
    // 让整个 Table owner 在被父级移除时保持 pending leave。
    .leave(Transition::fade_out(10.0));
    // 把可离场 DataTable 挂到稳定容器根。
    let mut leaving_tree = ViewAdapter::build_nodes(ViewNode::new(
        // 使用普通容器作为长期父 owner。
        crate::ui::widgets::Container::new(),
        // 交付唯一 Table child。
        vec![table_node],
    ));
    // 读取容器根。
    let container = leaving_tree.root_id().expect("容器根必须存在");
    // 读取唯一 Table child 的 owner 身份。
    let leaving_table = *leaving_tree
        .get(container)
        // 容器根必须存在。
        .expect("容器根必须存在")
        // 容器必须持有唯一 Table child。
        .children()
        // 读取唯一 child。
        .first()
        // 子 Table 必须在首次建树已挂载。
        .expect("容器必须拥有 Table child");
    // 移除声明 child 但保留其 leave 墓碑。
    ViewAdapter::reconcile_nodes(
        // 在同一容器树上协调空子集合。
        &mut leaving_tree,
        // 保持容器根类型而移除 Table child。
        ViewNode::leaf(crate::ui::widgets::Container::new()),
    );
    // Table owner 必须仍存在且处于 pending leave。
    assert!(
        leaving_tree
            .get(leaving_table)
            .expect("离场 Table 必须存在")
            .pending_removal()
    );
    // 记录移除后的 renderer 调用次数。
    let calls_before_reject = factory_calls.load(Ordering::Relaxed);
    // leaving owner 绝不能重新调用用户 renderer。
    assert!(!leaving_tree.refresh_table_cell_component(leaving_table));
    // 准入拒绝不得增加 factory 调用次数。
    assert_eq!(factory_calls.load(Ordering::Relaxed), calls_before_reject);
}

// 验证直接刷新 renderer panic 可回滚，而父级协调中的 renderer panic 必须 fail-stop。
#[test]
// 执行直接 capture 回滚与父级发布后 fail-stop 语义回归。
fn table_cell_dynamic_capture_distinguishes_pre_publish_and_post_publish_panics() {
    // 建立足够产生不同虚拟化物化窗口的稳定业务数据。
    let rows = (0..20)
        // 为每个逻辑行构造唯一业务 key 与可识别文本。
        .map(|index| TestRow {
            // 使用索引派生的稳定 row_key。
            key: format!("row-{index}"),
            // 使用索引派生的展示文本。
            label: format!("Row {index}"),
        })
        // 收集为真实 typed DataTable 的行集合。
        .collect::<Vec<_>>();
    // 建立当前 renderer 是否应 panic 的开关。
    let panic_now = Arc::new(AtomicBool::new(false));
    // 建立安全与 panic 调用共用的 renderer 调用记录。
    let pre_calls = Arc::new(AtomicUsize::new(0));
    // 复制切换开关给实际 typed renderer。
    let renderer_panic_now = Arc::clone(&panic_now);
    // 复制调用记录给实际 typed renderer。
    let renderer_calls = Arc::clone(&pre_calls);
    // 构造可切换安全与 panic 的虚拟化 DataTable。
    let initial = Table::data(rows.clone(), |row| row.key.clone())
        // 测试数据必须满足 row_key 唯一性。
        .expect("DataTable 测试行 key 必须唯一")
        // 注册可在 capture 边界内切换失败的唯一 View 列。
        .columns(vec![
            // 构造逻辑列零。
            TableColumn::new("内容", 120.0)
                // 保留普通绑定文本。
                .bind(|row: &TestRow| row.label.clone())
                // 在运行时开关请求时才触发 capture 阶段异常。
                .render(move |row| {
                    // 记录 renderer 确实已进入用户代码。
                    renderer_calls.fetch_add(1, Ordering::Relaxed);
                    // 开关置位时在首个运行时发布 mutator 前触发异常。
                    assert!(
                        !renderer_panic_now.load(Ordering::Relaxed),
                        "DataTable 单元格捕获阶段测试异常"
                    );
                    // 安全路径返回可协调的普通单元格 View。
                    ViewNode::leaf(Label::new(row.label.clone()))
                }),
        ])
        // 开启虚拟化以便改变滚动偏移请求新的物化窗口。
        .virtualized(true)
        // 固定较小高度以限制首轮物化范围。
        .size(240.0, 80.0)
        // 让每行高度固定，滚动偏移可确定地跨越首个窗口。
        .row_height(20.0);
    // 建立安全 renderer 的真实 DataTable 树。
    let mut tree = ViewAdapter::build_nodes(captured_table_view(initial));
    // 保存直接刷新 panic 后必须继续有效的 Table owner。
    let stable_root = tree.root_id().expect("DataTable 必须拥有根节点");
    // 保存首个虚拟窗口中的第一条动态单元格身份。
    let first_window_child = *tree
        // 读取当前 Table owner。
        .get(stable_root)
        // 首轮物化后 owner 必须可寻址。
        .expect("DataTable 必须拥有根节点")
        // 读取当前虚拟窗口的直接单元格集合。
        .children()
        // 第一条单元格必须由安全 renderer 物化。
        .first()
        // 保存轻量运行时身份供换窗后验证释放。
        .expect("首个虚拟窗口必须物化单元格");
    // 通过真实运行时 Table 写入下一个虚拟窗口的滚动偏移。
    tree.get_mut(stable_root)
        // 根节点必须仍可寻址。
        .expect("DataTable 必须拥有根节点")
        // 取得实际 Table 组件。
        .component_mut()
        // 恢复具体 DataTable 运行时类型。
        .as_any_mut()
        // 根声明必须保持 Table 类型。
        .downcast_mut::<Table>()
        // Table 根必须存在以更新其虚拟滚动状态。
        .expect("DataTable 根必须保持 Table 类型")
        // 改变物化范围，使直接刷新需要再次调用 renderer。
        .body_scroll
        // 跳过若干完整行，避免仍命中首轮 materialized range。
        .set_scroll_offset(120.0);
    // 仅为这次直接 refresh 打开 capture 阶段 panic。
    panic_now.store(true, Ordering::Relaxed);
    // 捕获直接动态刷新在首个 reconcile 发布前发生的 renderer panic。
    let pre_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 直接刷新入口必须先 capture 本窗口再进入动态协调事务。
        tree.refresh_table_cell_component(stable_root);
    }));
    // capture 阶段 panic 必须向调用方传播。
    assert!(pre_result.is_err());
    // 直接刷新尚未发布结构，失败后旧树必须仍保持 Operational。
    assert!(tree.accepts_external_work());
    // 直接 capture 失败必须保留原 Table owner identity。
    assert_eq!(tree.root_id(), Some(stable_root));
    // 关闭 panic 开关，使相同 range 随后可以安全重试。
    panic_now.store(false, Ordering::Relaxed);
    // 回滚后安全 renderer 必须能物化该窗口并继续协调。
    assert!(tree.refresh_table_cell_component(stable_root));
    // 已完全离开新虚拟窗口的首条旧单元格必须真实释放。
    assert!(tree.get(first_window_child).is_none());

    // 复制调用记录给父级协调的最新 renderer。
    let latest_calls = Arc::clone(&pre_calls);
    // 构造父级协调时会触发 renderer panic 的最新 DataTable 声明。
    let post_publish = Table::data(rows, |row| row.key.clone())
        // 测试数据必须满足 row_key 唯一性。
        .expect("DataTable 测试行 key 必须唯一")
        // 注册由父级 reconcile 调用的最新 renderer。
        .columns(vec![
            // 构造逻辑列零。
            TableColumn::new("内容", 120.0)
                // 保留普通绑定文本。
                .bind(|row: &TestRow| row.label.clone())
                // 在父级事务已发布后触发稳定 renderer panic。
                .render(move |_row| -> ViewNode {
                    // 记录最新 renderer 的确进入用户代码。
                    latest_calls.fetch_add(1, Ordering::Relaxed);
                    // 父级 reconcile 已跨过发布线，异常必须触发 fail-stop。
                    panic!("DataTable 单元格父级协调测试异常")
                }),
        ])
        // 保持虚拟化配置与当前 owner 类型完全一致。
        .virtualized(true)
        // 保持固定根尺寸。
        .size(240.0, 80.0)
        // 保持固定行高与当前 owner 一致。
        .row_height(20.0);
    // 捕获父级结构发布开始后产生的最新 renderer panic。
    let post_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 父级 reconcile 会在 publish marker 之后刷新最新 renderer。
        ViewAdapter::reconcile_nodes(&mut tree, captured_table_view(post_publish));
    }));
    // 父级发布阶段异常必须继续传播。
    assert!(post_result.is_err());
    // 父级发布后异常必须让半树永久进入 fail-stop。
    assert!(!tree.accepts_external_work());
    // fail-stop 不得暴露半发布根身份。
    assert!(tree.root_id().is_none());
    // 受控 teardown 必须保持可调用且幂等。
    tree.shutdown();
    // shutdown 后旧树仍不得恢复工作能力。
    assert!(!tree.accepts_external_work());
    // 安全、直接 panic、重试与最新 renderer 均必须确实进入过用户代码。
    assert!(pre_calls.load(Ordering::Relaxed) > 1);
}
