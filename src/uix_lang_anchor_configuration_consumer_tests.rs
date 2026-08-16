// 引入宏生成代码承诺使用的公开 prelude。
use crate::prelude::*;

// 验证 Anchor 指示线策略只依赖公开 UIX 运行时契约。
#[test]
fn anchor_configuration_compiles_against_public_uix_api() {
    // 创建公开 AnchorItem 元数据集合。
    let anchor_items = vec![
        // 首项指向基础区域。
        AnchorItem::new("基础", "#basic"),
        // 次项指向高级区域。
        AnchorItem::new("高级", "#advanced"),
    ];
    // 声明由 Rust 类型系统核对的动态指示线策略。
    let show_anchor_ink = false;
    // 展开带定位偏移与动态指示线配置的 Anchor。
    let _anchor: ViewNode = crate::uix!(
        // 使用完整已登记 Anchor UIX 形状。
        r#"<Anchor items={anchor_items} offsetTop="24px" showInk={show_anchor_ink} />"#
    );
}
