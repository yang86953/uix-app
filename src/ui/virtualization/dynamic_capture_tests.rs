// 导入被测 VirtualScroll 运行时组件。
use super::{VirtualScroll, VirtualScrollBuilder};
// 导入创建真实 UIX 私有组件状态所需的隐藏运行时入口。
use crate::ui::component_state::{uix_component_scope, uix_component_state};
// 导入按 key 读取虚拟行运行时身份所需的树节点接口。
use crate::ui::component::widget::WidgetCore;
// 导入 View 构建与协调入口。
use crate::ui::adapter::ViewAdapter;
// 导入声明行节点类型。
use crate::ui::view::ViewNode;
// 导入最小行组件。
use crate::ui::widgets::Label;
// 导入动画、组件身份、Effect 与 State 行为观察类型。
use crate::ui::{Animated, ComponentId, Easing, Effect, State, WidgetTree};
// 导入跨 renderer 调用保存每个绝对索引状态句柄的映射。
use std::collections::HashMap;
// 导入测试闭包共享可变记录所需的单线程所有权容器。
use std::sync::{Arc, Mutex};

// 构造会在真实 VirtualScroll renderer 中产生私有 State、Effect 与 AnimatedSource 的行。
fn captured_row(
    // 接收运行时请求的绝对项目索引。
    index: usize,
    // 接收每个索引独立的结构性状态依赖。
    state_dependencies: &[State<i32>],
    // 接收每个索引独立的 Effect 依赖。
    effect_dependencies: &[State<i32>],
    // 接收每个索引独立的动画源。
    animations: &[Animated<f32>],
    // 回传每次 renderer 调用取得的私有状态句柄。
    captured_states: &Arc<Mutex<HashMap<usize, State<i32>>>>,
    // 回传每个索引 Effect 的实际执行次数。
    effect_runs: &Arc<Vec<std::sync::atomic::AtomicUsize>>,
    // 返回带稳定行 key 和组件 scope marker 的声明节点。
) -> ViewNode {
    // 读取当前项目的结构性 State，使本行拥有独立 reconcile 租约。
    let _ = state_dependencies[index].get();
    // 克隆当前项目的 Effect 依赖供副作用闭包拥有。
    let effect_dependency = effect_dependencies[index].clone();
    // 克隆运行次数数组供副作用闭包拥有。
    let effect_runs = Arc::clone(effect_runs);
    // 创建只依赖当前绝对索引状态的节点 Effect。
    let _ = Effect::new(move || {
        // 读取当前索引依赖以建立可取消的 Effect 订阅。
        let _ = effect_dependency.get();
        // 记录当前索引副作用的首次与后续执行。
        effect_runs[index].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    });
    // 读取当前索引动画值，使本行声明捕获其 AnimatedSource。
    let _ = animations[index].value();
    // 为同一静态行组件调用申请当前动态命名空间内的作用域。
    let scope = uix_component_scope("virtual-scroll-dynamic-row-test", 1);
    // 取得或初始化当前绝对索引对应的组件私有状态。
    let private_state = uix_component_state(&scope, 1, || 0_i32);
    // 保存最新捕获句柄供测试跨刷新直接观察槽身份与值。
    captured_states
        // 锁定共享映射仅覆盖本次同步写入。
        .lock()
        // 测试线程中的锁不得被先前 panic 污染。
        .unwrap_or_else(|error| error.into_inner())
        // 用绝对索引覆盖该行本轮返回的状态句柄。
        .insert(index, private_state);
    // 构造不自行设置根 key 的行节点，身份由 VirtualScroll 键工厂权威注入。
    ViewNode::leaf(Label::new(format!("row-{index}")))
        // 让实际挂载行节点承载该组件私有状态作用域。
        .uix_component_scope(scope, 0)
}

// 读取指定虚拟行的运行时组件身份。
fn row_id(
    // 接收被检查的运行时树。
    tree: &WidgetTree,
    // 接收 VirtualScroll 宿主身份。
    root: ComponentId,
    // 接收目标绝对索引。
    index: usize,
    // 返回当前带对应稳定 key 的实际行身份。
) -> ComponentId {
    // 构造 renderer 为当前绝对索引声明的运行时 key。
    let expected_key = format!("virtual-scroll-item:{index}");
    // 遍历宿主当前实际物化的直接子节点。
    tree.get(root)
        // VirtualScroll 根必须仍可寻址。
        .expect("VirtualScroll 根必须存在")
        // 借用当前物化行身份集合。
        .children()
        // 逐项查找目标稳定 key。
        .iter()
        // 复制找到的实际运行时身份。
        .copied()
        // 读取节点 key 并与目标绝对索引比较。
        .find(|id| tree.get(*id).and_then(|node| node.key()) == Some(expected_key.as_str()))
        // 目标索引在当前物化范围内时必须存在。
        .expect("目标虚拟行必须已经物化")
}

