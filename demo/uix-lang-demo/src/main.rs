// 导入应用组合根、ViewNode 与 uix-lang 编译期入口。
use std::collections::HashSet;
// 导入窗口级句柄静态槽与互斥所有权。
use std::sync::{Mutex, OnceLock};
// 导入秒级计时器时长。
use std::time::Duration;

use uix::prelude::*;

// 保存窗口级持久状态与 uix-lang 语言面之外的类型化状态与数据。
// 大部分交互状态已迁入 main.uix 的组件私有 state；这里只保留三类：
// 1) 窗口生命周期状态（页面、首页计数与秒级 tick）；
// 2) 无法用声明语法初始化的类型化状态（集合、日期、时间、颜色等）；
// 3) 由 Rust 侧构造的类型化演示数据。
#[derive(Clone)]
struct DemoStates {
    // 当前页面索引（与 DemoApp 的 State<number> prop 共享同一槽）。
    page: State<f64>,
    // 首页窗口级计数状态。
    count: State<f64>,
    // App.on_start 注册的秒级 tick 状态。
    tick: State<f64>,
    // 多选下拉集合状态。
    cities: State<HashSet<String>>,
    // 级联选择完整路径状态。
    region: State<CascaderValue>,
    // 日期选择状态。
    selected_date: State<Date>,
    // 日期范围起点状态。
    range_start_date: State<Date>,
    // 日期范围终点状态。
    range_end_date: State<Date>,
    // 时间选择状态。
    selected_time: State<Time>,
    // 颜色选择状态。
    selected_color: State<Color>,
    // 滚动容器偏移双向绑定状态。
    scroll_point: State<Point>,
    // 类型化表单模型状态。
    profile: State<DemoProfile>,
    // 单选组候选集合。
    gender_options: Vec<String>,
    // 分段控制器候选集合。
    view_options: Vec<String>,
    // 下拉选择器结构化选项。
    city_options: Vec<SelectOption>,
    // 级联选择器选项树。
    region_options: Vec<CascaderOption>,
    // 自动完成候选集合。
    suggestions: Vec<String>,
    // 提及输入候选集合。
    mention_suggestions: Vec<String>,
    // 表单等级选择候选集合。
    level_options: [&'static str; 3],
    // 表单通知渠道候选集合。
    channel_options: [&'static str; 2],
    // 描述列表类型化数据。
    description_items: Vec<DescriptionsItem>,
    // 时间轴类型化事件集合。
    timeline_items: Vec<TimelineItem>,
    // 面包屑类型化路径数据。
    breadcrumb_items: Vec<BreadcrumbItem>,
    // 锚点导航类型化目标集合。
    anchor_items: Vec<AnchorItem>,
    // 步骤条类型化步骤集合。
    steps_items: Vec<Step>,
    // 卡片操作项字符串集合。
    card_actions: Vec<&'static str>,
    // 列表组件文本数据。
    list_items: [&'static str; 3],
    // 图片组路径集合。
    gallery_images: [&'static str; 2],
    // 虚拟滚动惰性行数据。
    virtual_items: Vec<DemoRow>,
}

// 表示虚拟滚动演示的拥有所有权行数据。
#[derive(Clone)]
struct DemoRow {
    // 保存稳定行身份。
    id: u64,
    // 保存可插值行内容。
    name: String,
}

// 表示 Form 演示使用的类型化业务模型。
#[derive(Clone)]
struct DemoProfile {
    // 保存邮箱字段。
    email: String,
    // 保存等级选择字段。
    level: String,
    // 保存协议确认布尔字段。
    accepted: bool,
    // 保存通知渠道单选组字段。
    channel: String,
    // 保存通知开关布尔字段。
    notifications: bool,
    // 保存音量滑块 f64 字段。
    volume: f64,
}

// 保存 setTheme 内置操作读取的窗口级 AppHandle。
static APP_HANDLE: OnceLock<Mutex<Option<AppHandle>>> = OnceLock::new();

// 实现 uix-lang setTheme 内置操作的 Rust 侧真实主题切换。
// 组件体内的字符串字面量会规范化为拥有所有权的 String。
#[allow(non_snake_case, reason = "uix-lang 内置操作名固定为 setTheme")]
fn setTheme(name: String) {
    // 读取 on_start 注册的窗口级句柄。
    let handle = APP_HANDLE
        // 读取已初始化的静态槽。
        .get()
        // 取得互斥锁并忽略中毒状态。
        .and_then(|slot| slot.lock().unwrap_or_else(|e| e.into_inner()).clone());
    // 未注册句柄时保持声明式行为无副作用。
    let Some(handle) = handle else {
        // 结束当前调用。
        return;
    };
    // 按名称选择演示主题。
    let theme = match name.as_str() {
        // 暗色主题。
        "dark" => Theme::antd_dark(),
        // 其余名称回到亮色主题。
        _ => Theme::antd_light(),
    };
    // 通过公开句柄切换全局主题。
    let _ = handle.set_theme(theme);
}

// 记录 Input @change 的文本载荷。
fn on_name_change(value: &str) {
    // 输出变化文本供演示日志核对。
    tracing::info!(value, "uix-lang Input @change");
}

// 记录 ImageGroup @change 的索引文本载荷。
fn on_gallery_change(index: &str) {
    // 输出变化索引供演示日志核对。
    tracing::info!(index, "uix-lang ImageGroup @change");
}

// 记录 Pagination @change 的页码文本载荷。
fn on_page_change(value: &str) {
    // 输出变化页码供演示日志核对。
    tracing::info!(value, "uix-lang Pagination @change");
}

// 记录 Anchor @change 的 href 文本载荷。
fn on_anchor_change(href: &str) {
    // 输出目标锚点供演示日志核对。
    tracing::info!(href, "uix-lang Anchor @change");
}

// 记录 Alert @close 的关闭载荷。
fn on_alert_closed(value: &str) {
    // 输出关闭载荷供演示日志核对。
    tracing::info!(value, "uix-lang Alert @close");
}

// 提交类型化表单模型并返回空错误。
fn submit_profile(model: DemoProfile) -> Result<(), String> {
    // 输出关键字段供演示日志核对。
    tracing::info!(
        email = %model.email,
        level = %model.level,
        accepted = model.accepted,
        channel = %model.channel,
        notifications = model.notifications,
        volume = model.volume,
        "uix-lang Form @submit"
    );
    // 演示提交始终成功。
    Ok(())
}

// 构造窗口级持久状态与全部演示数据。
impl DemoStates {
    // 创建默认演示状态槽。
    fn new() -> Self {
        Self {
            // 从首页开始。
            page: State::new(0.0_f64),
            // 首页计数从零开始。
            count: State::new(0.0_f64),
            // 全局 tick 从零开始。
            tick: State::new(0.0_f64),
            // 多选下拉初始空集合。
            cities: State::new(HashSet::<String>::new()),
            // 级联选择初始空路径。
            region: State::new(CascaderValue {
                // 初始显示路径为空。
                labels: Vec::new(),
                // 初始稳定值路径为空。
                values: Vec::new(),
            }),
            // 初始选中日期。
            selected_date: State::new(Date::new(2026, 8, 10)),
            // 初始范围起点。
            range_start_date: State::new(Date::new(2026, 8, 1)),
            // 初始范围终点。
            range_end_date: State::new(Date::new(2026, 8, 31)),
            // 初始选中时间。
            selected_time: State::new(Time::new(14, 30)),
            // 初始选中颜色。
            selected_color: State::new(Color::hex("#1677ff")),
            // 滚动偏移初始零。
            scroll_point: State::new(Point::new(0.0, 0.0)),
            // 表单模型初始值。
            profile: State::new(DemoProfile {
                // 提供通过邮箱规则的初始值。
                email: "owner@example.com".to_string(),
                // 提供已包含在候选集合中的初始等级。
                level: "中级".to_string(),
                // 提供已勾选的协议确认状态。
                accepted: true,
                // 提供已包含在单选组候选集合中的初始值。
                channel: "邮件".to_string(),
                // 提供已开启的通知初始值。
                notifications: true,
                // 提供位于声明范围内的音量初始值。
                volume: 35.0,
            }),
            // 单选组候选集合。
            gender_options: vec![String::from("female"), String::from("male")],
            // 分段控制器候选集合。
            view_options: vec![String::from("grid"), String::from("list")],
            // 下拉选择器结构化选项。
            city_options: vec![
                // 中文文案绑定稳定地区代码。
                SelectOption::new("中国", "cn"),
                // 另一项用于覆盖多个结构化选项。
                SelectOption::new("美国", "us"),
                // 第三项保持多选演示集合非平凡。
                SelectOption::new("日本", "jp"),
            ],
            // 级联选择器选项树。
            region_options: vec![
                // 省级根项包含城市叶项。
                CascaderOption::new("浙江", "zj").children(vec![
                    // 城市叶项提交完整路径。
                    CascaderOption::new("杭州", "hz"),
                    // 第二个城市叶项。
                    CascaderOption::new("宁波", "nb"),
                ]),
                // 第二个省级根项。
                CascaderOption::new("江苏", "js").children(vec![
                    // 城市叶项。
                    CascaderOption::new("南京", "nj"),
                ]),
            ],
            // 自动完成候选集合。
            suggestions: vec![
                String::from("apple"),
                String::from("apricot"),
                String::from("application"),
            ],
            // 提及输入候选集合。
            mention_suggestions: vec![String::from("alice"), String::from("adam")],
            // 表单等级选择候选集合。
            level_options: ["初级", "中级", "高级"],
            // 表单通知渠道候选集合。
            channel_options: ["邮件", "短信"],
            // 描述列表类型化数据。
            description_items: vec![
                // 创建第一项公开描述数据。
                DescriptionsItem::new("姓名", "Ada"),
                // 创建第二项公开描述数据。
                DescriptionsItem::new("角色", "管理员"),
                // 创建第三项公开描述数据。
                DescriptionsItem::new("邮箱", "ada@ex.com"),
            ],
            // 时间轴类型化事件集合。
            timeline_items: vec![
                // 首条时间轴事件。
                TimelineItem::new("创建").description("2026-08-01"),
                // 中间时间轴事件。
                TimelineItem::new("设计").description("2026-08-05"),
                // 上线时间轴事件。
                TimelineItem::new("上线").description("2026-08-10"),
            ],
            // 面包屑类型化路径数据。
            breadcrumb_items: vec![
                // 首级面包屑。
                BreadcrumbItem::new("首页"),
                // 中间级面包屑。
                BreadcrumbItem::new("组件"),
                // 末级面包屑。
                BreadcrumbItem::new("导航").active(),
            ],
            // 锚点导航类型化目标集合。
            anchor_items: vec![
                // 基础锚点。
                AnchorItem::new("基础", "#basic"),
                // 高级锚点。
                AnchorItem::new("高级", "#advanced"),
                // API 锚点。
                AnchorItem::new("API", "#api"),
            ],
            // 步骤条类型化步骤集合。
            steps_items: vec![
                // 已完成步骤。
                Step::new("注册").status(StepStatus::Finish),
                // 进行中步骤。
                Step::new("验证").status(StepStatus::Process),
                // 等待中步骤。
                Step::new("完成").status(StepStatus::Wait),
            ],
            // 卡片操作项字符串集合。
            card_actions: vec!["编辑", "删除"],
            // 列表组件文本数据。
            list_items: ["视觉基线", "语义断言", "交互回归"],
            // 图片组路径集合。
            gallery_images: ["assets/images/demo.png", "assets/images/demo.png"],
            // 虚拟滚动惰性行数据。
            virtual_items: vec![
                // 首行。
                DemoRow { id: 1, name: "惰性行 1".to_string() },
                // 第二行。
                DemoRow { id: 2, name: "惰性行 2".to_string() },
                // 第三行。
                DemoRow { id: 3, name: "惰性行 3".to_string() },
                // 第四行。
                DemoRow { id: 4, name: "惰性行 4".to_string() },
                // 第五行。
                DemoRow { id: 5, name: "惰性行 5".to_string() },
                // 第六行。
                DemoRow { id: 6, name: "惰性行 6".to_string() },
            ],
        }
    }
}

// 声明由应用组合根调用并接收窗口级持久状态的界面构造函数。
fn build_view(states: DemoStates) -> ViewNode {
    // 一次按值解构把声明文件引用的全部 Rust 绑定放入函数作用域。
    let DemoStates {
        // 取出窗口级生命周期状态句柄。
        page,
        count,
        tick,
        // 取出语言面之外的类型化状态句柄。
        cities,
        region,
        selected_date,
        range_start_date,
        range_end_date,
        selected_time,
        selected_color,
        scroll_point,
        profile,
        // 取出由 Rust 侧构造的类型化数据绑定。
        gender_options,
        view_options,
        city_options,
        region_options,
        suggestions,
        mention_suggestions,
        level_options,
        channel_options,
        description_items,
        timeline_items,
        breadcrumb_items,
        anchor_items,
        steps_items,
        card_actions,
        list_items,
        gallery_images,
        virtual_items,
    } = states;
    // TreeNode 内部持有非 Send 的 Rc 回调，只能在 UI 线程构建。
    let tree_nodes = vec![
        // 根节点包含一个可选成员叶节点。
        TreeNode::new("部门", "dept").children(vec![
            // 叶节点的稳定 key 与初始受控状态一致。
            TreeNode::new("成员", "member"),
            // 第二个叶节点。
            TreeNode::new("访客", "guest"),
        ]),
        // 第二个根节点。
        TreeNode::new("项目", "project"),
    ];
    // 编译期读取相对当前 crate 清单目录的 uix-lang 文件。
    uix!("src/main.uix")
    // 结束界面构造函数。
}

// 启动由 Rust 持有窗口与运行生命周期的演示应用。
fn main() {
    // 安装环境过滤日志订阅器（UIX 日志域默认 info）。
    init_tracing();
    // 创建窗口级持久状态槽与演示数据。
    let states = DemoStates::new();
    // 克隆 tick 句柄供 on_start 秒级计时器更新。
    let tick = states.tick.clone();
    // 声明式文档展开出较深视图树，按平台入口执行窗口事件循环。
    #[cfg(windows)]
    {
        // Windows 主线程默认栈较小；深层 ViewNode 树的构建、协调与布局递归
        // 需要与 API GUI Demo 一致的有界大栈 UI 线程。
        const WINDOWS_GUI_STACK_BYTES: usize = 8 * 1024 * 1024;
        // 在专用大栈线程上运行声明式演示窗口。
        let ui_thread = match std::thread::Builder::new()
            .name("uix-lang-demo-gui".to_string())
            .stack_size(WINDOWS_GUI_STACK_BYTES)
            .spawn(move || run_gui(states.clone(), tick.clone()))
        {
            // 返回已创建的 UI 线程。
            Ok(ui_thread) => ui_thread,
            // 线程创建失败直接失败退出。
            Err(error) => panic!("spawn Windows UI thread: {error}"),
        };
        // 把子线程 panic 恢复到主线程。
        if let Err(payload) = ui_thread.join() {
            std::panic::resume_unwind(payload);
        }
    }

    // 其他平台沿用主线程入口。
    #[cfg(not(windows))]
    run_gui(states, tick);
    // 结束进程入口。
}

// 组装并运行 uix-lang 演示窗口。
fn run_gui(states: DemoStates, tick: State<f64>) {
    // 组装演示 App 与窗口级句柄。
    let app = App::new()
        // 设置演示窗口标题。
        .title("UIX Demo")
        // 使用与 API GUI Demo 一致的初始窗口尺寸。
        .size(1200, 800)
        // 由声明式根视图绘制与 API GUI Demo 一致的自定义标题栏。
        .custom_title_bar(true)
        // 启动后注册句柄与秒级计时器。
        .on_start(move |handle| {
            // 把窗口句柄写入 setTheme 内置操作读取的静态槽。
            let slot = APP_HANDLE.get_or_init(|| Mutex::new(None));
            // 写入或替换当前演示窗口句柄。
            *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle.clone());
            // 秒级计数器驱动声明文件中的 tick 展示。
            let ticks = tick.clone();
            // 注册一秒间隔的演示计时器。
            handle
                .run_interval(Duration::from_secs(1), move || {
                    // 每次间隔推进全局 tick。
                    ticks.update(|value| *value += 1.0);
                })
                .detach();
        });
    // 把捕获持久状态句柄的 ViewNode 工厂交给应用组合根。
    app.root(move || {
        // 每次 reconcile 只复制句柄，底层状态槽保持不变。
        build_view(states.clone())
        // 结束声明式根构造闭包。
    })
    // 进入并由 App 持有原生窗口事件循环。
    .run();
}

// 初始化演示程序使用的环境过滤日志订阅器。
fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    // 优先读取 RUST_LOG，缺省时只显示 uix 日志域的 info 级。
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("uix=info"));
    // 订阅器初始化失败不影响演示运行。
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
