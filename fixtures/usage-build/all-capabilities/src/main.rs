// 导入全部使用方 capability 的代表公开类型与方法。
use uix::prelude::{
    parse_rich_text, Alert, App, BarChart, BarData, Breadcrumb, BreadcrumbItem, Color, DataTable,
    DropPosition, Form, Gauge, GraphicsBackend, ImageService, Modal, Navigation, QRCode, RichText,
    SettingsService, SnapshotTreeNode, State, Table, TableColumn, Tree, TreeNode, TreeSelect,
};
// 导入反馈 capability 的全局消息与通知门面。
use uix::ui::{message, notify};

// 声明用于验证泛型表格入口的最小行模型。
#[derive(Clone)]
// 保存全能力 fixture 的稳定行数据。
struct FullCapabilityRow {
    // 保存稳定行键。
    id: u64,
    // 保存列投影使用的名称。
    name: &'static str,
}

// 提供全公开能力 release fixture 的稳定入口。
fn main() {
    // 同时引用五个受 feature 门控的公开 backend 变体。
    let backends = [
        GraphicsBackend::Direct3D11,
        GraphicsBackend::Direct3D12,
        GraphicsBackend::Metal,
        GraphicsBackend::OpenGlEs,
        GraphicsBackend::Vulkan,
    ];
    // 组合图形选择与 Agent builder，证明两个应用级公开入口可共同启用。
    let app = App::new()
        .graphics_backend(backends[0])
        .enable_agent_control();
    // 构造设置服务以调用结构化序列化公开方法。
    let settings = SettingsService::new();
    // 写入只依赖标准元组实现的结构化值，避免 fixture 自身引入 serde。
    let encoded_settings = settings.set_struct("profile", &(7_u8, "Ada"));
    // 读取相同结构化值，覆盖反序列化公开入口。
    let decoded_settings = settings.get_struct::<(u8, String)>("profile");
    // 调用图片字节解码路径并保留结果参与 release 代码生成。
    let image_result = ImageService::new().load_from_bytes(std::hint::black_box(&[]));
    // 构造二维码组件以覆盖编码能力公开面。
    let qrcode = QRCode::new("https://uix.dev/all-capabilities");
    // 构造使用正则规则的表单，覆盖 form-pattern 方法边界。
    let form = Form::new()
        .field("code", "编码")
        .initial("UIX-42")
        .validate_pattern(r"^UIX-[0-9]+$", "编码格式错误")
        .build();
    // 解析富文本内容以覆盖公开辅助函数。
    let rich_segments = parse_rich_text("**UIX** `all-capabilities`");
    // 构造富文本组件以覆盖组件公开面。
    let rich_text = RichText::new().content(rich_segments);
    // 构造基础柱状图以覆盖基础图表公开面。
    let bar_chart = BarChart::new().data(vec![BarData::new("A", 1.0, Color::BLUE)]);
    // 构造高级仪表盘以覆盖高级图表公开面。
    let gauge = Gauge::new().value(50.0).min(0.0).max(100.0);
    // 构造基础表格以覆盖列与行公开模型。
    let table = Table::new()
        .columns(vec![TableColumn::new("Name", 120.0)])
        .rows(vec![vec!["Ada".to_owned()]]);
    // 准备泛型表格使用的结构化行数据。
    let rows = vec![FullCapabilityRow { id: 1, name: "Ada" }];
    // 构造泛型表格并绑定结构化列投影。
    let data_table: DataTable<FullCapabilityRow> =
        match Table::data(rows, |row| row.id.to_string()) {
            // 绑定成功时投影名称列。
            Ok(table) => table
                .columns(vec![TableColumn::new("Name", 120.0)
                    .bind(|row: &FullCapabilityRow| row.name.to_owned())]),
            // 行键校验失败时结束 fixture。
            Err(_) => return,
        };
    // 构造基础面包屑以覆盖导航条目公开面。
    let breadcrumb = Breadcrumb::new()
        .item(BreadcrumbItem::new("Home"))
        .item(BreadcrumbItem::new("All").active());
    // 准备泛型导航容器的受控页面状态。
    let page = State::new(1_u8);
    // 构造泛型导航容器以覆盖 typed key 与状态绑定。
    let navigation: Navigation<u8> = Navigation::new("Fixture")
        .item("Home", 1)
        .item("All", 2)
        .active_page(&page);
    // 构造基础反馈组件。
    let alert = Alert::success("全能力已启用");
    // 构造弹层反馈组件。
    let modal = Modal::new("确认").closable(true).overlay(true);
    // 读取全局消息门面的可用状态。
    let message_available = message().is_available();
    // 读取全局通知门面的可用状态。
    let notification_available = notify().is_available();
    // 构造展示树与共享节点模型。
    let root = TreeNode::new("根节点", "root").add(TreeNode::new("子节点", "child"));
    // 从节点生成公开树快照模型。
    let snapshot = SnapshotTreeNode::from_tree_node(&root);
    // 构造展示树并显式消费拖放位置枚举。
    let tree = Tree::new(vec![root.clone()]).on_drop(|_, _, position| {
        // 匹配公开拖放位置以覆盖枚举公开面。
        let _inside = matches!(position, DropPosition::Inside);
    });
    // 构造树选择器并复用节点模型。
    let tree_select = TreeSelect::new().nodes(vec![root]);
    // 通过黑盒消费可能失败的能力结果，防止 release 优化提前删除调用。
    let _ = std::hint::black_box((encoded_settings, decoded_settings, image_result));
    // 消费应用、backend 与基础 capability 值。
    drop((app, backends, qrcode, form, rich_text, bar_chart, gauge));
    // 消费表格、导航、反馈与树组件值。
    drop((
        table,
        data_table,
        breadcrumb,
        navigation,
        alert,
        modal,
        message_available,
        notification_available,
        snapshot,
        tree,
        tree_select,
    ));
}