// 从共享映射取得指定绝对索引最近一次捕获的私有状态句柄。
fn captured_state(
    // 接收 renderer 回传状态句柄的共享映射。
    captured_states: &Arc<Mutex<HashMap<usize, State<i32>>>>,
    // 接收目标绝对索引。
    index: usize,
    // 返回该索引当前捕获的私有状态句柄。
) -> State<i32> {
    // 锁定映射只覆盖句柄克隆操作。
    captured_states
        // 进入共享测试记录。
        .lock()
        // 即使先前断言 panic 也恢复记录以便诊断。
        .unwrap_or_else(|error| error.into_inner())
        // 读取当前索引最近一次 renderer 调用结果。
        .get(&index)
        // 克隆公开 State 句柄以脱离映射锁。
        .cloned()
        // 当前物化索引必须已经执行过 renderer。
        .expect("目标索引必须已经返回私有状态")
}

// 构造以业务稳定键统一拥有节点身份与组件私有状态的可重排虚拟列表。
fn keyed_reorder_view(
    // 接收当前绝对索引到业务项名称的可变顺序。
    order: Arc<Mutex<Vec<String>>>,
    // 回传每个业务项最近一次捕获的私有 State 句柄。
    states: Arc<Mutex<HashMap<String, State<i32>>>>,
    // 返回可进入真实 ViewAdapter 协调路径的 VirtualScroll builder。
) -> VirtualScrollBuilder {
    // 在构建两个 static 闭包前读取当前项目总数。
    let item_count = order
        // 锁定顺序只覆盖长度读取。
        .lock()
        // 测试线程中的 poison 继续恢复内部值供诊断。
        .unwrap_or_else(|error| error.into_inner())
        // 读取稳定项目总数。
        .len();
    // 克隆顺序供键工厂独立拥有。
    let key_order = Arc::clone(&order);
    // 克隆顺序供行工厂独立拥有。
    let row_order = Arc::clone(&order);
    // 克隆状态记录映射供行工厂写入。
    let row_states = Arc::clone(&states);
    // 构造完整业务 keyed 虚拟列表。
    VirtualScroll::new()
        // 声明当前顺序中的项目总数。
        .item_count(item_count)
        // 使用固定十像素行高。
        .item_height(10.0)
        // 当前测试同时物化两个业务项。
        .size(100.0, 20.0)
        // 在行构建前取得业务稳定键。
        .render_keyed(
            // 键工厂按当前绝对索引读取逻辑项名称。
            move |index| {
                // 锁定顺序只覆盖单项克隆。
                key_order
                    // 取得当前顺序快照。
                    .lock()
                    // poison 后恢复内部值供失败诊断。
                    .unwrap_or_else(|error| error.into_inner())
                    // 读取对应逻辑业务项。
                    .get(index)
                    // 业务项数量必须与 item_count 一致。
                    .expect("业务键索引必须有效")
                    // 返回键工厂拥有的字符串。
                    .clone()
            },
            // 行工厂按相同当前顺序构建业务项 View。
            move |index| {
                // 克隆当前绝对索引对应的业务项名称。
                let name = row_order
                    // 锁定顺序只覆盖单项克隆。
                    .lock()
                    // poison 后恢复内部值供失败诊断。
                    .unwrap_or_else(|error| error.into_inner())
                    // 读取对应逻辑业务项。
                    .get(index)
                    // 业务项数量必须与 item_count 一致。
                    .expect("业务行索引必须有效")
                    // 脱离顺序锁后继续构建 View。
                    .clone();
                // 为同一静态行组件声明取得当前动态业务命名空间作用域。
                let scope = uix_component_scope("virtual-scroll-keyed-reorder-test", 1);
                // 在业务键命名空间内取得或初始化私有状态。
                let state = uix_component_state(&scope, 1, || 0_i32);
                // 回传业务项与私有状态句柄的最新映射。
                row_states
                    // 锁定记录映射只覆盖单次写入。
                    .lock()
                    // poison 后恢复内部值供失败诊断。
                    .unwrap_or_else(|error| error.into_inner())
                    // 用业务名称覆盖本轮句柄。
                    .insert(name.clone(), state);
                // 返回不自行设置根 key 的唯一业务行节点。
                ViewNode::leaf(Label::new(name))
                    // 让挂载行承载当前组件私有状态作用域。
                    .uix_component_scope(scope, 0)
            },
        )
}

