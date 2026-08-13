// 导入父模块公开给适配器回归的声明树协调入口。
use super::ViewAdapter;
// 导入动态日期格在真实组件私有状态作用域中申请状态的隐藏入口。
use crate::ui::component_state::{uix_component_scope, uix_component_state};
// 导入构造日历标题栏真实指针命中坐标所需的基础点类型。
use crate::core::Point;
// 导入读取日历运行时直接子节点数量所需的树节点核心接口。
use crate::ui::component::widget::WidgetCore;
// 导入构造真实日历 View 工厂输出所需的声明节点类型。
use crate::ui::view::ViewNode;
// 导入日历、日期、直接入口宿主与最小动态日期格标签组件。
use crate::ui::widgets::{Calendar, Container, Date, Label};
// 导入日期格动画、Effect 与真实指针导航事件类型。
use crate::ui::{Animated, Easing, Effect, EventResult, KeyMod, MouseButton};
// 导入响应式状态、系统事件与运行时树类型。
use crate::ui::{State, SystemEvent, WidgetTree};
// 导入按日期记录工厂最近一次状态句柄的映射。
use std::collections::HashMap;
// 导入在静态日期格工厂与断言之间共享记录所需的所有权类型。
use std::sync::{Arc, Mutex};

// 定义按稳定日期格式记录的组件私有状态观察表。
type CapturedStates = Arc<Mutex<HashMap<String, State<i32>>>>;

// 构造把每个实际日期格状态回传给测试的真实 Calendar 组件。
fn calendar_component(
    // 接收日历初始展示年份。
    year: i32,
    // 接收日历初始展示月份。
    month: usize,
    // 接收由工厂长期共享的日期状态观察表。
    captured_states: &CapturedStates,
    // 返回可交给声明树或直接 WidgetTree 入口的 Calendar 组件。
) -> Calendar {
    // 克隆观察表以交给日历长期持有的日期格工厂。
    let factory_states = Arc::clone(captured_states);
    // 创建从调用方指定月份开始展示的日历。
    Calendar::new()
        // 固定首轮月份，使日期格集合和导航迁移可预测。
        .default_displayed(year, month)
        // 让每个日期通过生产日期格工厂创建自己的声明子树。
        .date_cell(move |date, _info| {
            // 为同一静态日期格组件调用申请由动态捕获命名空间消歧的作用域。
            let scope = uix_component_scope("calendar-dynamic-capture-test", 1);
            // 在日期业务键限定的树级存储中取得或初始化私有字段状态。
            let state = uix_component_state(&scope, 1, || 0_i32);
            // 克隆日期私有状态供本日期格 Effect 建立依赖。
            let effect_state = state.clone();
            // 创建由实际日期格节点拥有的响应式 Effect。
            let _effect = Effect::new(move || {
                // 读取日期状态以让后续写入调度所属节点 Effect。
                let _ = effect_state.get();
            });
            // 创建保持活动的日期格动画源以验证动态节点动画所有权。
            let animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
            // 在捕获边界内读取动画值，把来源显式交给日期格节点。
            let _ = animation.value();
            // 锁定观察表仅覆盖本次工厂同步回传。
            factory_states
                // 进入共享状态记录。
                .lock()
                // 即使先前断言 panic 也恢复记录以便继续给出诊断。
                .unwrap_or_else(|error| error.into_inner())
                // 使用规范日期键覆盖当前工厂取得的状态句柄。
                .insert(date.format(), state);
            // 返回由实际挂载节点承接组件作用域的最小日期格 View。
            ViewNode::leaf(Label::new(date.format()))
                // 让树释放日期格时同时释放其私有状态租约。
                .uix_component_scope(scope, 0)
        })
}

// 构造可交给 ViewAdapter 建树或协调的 Calendar 声明根。
fn calendar_view(
    // 接收日历初始展示年份。
    year: i32,
    // 接收日历初始展示月份。
    month: usize,
    // 接收由工厂长期共享的日期状态观察表。
    captured_states: &CapturedStates,
    // 返回日期格由运行时动态拥有的声明节点。
) -> ViewNode {
    // 把同一 Calendar 组件包装成声明树叶节点。
    ViewNode::leaf(calendar_component(year, month, captured_states))
}

