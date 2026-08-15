// 引入生成代码承诺调用的公开 prelude。
use crate::prelude::*;

// 验证 List 外观配置只依赖公开 UIX 运行时契约。
#[test]
fn list_configuration_compiles_against_public_uix_api() {
    // 提供调用方拥有且元素可转换为 String 的文本数组。
    let list_items = ["待处理", "进行中", "已完成"];
    // 提供动态边框状态以覆盖表达式映射。
    let show_border = false;
    // 展开动态边框与静态小尺寸的真实消费者文档。
    let _view: ViewNode = uix!(
        r#"<List data={list_items} bordered={show_border} size="small" automationId="task-list" />"#
    );
    // 生成代码必须复制集合，保留调用方所有权。
    assert_eq!(list_items[0], "待处理");
    // 动态布尔表达式仍由调用方持有。
    assert!(!show_border);
}