// 按规范化业务 key 读取当前物化行的运行时组件身份。
fn business_row_id(
    // 接收被检查的运行时树。
    tree: &WidgetTree,
    // 接收 VirtualScroll 宿主身份。
    root: ComponentId,
    // 接收原始业务键。
    key: &str,
    // 返回当前业务项的真实组件身份。
) -> ComponentId {
    // 生成与运行时业务身份模式一致的规范 key。
    let expected = format!("virtual-scroll-business:{key}");
    // 遍历宿主直接物化子项寻找规范业务 key。
    tree.get(root)
        // VirtualScroll 根必须存在。
        .expect("VirtualScroll 根必须存在")
        // 读取直接物化行集合。
        .children()
        // 按声明顺序遍历运行时身份。
        .iter()
        // 复制找到的轻量组件身份。
        .copied()
        // 比较节点权威 key。
        .find(|id| tree.get(*id).and_then(|node| node.key()) == Some(expected.as_str()))
        // 当前两项都在物化窗口内，必须找到目标。
        .expect("业务项必须已物化")
}

// 验证 VirtualScroll 生产 renderer 按绝对索引继承树级私有状态及完整输出生命周期。
#[test]
// 执行两个索引隔离、重叠复用、移出释放与再次进入重建回归。
fn virtual_scroll_dynamic_capture_preserves_each_index_and_releases_removed_rows() {
    // 为三个可能物化的索引分别建立结构性状态依赖。
    let state_dependencies = Arc::new(vec![
        State::new(0_i32),
        State::new(0_i32),
        State::new(0_i32),
    ]);
    // 为三个索引分别建立 Effect 依赖。
    let effect_dependencies = Arc::new(vec![
        State::new(0_i32),
        State::new(0_i32),
        State::new(0_i32),
    ]);
    // 为三个索引分别建立持续活动的动画源。
    let animations = Arc::new(vec![
        // 索引零使用独立工作身份。
        Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear),
        // 索引一使用独立工作身份。
        Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear),
        // 索引二使用独立工作身份。
        Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear),
    ]);
    // 保存每个绝对索引最近一次取得的组件私有状态。
    let captured_states = Arc::new(Mutex::new(HashMap::new()));
    // 保存每个索引副作用的实际运行次数。
    let effect_runs = Arc::new(vec![
        // 索引零初始尚未执行。
        std::sync::atomic::AtomicUsize::new(0),
        // 索引一初始尚未执行。
        std::sync::atomic::AtomicUsize::new(0),
        // 索引二初始尚未执行。
        std::sync::atomic::AtomicUsize::new(0),
    ]);
    // 克隆结构性依赖供应用 renderer 长期拥有。
    let renderer_state_dependencies = Arc::clone(&state_dependencies);
    // 克隆 Effect 依赖供应用 renderer 长期拥有。
    let renderer_effect_dependencies = Arc::clone(&effect_dependencies);
    // 克隆动画集合供应用 renderer 长期拥有。
    let renderer_animations = Arc::clone(&animations);
    // 克隆私有状态记录供应用 renderer 回传句柄。
    let renderer_captured_states = Arc::clone(&captured_states);
    // 克隆副作用计数供应用 renderer 写入。
    let renderer_effect_runs = Arc::clone(&effect_runs);
    // 构造每次只物化两个连续绝对索引的真实 VirtualScroll。
    let view = VirtualScroll::new()
        // 声明三个可滚动项目。
        .item_count(3)
        // 每行使用十像素固定高度。
        .item_height(10.0)
        // 禁用 overscan 以精确控制移入移出。
        .overscan(0)
        // 二十像素视口同时物化两个项目。
        .size(100.0, 20.0)
        // 使用生产延迟 renderer 构造带全部捕获输出的行。
        .render(move |index| {
            // 把当前绝对索引交给共享测试行工厂。
            captured_row(
                // 传递运行时请求的绝对索引。
                index,
                // 传递每项结构性 State 依赖。
                renderer_state_dependencies.as_slice(),
                // 传递每项 Effect 依赖。
                renderer_effect_dependencies.as_slice(),
                // 传递每项 AnimatedSource。
                renderer_animations.as_slice(),
                // 回传每项组件私有状态。
                &renderer_captured_states,
                // 记录每项 Effect 实际运行次数。
                &renderer_effect_runs,
            )
        });
    // 通过真实建树路径立即物化索引零与一。
    let mut tree = ViewAdapter::build(view);
    // 读取 VirtualScroll 实际宿主身份。
    let root = tree.root_id().expect("VirtualScroll 必须拥有运行时根");
    // 保存索引一首次物化的运行时行身份。
    let first_row_one = row_id(&tree, root, 1);
    // 读取索引零私有状态。
    let first_state_zero = captured_state(&captured_states, 0);
    // 读取索引一私有状态。
    let first_state_one = captured_state(&captured_states, 1);
    // 两个索引都应分别建立动画源注册。
    assert_eq!(tree.animated_source_registrations().len(), 2);
    // 两个索引的 Effect 都必须完成首次运行。
    assert_eq!(effect_runs[0].load(std::sync::atomic::Ordering::Relaxed), 1);
    // 索引一的 Effect 同样不能被批量捕获覆盖。
    assert_eq!(effect_runs[1].load(std::sync::atomic::Ordering::Relaxed), 1);
    // 写入索引零私有状态。
    first_state_zero.set(7);
    // 写入索引一私有状态。
    first_state_one.set(9);
    // 两个独立槽必须保存各自值。
    assert_eq!(first_state_zero.get(), 7);
    // 索引一不得与索引零串槽。
    assert_eq!(first_state_one.get(), 9);
    // 清除建树与私有状态更新产生的协调请求。
    let _ = tree.take_reconcile_requested();
    // 修改索引零结构性依赖以验证首项 State bind 没有被后一项覆盖。
    state_dependencies[0].set(1);
    // 首项依赖必须能请求所属树协调。
    assert!(tree.take_reconcile_requested());
    // 修改索引一结构性依赖以验证末项绑定同样存在。
    state_dependencies[1].set(1);
    // 末项依赖也必须请求同一所属树协调。
    assert!(tree.take_reconcile_requested());
    // 同时触发两个行节点 Effect 的依赖。
    effect_dependencies[0].set(1);
    // 触发索引一独立副作用依赖。
    effect_dependencies[1].set(1);
    // 一次树级 tick 必须遍历两个实际行节点的 Effect。
    assert!(tree.tick_effects());
    // 索引零 Effect 应恰好增加一次运行。
    assert_eq!(effect_runs[0].load(std::sync::atomic::Ordering::Relaxed), 2);
    // 索引一 Effect 也应恰好增加一次运行。
    assert_eq!(effect_runs[1].load(std::sync::atomic::Ordering::Relaxed), 2);

    // 把运行态偏移推进一行，使窗口从零一平移到一二。
    tree.get_mut(root)
        // VirtualScroll 根必须仍可寻址。
        .expect("VirtualScroll 根必须存在")
        // 取得运行时组件可变引用。
        .component_mut()
        // 转换为可变类型擦除接口。
        .as_any_mut()
        // 恢复真实 VirtualScroll 类型。
        .downcast_mut::<VirtualScroll>()
        // 根类型在原位协调前不得变化。
        .expect("运行时根必须是 VirtualScroll")
        // 写入一行滚动偏移。
        .scroll_offset
        // 保存确定性偏移。
        .set(10.0);
    // 通过生产刷新入口物化索引一与二并移除索引零。
    assert!(tree.refresh_virtual_scroll_component(root, Some(20.0)));
    // 重叠索引一必须继续复用原运行时组件身份。
    assert_eq!(row_id(&tree, root, 1), first_row_one);
    // 重叠索引一必须复用同一私有状态值。
    assert_eq!(captured_state(&captured_states, 1).get(), 9);
    // 新进入索引二必须使用私有状态初值。
    assert_eq!(captured_state(&captured_states, 2).get(), 0);
    // 物化窗口仍只有两个节点动画源 owner。
    assert_eq!(tree.animated_source_registrations().len(), 2);
    // 清除窗口替换本身产生的协调请求，隔离已移除依赖的租约断言。
    let _ = tree.take_reconcile_requested();
    // 被真实移除索引零的旧结构性依赖不得再请求树协调。
    state_dependencies[0].set(2);
    // 旧节点租约必须已经释放。
    assert!(!tree.take_reconcile_requested());
    // 被真实移除索引零的旧 Effect 依赖不得进入树 pending 集合。
    effect_dependencies[0].set(2);
    // 树中没有索引零 Effect 可供调度。
    assert!(!tree.has_pending_effects());

    // 把偏移恢复到零，使索引零重新进入物化窗口。
    tree.get_mut(root)
        // VirtualScroll 根必须仍可寻址。
        .expect("VirtualScroll 根必须存在")
        // 取得运行时组件可变引用。
        .component_mut()
        // 转换为可变类型擦除接口。
        .as_any_mut()
        // 恢复真实 VirtualScroll 类型。
        .downcast_mut::<VirtualScroll>()
        // 根类型不得在滚动刷新中变化。
        .expect("运行时根必须是 VirtualScroll")
        // 访问内部滚动状态。
        .scroll_offset
        // 恢复到首个窗口。
        .set(0.0);
    // 刷新后重新物化索引零与一。
    assert!(tree.refresh_virtual_scroll_component(root, Some(20.0)));
    // 真实移出再进入的索引零必须重新初始化私有状态。
    assert_eq!(captured_state(&captured_states, 0).get(), 0);
}