// 按日期从工厂观察表读取最近一次已捕获的私有状态句柄。
fn captured_state(
    // 接收由日期格工厂持续写入的状态观察表。
    captured_states: &CapturedStates,
    // 接收目标日期的稳定业务身份。
    date: Date,
    // 返回该日期本轮实际工厂取得的状态句柄。
) -> State<i32> {
    // 锁定观察表仅覆盖句柄克隆操作。
    captured_states
        // 进入测试共享记录。
        .lock()
        // 即使先前断言 panic 也恢复记录以便继续诊断。
        .unwrap_or_else(|error| error.into_inner())
        // 按工厂使用的规范日期格式读取状态。
        .get(&date.format())
        // 脱离锁后保留公开状态句柄。
        .cloned()
        // 已物化日期格必须已同步执行其工厂。
        .expect("已物化日期格必须返回组件私有状态")
}

// 读取日历实际挂载的直接日期格宿主数量。
fn calendar_child_count(
    // 接收真实运行时组件树。
    tree: &WidgetTree,
    // 接收日历根组件身份。
    root: crate::ui::ComponentId,
    // 返回日历当前实际物化的日期格宿主数量。
) -> usize {
    // 根日历在整个测试期间必须仍可寻址。
    tree.get(root)
        // 报告构建或协调意外移除根节点的明确错误。
        .expect("日历根节点必须存在")
        // 读取日历当前直接子节点。
        .children()
        // 返回实际日期格宿主总数。
        .len()
}

// 向运行时日历标题栏的上一个或下一个导航命中区发送一次真实左键事件。
fn navigate_calendar(
    // 接收待导航的真实运行时树。
    tree: &mut WidgetTree,
    // 接收日历根组件身份以读取本轮实际控制区域。
    root: crate::ui::ComponentId,
    // 指示本次导航是否前往下一个月。
    next: bool,
) {
    // 从真实运行时日历读取布局后控制区域，避免测试依赖固定像素尺寸。
    let control = tree
        // 读取当前仍挂载的日历根节点。
        .get(root)
        // 日历导航期间根节点必须保持存在。
        .expect("日历导航必须拥有根节点")
        // 取得节点实际组件接口。
        .component()
        // 擦除动态类型以恢复 Calendar。
        .as_any()
        // 读取测试专用 Calendar 引用。
        .downcast_ref::<Calendar>()
        // 根声明必须仍是 Calendar。
        .expect("日历根节点必须保持 Calendar 类型")
        // 读取最近一次布局生成的真实控制区域。
        .control_rect_for_test()
        // 导航前已执行布局，控制区域必须可用。
        .expect("布局后的日历必须拥有控制区域");
    // 根据导航方向选择标题栏最左或最右的安全内部坐标。
    let x = if next {
        // 下月按钮位于控制区域最右侧。
        control.x + control.w - 1.0
    } else {
        // 上月按钮位于控制区域最左侧。
        control.x + 1.0
    };
    // 标题栏从控制区域顶部开始，取内部一点规避边界判定。
    let position = Point::new(x, control.y + 1.0);
    // 通过公开系统事件入口模拟实际用户导航。
    let result = tree.dispatch_event(
        // 构造无修饰键的左键按下事件。
        &SystemEvent::PointerDown {
            // 使用调用方指定的标题栏导航坐标。
            pos: position,
            // 使用日历支持的主鼠标按键。
            button: MouseButton::Left,
            // 测试不附加键盘修饰键。
            mods: KeyMod::NONE,
        },
    );
    // 导航命中必须由日历接收并同步刷新日期格子树。
    assert_eq!(result, EventResult::Handled);
}

