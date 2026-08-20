// 引入 Breadcrumb 公开值与组件契约。
use super::{Breadcrumb, BreadcrumbItem};
// 引入测试点击坐标。
use crate::core::Point;
// 引入事件与语义事件 trait。
use crate::ui::widget_runtime::traits::EventHandler;
// 引入事件身份、输入和载荷类型。
use crate::ui::{KeyMod, MouseButton, SemanticPayload, SystemEvent, WidgetId};

// 构造点击首个可见面包屑条目的输入。
fn click_first_item() -> SystemEvent {
    // 返回落在首个条目本地矩形内的左键按下事件。
    SystemEvent::PointerDown {
        // 首项从原点开始，使用稳定内部坐标。
        pos: Point::new(1.0, 1.0),
        // 使用组件登记的激活鼠标键。
        button: MouseButton::Left,
        // 不施加额外修饰键。
        mods: KeyMod::NONE,
    }
}

// 提取一次已建立选择事实的文本载荷。
fn selection_payload(breadcrumb: &Breadcrumb, event: &SystemEvent) -> String {
    // 从组件窄语义输出取得 Change 事件。
    let semantic = breadcrumb
        // 使用稳定测试组件身份读取待发事实。
        .semantic_event(WidgetId::default(), event)
        // 点击有效条目必须建立选择事实。
        .expect("有效 Breadcrumb 点击应产生 Change");
    // 只接受 Breadcrumb 登记的文本载荷。
    match semantic.payload {
        // 返回拥有型文本。
        SemanticPayload::Text(value) => value,
        // 其他载荷形状表示公开契约回归。
        _ => panic!("Breadcrumb Change 应使用文本载荷"),
    }
}

// 验证 link 值与末项当前页构建契约。
#[test]
// 声明条目值和末项激活测试。
fn link_and_last_active_contract_is_stable() {
    // 空集合调用末项激活必须保持安全。
    let empty = Breadcrumb::new().last_active();
    // 空集合没有稳定链接。
    assert_eq!(empty.active_link(), None);

    // 构造两个带稳定路径的条目并声明末项当前。
    let breadcrumb = Breadcrumb::new()
        // 注入拥有型条目集合。
        .items(vec![
            // 首项使用首页路径。
            BreadcrumbItem::new("首页").link("/"),
            // 末项使用详情路径。
            BreadcrumbItem::new("详情").link("/details"),
        ])
        // UIX 文档要求末项为当前页。
        .last_active();
    // 当前标题应来自末项。
    assert_eq!(breadcrumb.active_title(), Some("详情"));
    // 当前稳定身份应来自末项 link。
    assert_eq!(breadcrumb.active_link(), Some("/details"));
}

// 验证 reconcile 以 link 而非重复标题保持激活项。
#[test]
// 声明稳定身份 reconcile 测试。
fn reconcile_preserves_selection_by_link_with_duplicate_titles() {
    // 旧运行节点激活第二个同名条目。
    let mut breadcrumb = Breadcrumb::new().items(vec![
        // 第一个同名条目使用路径 A。
        BreadcrumbItem::new("详情").link("/a"),
        // 第二个同名条目使用路径 B 并激活。
        BreadcrumbItem::new("详情").link("/b").active(),
    ]);
    // 新声明调换两个同名条目的顺序。
    let next = Breadcrumb::new().items(vec![
        // 路径 B 移到首位。
        BreadcrumbItem::new("详情").link("/b"),
        // 路径 A 移到末位且由声明默认激活。
        BreadcrumbItem::new("详情").link("/a").active(),
    ]);
    // reconcile 应保留运行中的稳定路径选择。
    breadcrumb.sync_from(next);
    // 激活项应跟随路径 B 到新索引零。
    assert_eq!(breadcrumb.active_index(), 0);
    // 重复标题不得把选择串到路径 A。
    assert_eq!(breadcrumb.active_link(), Some("/b"));
}

// 验证选择事实优先使用 link，并兼容旧标题载荷。
#[test]
// 声明语义事件载荷测试。
fn change_payload_prefers_link_and_falls_back_to_title() {
    // 构造带稳定 link 的组件。
    let mut linked = Breadcrumb::new().items(vec![
        // 首项提供稳定路径。
        BreadcrumbItem::new("首页").link("/home"),
    ]);
    // 构造有效点击。
    let linked_event = click_first_item();
    // 通过真实输入路径建立选择事实。
    linked.on_event(&linked_event);
    // 非空 link 必须成为 Change 载荷。
    assert_eq!(selection_payload(&linked, &linked_event), "/home");

    // 构造没有 link 的旧兼容条目。
    let mut legacy = Breadcrumb::new().items(vec![
        // 旧条目只提供显示标题。
        BreadcrumbItem::new("旧首页"),
    ]);
    // 构造第二次有效点击。
    let legacy_event = click_first_item();
    // 通过同一真实输入路径建立选择事实。
    legacy.on_event(&legacy_event);
    // 空 link 应回退到旧标题载荷。
    assert_eq!(selection_payload(&legacy, &legacy_event), "旧首页");
}