// 验证业务稳定键在重排后同时保留 ComponentId 与组件私有 State。
#[test]
// 将两个业务项交换绝对索引，检查所有权随业务键而非位置移动。
fn virtual_scroll_keyed_reorder_preserves_business_identity_and_private_state() {
    // 建立初始业务顺序 a、b。
    let order = Arc::new(Mutex::new(vec!["a".to_string(), "b".to_string()]));
    // 保存每个业务项最近一次捕获的私有 State。
    let states = Arc::new(Mutex::new(HashMap::new()));
    // 通过真实建树路径物化两个业务项。
    let mut tree = ViewAdapter::build(keyed_reorder_view(
        // 让首版键工厂与行工厂共享顺序句柄。
        Arc::clone(&order),
        // 让首版 renderer 回传状态句柄。
        Arc::clone(&states),
    ));
    // 读取稳定 VirtualScroll 根身份。
    let root = tree.root_id().expect("VirtualScroll 必须拥有根");
    // 保存 a 的首版运行时组件身份。
    let a_id = business_row_id(&tree, root, "a");
    // 保存 b 的首版运行时组件身份。
    let b_id = business_row_id(&tree, root, "b");
    // 取得 a 的私有 State 句柄并写入非初值。
    states
        // 锁定记录映射只覆盖句柄读取。
        .lock()
        // poison 后恢复内部值供失败诊断。
        .unwrap_or_else(|error| error.into_inner())
        // 业务项 a 必须已捕获状态。
        .get("a")
        // 克隆句柄后脱离映射锁。
        .expect("a 必须拥有私有状态")
        // 写入可区分值。
        .set(7);
    // 取得 b 的私有 State 句柄并写入另一非初值。
    states
        // 锁定记录映射只覆盖句柄读取。
        .lock()
        // poison 后恢复内部值供失败诊断。
        .unwrap_or_else(|error| error.into_inner())
        // 业务项 b 必须已捕获状态。
        .get("b")
        // 克隆句柄后脱离映射锁。
        .expect("b 必须拥有私有状态")
        // 写入可区分值。
        .set(9);
    // 把业务顺序改为 b、a，两个逻辑项交换绝对索引。
    *order
        // 锁定顺序只覆盖整份替换。
        .lock()
        // poison 后恢复内部值供失败诊断。
        .unwrap_or_else(|error| error.into_inner()) = vec!["b".to_string(), "a".to_string()];
    // 用新版 renderer 声明协调同一根树。
    ViewAdapter::reconcile(
        // 原位协调现有树。
        &mut tree,
        // 新版声明读取交换后的业务顺序。
        keyed_reorder_view(Arc::clone(&order), Arc::clone(&states)),
    );
    // a 交换到索引一后仍必须复用原 ComponentId。
    assert_eq!(business_row_id(&tree, root, "a"), a_id);
    // b 交换到索引零后仍必须复用原 ComponentId。
    assert_eq!(business_row_id(&tree, root, "b"), b_id);
    // 读取协调后回传的业务状态映射。
    let states = states
        // 锁定映射覆盖最终断言。
        .lock()
        // poison 后恢复内部值供失败诊断。
        .unwrap_or_else(|error| error.into_inner());
    // a 的私有状态必须随业务键保留七。
    assert_eq!(states.get("a").expect("a 状态必须存在").get(), 7);
    // b 的私有状态必须随业务键保留九。
    assert_eq!(states.get("b").expect("b 状态必须存在").get(), 9);
    // 父节点首项必须是重排后的业务 b。
    assert_eq!(
        // 读取重排后第一个活动子项。
        tree.get(root).expect("根必须存在").children()[0],
        // 业务 b 的稳定运行时身份。
        b_id
    );
}