// 验证首次建树立即物化日期格，并在同日期协调中保留每个日期独立的私有状态。
#[test]
// 执行首轮 materialization、同日期重建与不同日期隔离回归。
fn calendar_dynamic_capture_materializes_and_preserves_same_date_state() {
    // 创建由日期格工厂写入的空状态观察表。
    let captured_states = Arc::new(Mutex::new(HashMap::new()));
    // 用带自定义日期格的七月日历执行首次真实建树。
    let mut tree = ViewAdapter::build_nodes(calendar_view(2026, 7, &captured_states));
    // 取得实际日历根身份以观察其直接动态子节点。
    let root = tree.root_id().expect("日历测试必须拥有根节点");
    // 七月首次建树必须立即物化全部三十一个日期格。
    assert_eq!(calendar_child_count(&tree, root), 31);
    // 每个已物化日期格必须各自登记一条节点动画源。
    assert_eq!(tree.animated_source_registrations().len(), 31);
    // 取得七月一日首次工厂创建的私有状态。
    let july_first = captured_state(&captured_states, Date::new(2026, 7, 1));
    // 取得七月二日首次工厂创建的私有状态。
    let july_second = captured_state(&captured_states, Date::new(2026, 7, 2));
    // 写入首日独有的状态值。
    july_first.set(41);
    // 写入次日独有的状态值。
    july_second.set(92);

    // 以相同展示日期重新协调日历，模拟父 View 常规刷新。
    ViewAdapter::reconcile_nodes(&mut tree, calendar_view(2026, 7, &captured_states));
    // 同日期协调不得改变实际日期格数量。
    assert_eq!(calendar_child_count(&tree, root), 31);
    // 重新读取同一日期本轮工厂取得的状态句柄。
    let preserved_first = captured_state(&captured_states, Date::new(2026, 7, 1));
    // 同一日期业务键必须复用其树拥有的私有状态值。
    assert_eq!(preserved_first.get(), 41);
    // 重新读取相邻日期本轮工厂取得的状态句柄。
    let preserved_second = captured_state(&captured_states, Date::new(2026, 7, 2));
    // 相邻日期业务键必须保留各自独立的私有状态值。
    assert_eq!(preserved_second.get(), 92);
}

// 验证翻月真实移除日期格后，返回旧月会为已释放日期重新初始化私有状态。
#[test]
// 执行跨月移除、隔离与重新进入重建回归。
fn calendar_dynamic_capture_releases_removed_month_and_rebuilds_on_return() {
    // 创建由日期格工厂写入的空状态观察表。
    let captured_states = Arc::new(Mutex::new(HashMap::new()));
    // 用带自定义日期格的七月日历执行首次真实建树。
    let mut tree = ViewAdapter::build_nodes(calendar_view(2026, 7, &captured_states));
    // 取得实际日历根身份以执行真实布局与指针导航。
    let root = tree.root_id().expect("日历测试必须拥有根节点");
    // 取得即将在翻月时被真实移除的七月一日状态句柄。
    let removed_july_first = captured_state(&captured_states, Date::new(2026, 7, 1));
    // 写入七月一日状态以验证日期格 Effect 已由树接管。
    removed_july_first.set(71);
    // 活着的七月日期格 Effect 必须被状态写入置为待执行。
    assert!(tree.has_pending_effects());
    // 执行并清除本轮日期格 Effect，建立翻月前的稳定基线。
    assert!(tree.tick_effects());
    // 完成根布局，使树的命中测试与日期格 frame 使用真实几何。
    tree.layout();

    // 点击标题栏右侧的下月导航按钮进入八月。
    navigate_calendar(&mut tree, root, true);
    // 读取八月一日首次物化得到的私有状态。
    let august_first = captured_state(&captured_states, Date::new(2026, 8, 1));
    // 八月首次进入必须创建独立的日期格私有状态。
    assert_eq!(august_first.get(), 0);
    // 八月拥有三十一个实际日期格宿主。
    assert_eq!(calendar_child_count(&tree, root), 31);
    // 七月动画源必须随真实移除释放，树中只保留八月三十一条来源。
    assert_eq!(tree.animated_source_registrations().len(), 31);
    // 修改已移除日期的旧 State 不得再唤醒任何日期格 Effect。
    removed_july_first.set(72);
    // 七月 Effect 已随节点释放，旧状态写入不得产生待执行副作用。
    assert!(!tree.has_pending_effects());
    // 重新布局以使用八月刷新后的真实标题栏命中区域。
    tree.layout();

    // 点击标题栏左侧的上月导航按钮返回七月。
    navigate_calendar(&mut tree, root, false);
    // 读取七月重新物化后返回的新状态句柄。
    let remounted_july_first = captured_state(&captured_states, Date::new(2026, 7, 1));
    // 返回已真实移除的七月时必须获得重新初始化的状态而非旧值。
    assert_eq!(remounted_july_first.get(), 0);
    // 返回七月后仍须实际物化完整三十一个日期格宿主。
    assert_eq!(calendar_child_count(&tree, root), 31);
    // 八月动画源同样必须释放，返回后只保留新七月日期格来源。
    assert_eq!(tree.animated_source_registrations().len(), 31);
}

