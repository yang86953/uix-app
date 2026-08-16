// 引入宏生成代码承诺使用的公开 prelude。
use crate::prelude::*;

// 验证 Breadcrumb 分隔符与折叠阈值只依赖公开 UIX 运行时契约。
#[test]
fn breadcrumb_configuration_compiles_against_public_uix_api() {
    // 创建公开 BreadcrumbItem 元数据集合。
    let breadcrumb_items = vec![
        // 首项提供稳定首页链接。
        BreadcrumbItem::new("首页").link("/"),
        // 次项提供稳定设置链接。
        BreadcrumbItem::new("设置").link("/settings"),
        // 末项提供稳定详情链接。
        BreadcrumbItem::new("详情").link("/settings/detail"),
    ];
    // 声明由 Rust 类型系统核对的动态分隔文本。
    let separator_text = String::from("›");
    // 声明由 Rust 类型系统核对的动态折叠阈值。
    let maximum_items = 3_usize;
    // 展开带动态分隔符与折叠阈值的 Breadcrumb。
    let _breadcrumb: ViewNode = crate::uix!(
        // 使用完整已登记 Breadcrumb UIX 形状。
        r#"<Breadcrumb items={breadcrumb_items} separator={separator_text} maxItems={maximum_items} />"#
    );
}