// 验证重复业务键会在执行任何行工厂或状态捕获前失败。
#[test]
// 通过 renderer 调用计数证明整批键预检先于 View 构建。
fn virtual_scroll_duplicate_keys_fail_before_rendering_rows() {
    // 记录用户行工厂是否被错误调用。
    let render_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    // 克隆计数器供 static 行工厂持有。
    let renderer_calls = Arc::clone(&render_calls);
    // 在 unwind 边界内构造同一窗口重复业务键。
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 两个物化项都返回相同业务键。
        let view = VirtualScroll::new()
            // 声明两个项目。
            .item_count(2)
            // 使用有效固定行高。
            .item_height(10.0)
            // 固定两行视口，使重复键位于同一批次。
            .size(100.0, 20.0)
            // 建立重复键与可观察行工厂。
            .render_keyed(
                // 两个索引故意返回同一键。
                |_| "duplicate",
                // 行工厂若被调用就记录错误时序。
                move |index| {
                    // 记录实际行构建调用。
                    renderer_calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    // 返回最小无 key 行节点。
                    ViewNode::leaf(Label::new(format!("row-{index}")))
                },
            );
        // 真实建树会在首批物化中执行整批键预检。
        let _ = ViewAdapter::build(view);
    }));
    // 重复键必须产生可诊断失败而非静默覆盖 HashMap。
    assert!(result.is_err());
    // 任意行工厂与状态捕获都不得在重复键诊断前执行。
    assert_eq!(render_calls.load(std::sync::atomic::Ordering::Relaxed), 0);
}