// 验证非 Calendar 与陈旧 owner 会被动态日期格刷新入口安全拒绝。
#[test]
// 执行 owner 类型与 generation 的迟到刷新门禁回归。
fn calendar_dynamic_capture_rejects_non_calendar_and_stale_owner() {
    // 建立不拥有日期格工厂的普通标签根树。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Label::new("ordinary")));
    // 保存当前标签根身份以覆盖类型错误和后续 generation 失效场景。
    let stale_owner = tree.root_id().expect("测试树必须拥有标签根节点");
    // 非 Calendar owner 不得触发捕获断言或动态子树改写。
    assert!(!tree.refresh_calendar_cell_component(stale_owner));
    // 完整换根使先前 ComponentId 的 generation 失效。
    tree.set_root(Box::new(Label::new("replacement")));
    // 陈旧 owner 的迟到刷新必须静默拒绝而不是升级成 panic。
    assert!(!tree.refresh_calendar_cell_component(stale_owner));
}

// 验证声明从自定义日期格切回普通 Calendar 时会真实清理旧动态子树。
#[test]
// 执行 custom 到 plain 的日期格所有权释放回归。
fn calendar_dynamic_capture_removes_custom_children_when_factory_is_disabled() {
    // 建立日期格工厂状态观察表。
    let captured_states = Arc::new(Mutex::new(HashMap::new()));
    // 用七月自定义日期格执行首次真实建树。
    let mut tree = ViewAdapter::build_nodes(calendar_view(2026, 7, &captured_states));
    // 保存 Calendar 根身份以核对原位协调后的直接子树。
    let root = tree.root_id().expect("日历测试必须拥有根节点");
    // 初始自定义日期格必须完整物化。
    assert_eq!(calendar_child_count(&tree, root), 31);
    // 协调为相同月份但不再提供 date_cell 工厂的普通 Calendar。
    ViewAdapter::reconcile_nodes(
        // 在同一真实树上发布声明变化。
        &mut tree,
        // 保留 Calendar 类型与月份，仅关闭动态日期格扩展。
        ViewNode::leaf(Calendar::new().default_displayed(2026, 7)),
    );
    // 旧日期格宿主必须被真实移除，不能与普通 Calendar 绘制并存。
    assert_eq!(calendar_child_count(&tree, root), 0);
    // 所有旧日期格动画源也必须随真实节点移除释放。
    assert!(tree.animated_source_registrations().is_empty());
}

// 验证 WidgetTree 公开直接换根与加子节点入口同样立即物化 Calendar 日期格。
#[test]
// 执行绕过 ViewAdapter 的两条公开建树入口回归。
fn calendar_dynamic_capture_materializes_through_direct_tree_entrypoints() {
    // 建立直接换根入口使用的日期状态观察表。
    let root_states = Arc::new(Mutex::new(HashMap::new()));
    // 建立尚无运行时节点的空树。
    let mut tree = WidgetTree::new();
    // 直接把自定义日期格 Calendar 设为根组件。
    let calendar_root = tree.set_root(Box::new(calendar_component(2026, 7, &root_states)));
    // 直接换根返回前必须已经物化七月全部日期格。
    assert_eq!(calendar_child_count(&tree, calendar_root), 31);
    // 直接入口也必须执行树级私有状态捕获。
    assert_eq!(captured_state(&root_states, Date::new(2026, 7, 1)).get(), 0);
    // 直接入口必须把每个日期格动画来源交给实际节点所有权。
    assert_eq!(tree.animated_source_registrations().len(), 31);

    // 用普通容器完整替换前一个 Calendar 根，建立 add_child 宿主。
    let container_root = tree.set_root(Box::new(Container::new()));
    // 建立直接加子节点入口使用的独立日期状态观察表。
    let child_states = Arc::new(Mutex::new(HashMap::new()));
    // 直接向真实父节点添加另一个自定义日期格 Calendar。
    let calendar_child = tree.add_child(
        // 使用当前普通容器根作为父节点。
        container_root,
        // 让直接入口拥有完整 Calendar 组件而非声明节点。
        Box::new(calendar_component(2026, 8, &child_states)),
    );
    // 直接加子节点返回前必须已经物化八月全部日期格。
    assert_eq!(calendar_child_count(&tree, calendar_child), 31);
    // 子 Calendar 的日期状态必须归属同一 WidgetTree 但隔离于旧根。
    assert_eq!(
        captured_state(&child_states, Date::new(2026, 8, 1)).get(),
        0
    );
    // 旧根动画已释放，当前只保留子 Calendar 的三十一条来源。
    assert_eq!(tree.animated_source_registrations().len(), 31);
}
