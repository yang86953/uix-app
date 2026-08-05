// 导入基础面包屑、泛型导航容器与受控状态，覆盖单 capability 公开入口。
use uix::prelude::{Breadcrumb, BreadcrumbItem, Navigation, State};

// 提供可执行 fixture 的稳定入口。
fn main() {
    // 构造基础面包屑，证明条目模型和组件构造器公开面可达。
    let breadcrumb = Breadcrumb::new()
        // 添加普通路径条目。
        .item(BreadcrumbItem::new("Home"))
        // 添加当前激活条目。
        .item(BreadcrumbItem::new("Settings").active());
    // 准备泛型导航容器的受控页面状态。
    let page = State::new(1_u8);
    // 构造泛型导航容器，证明 typed key 与双向状态绑定公开面可达。
    let navigation: Navigation<u8> = Navigation::new("Fixture")
        // 添加第一个 typed key 导航项。
        .item("Home", 1)
        // 添加带图标的第二个 typed key 导航项。
        .item_with_icon("Settings", 2, "settings")
        // 将选中项绑定到外部页面状态。
        .active_page(&page);
    // 显式消费全部值，避免无意义的未使用警告。
    drop((breadcrumb, navigation));
}