// 验证真实 Wheel 分派在同一事件内物化新窗口，而不是等待手工刷新或无关布局。
#[test]
// 通过公开事件入口驱动 VirtualScroll 的滚动、动态协调与权威索引 key。
fn virtual_scroll_wheel_dispatch_materializes_new_window_immediately() {
    // 构造零 overscan、两行视口的顺序不可变虚拟列表。
    let view = VirtualScroll::new()
        // 提供足够项目让滚轮跨越初始窗口。
        .item_count(100)
        // 每行十像素，使八十像素事件稳定跨越多个索引。
        .item_height(10.0)
        // 禁用 overscan 以便精确比较物化窗口。
        .overscan(0)
        // 固定二十像素高视口。
        .size(100.0, 20.0)
        // 普通 renderer 由框架按绝对索引拥有身份。
        .render(|index| ViewNode::leaf(Label::new(format!("row-{index}"))));
    // 通过真实建树路径初始物化索引零与一。
    let mut tree = ViewAdapter::build(view);
    // 执行一次布局以建立真实 frame 与 hit-test 几何。
    tree.layout();
    // 读取 VirtualScroll 根身份。
    let root = tree.root_id().expect("VirtualScroll 必须拥有根");
    // 初始窗口必须包含前两个框架索引 key。
    assert!(tree
        // 读取根的直接物化子项。
        .get(root)
        // 根节点必须仍然存在。
        .expect("VirtualScroll 根必须存在")
        // 遍历初始两行。
        .children()
        // 检查索引零身份存在。
        .iter()
        // 任一行命中即可证明初始窗口已物化。
        .any(|id| tree.get(*id).and_then(|node| node.key()) == Some("virtual-scroll-item:0")));
    // 在视口中心派发正向 Wheel，VirtualScroll 会消费并推进偏移。
    let result = tree.dispatch_event(&crate::ui::SystemEvent::Wheel {
        // 使用根 frame 内部坐标命中 VirtualScroll。
        pos: crate::core::Point::new(5.0, 5.0),
        // 正向增量经既有四十倍滚动契约跨越初始窗口。
        delta: crate::core::Point::new(0.0, 2.0),
    });
    // VirtualScroll 必须处理该滚轮事件。
    assert_eq!(result, crate::ui::EventResult::Handled);
    // 不调用 layout 或 refresh，直接读取同一事件后的活动物化 key。
    let keys = tree
        // 根在事件后必须继续存在。
        .get(root)
        // 缺失根属于事件生命周期错误。
        .expect("Wheel 后 VirtualScroll 根必须存在")
        // 遍历事件内已协调的新窗口。
        .children()
        // 逐项读取框架权威 key。
        .iter()
        // 忽略任何没有 key 的非法测试节点以聚焦身份结果。
        .filter_map(|id| tree.get(*id).and_then(|node| node.key()).map(str::to_owned))
        // 保存确定性窗口顺序。
        .collect::<Vec<_>>();
    // 新窗口不得继续保留旧索引零与一。
    assert!(!keys.iter().any(|key| key == "virtual-scroll-item:0"));
    // 同一事件必须已经物化滚动后的有效索引行。
    assert!(keys.iter().any(|key| key == "virtual-scroll-item:8"));
}

// 验证离场墓碑继续可绘制但不参与 VirtualScroll 活动计数与绝对索引布局。
#[test]
// 覆盖离场期间重复刷新、活动 frame 与同 key 重入取消离场。
fn virtual_scroll_leave_rows_do_not_shift_or_rebuild_active_window() {
    // 记录真实行工厂调用次数，检测离场墓碑是否触发重复协调。
    let render_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    // 克隆计数器供 static 行工厂拥有。
    let renderer_calls = Arc::clone(&render_calls);
    // 构造每行都带长离场动画的两行视口。
    let view = VirtualScroll::new()
        // 提供三个连续项目供窗口零一切换到一二。
        .item_count(3)
        // 使用十像素固定行高。
        .item_height(10.0)
        // 禁用 overscan 以隔离活动窗口。
        .overscan(0)
        // 固定两行高度的视口。
        .size(100.0, 20.0)
        // 按绝对索引构建带离场动画的行。
        .render(move |index| {
            // 记录每次真实行工厂调用。
            renderer_calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            // 返回框架设置 key 的唯一行根并声明非零离场动画。
            ViewNode::leaf(Label::new(format!("row-{index}")))
                // 长动画确保测试期间墓碑不会自然完成。
                .leave(crate::ui::Transition::fade_out(10.0))
        });
    // 初始建树物化索引零与一。
    let mut tree = ViewAdapter::build(view);
    // 运行一次布局以保存两行初始 frame。
    tree.layout();
    // 读取 VirtualScroll 根身份。
    let root = tree.root_id().expect("VirtualScroll 必须拥有根");
    // 保存即将离场的索引零组件身份。
    let row_zero = row_id(&tree, root, 0);
    // 保存重叠窗口中索引一的组件身份。
    let row_one = row_id(&tree, root, 1);
    // 保存离场前最后一帧供后续核对。
    let row_zero_frame = tree
        // 读取索引零节点。
        .get(row_zero)
        // 初始行必须存在。
        .expect("索引零初始行必须存在")
        // 复制离场前实际 frame。
        .frame();
    // 初始两行恰好调用两次 renderer。
    assert_eq!(render_calls.load(std::sync::atomic::Ordering::Relaxed), 2);
    // 把偏移推进一行，使活动窗口变为索引一与二。
    tree.get_mut(root)
        // 根必须仍可寻址。
        .expect("VirtualScroll 根必须存在")
        // 取得运行时组件。
        .component_mut()
        // 进入类型擦除可变接口。
        .as_any_mut()
        // 恢复 VirtualScroll 类型。
        .downcast_mut::<VirtualScroll>()
        // 根类型不得变化。
        .expect("根必须是 VirtualScroll")
        // 访问内部滚动偏移。
        .scroll_offset
        // 推进一行。
        .set(10.0);
    // 物化新窗口并让索引零进入离场。
    assert!(tree.refresh_virtual_scroll_component(root, Some(20.0)));
    // 新窗口会重新声明索引一与二两行。
    assert_eq!(render_calls.load(std::sync::atomic::Ordering::Relaxed), 4);
    // 离场墓碑必须仍在树中供动画绘制。
    assert!(tree
        .get(row_zero)
        .is_some_and(|node| node.pending_removal()));
    // 父子链包含一个墓碑和两个活动行。
    assert_eq!(tree.get(root).expect("根必须存在").children().len(), 3);
    // 相同窗口再次刷新不得把墓碑误计为活动行并重调 renderer。
    assert!(!tree.refresh_virtual_scroll_component(root, Some(20.0)));
    // renderer 调用次数保持不变。
    assert_eq!(render_calls.load(std::sync::atomic::Ordering::Relaxed), 4);
    // 对当前树执行真实布局以验证活动子集的绝对索引映射。
    tree.layout();
    // 离场墓碑继续保持其最后 frame。
    assert_eq!(
        tree.get(row_zero).expect("墓碑必须存在").frame(),
        row_zero_frame
    );
    // 索引一作为重叠活动行必须继续复用原组件身份。
    assert_eq!(row_id(&tree, root, 1), row_one);
    // 读取索引一活动 frame。
    let row_one_frame = tree
        .get(row_id(&tree, root, 1))
        .expect("索引一必须存在")
        .frame();
    // 读取索引二活动 frame。
    let row_two_frame = tree
        .get(row_id(&tree, root, 2))
        .expect("索引二必须存在")
        .frame();
    // 活动首行不应被离场墓碑挤到第二个绝对索引位置。
    assert!((row_one_frame.y - 0.0).abs() < 0.001);
    // 活动第二行应紧随首行位于十像素位置。
    assert!((row_two_frame.y - 10.0).abs() < 0.001);
    // 在离场完成前把窗口恢复到零一。
    tree.get_mut(root)
        // 根必须仍可寻址。
        .expect("VirtualScroll 根必须存在")
        // 取得运行时组件。
        .component_mut()
        // 进入类型擦除可变接口。
        .as_any_mut()
        // 恢复 VirtualScroll 类型。
        .downcast_mut::<VirtualScroll>()
        // 根类型不得变化。
        .expect("根必须是 VirtualScroll")
        // 访问内部滚动偏移。
        .scroll_offset
        // 恢复首个窗口。
        .set(0.0);
    // 同 key 重入必须协调回当前活动窗口。
    assert!(tree.refresh_virtual_scroll_component(root, Some(20.0)));
    // 索引零必须复用原组件身份而非分配新节点。
    assert_eq!(row_id(&tree, root, 0), row_zero);
    // 重入会取消尚未完成的离场状态。
    assert!(!tree
        .get(row_zero)
        .expect("重入行必须存在")
        .pending_removal());
}

// 验证树关闭会释放 VirtualScroll sidecar 闭包捕获的应用资源。
#[test]
// 使用 Weak 观察重复 shutdown 前后的 renderer 资源生命周期。
fn virtual_scroll_shutdown_releases_renderer_captures() {
    // 创建只由测试与 key 闭包共享的资源。
    let resource = Arc::new(());
    // 保存弱引用以观察 sidecar 是否真正释放强所有权。
    let weak_resource = Arc::downgrade(&resource);
    // 克隆资源供业务键工厂长期持有。
    let renderer_resource = Arc::clone(&resource);
    // 构造最小业务 keyed 虚拟列表。
    let view = VirtualScroll::new()
        // 只需一个物化项目。
        .item_count(1)
        // 使用有效固定行高。
        .item_height(10.0)
        // 固定一行视口。
        .size(100.0, 10.0)
        // 让 key 闭包持有应用资源直到 sidecar 释放。
        .render_keyed(
            // 业务键工厂读取持有资源但不泄露其内容。
            move |index| {
                // 保持闭包对应用资源的强所有权。
                let _ = &renderer_resource;
                // 返回当前索引作为稳定业务键。
                index
            },
            // 行工厂返回无根 key 的唯一声明节点。
            |index| ViewNode::leaf(Label::new(format!("row-{index}"))),
        );
    // 建树后 renderer sidecar 接管两个应用闭包。
    let mut tree = ViewAdapter::build(view);
    // 释放测试本地的最后一个显式强引用。
    drop(resource);
    // sidecar 尚在时资源必须仍然存活。
    assert!(weak_resource.upgrade().is_some());
    // 第一次关闭释放所有延迟 renderer sidecar。
    tree.shutdown();
    // renderer 闭包捕获资源必须立即释放。
    assert!(weak_resource.upgrade().is_none());
    // 重复关闭必须保持幂等且不得复活资源。
    tree.shutdown();
    // 资源仍然没有任何强引用。
    assert!(weak_resource.upgrade().is_none());
}
