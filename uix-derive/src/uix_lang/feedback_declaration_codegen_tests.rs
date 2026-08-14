// 引入解析和 View 生成入口。
use super::{generate_view, parse_document};

// 把 UIX 源码生成成可断言令牌文本。
fn generate(source: &str) -> Result<String, super::Diagnostic> {
    // 先解析完整文档。
    let document = parse_document(source).expect("反馈声明测试源码应满足语法");
    // 再生成公开 Rust View 并标准化令牌文本。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证两类声明的属性、默认值、稳定 key 与关闭事实映射。
#[test]
fn generates_keyed_message_and_notification_declarations() {
    // 生成覆盖状态、动态时长、关闭能力和类型化事件的两个声明节点。
    let tokens = generate(
        r#"<Container><Message key="saved" type="success" content="保存成功" duration={message_seconds} closable @close="record_message($event)" /><Notification key="sync" title="同步完成" content="全部数据已更新" /></Container>"#,
    )
    // 合法声明必须进入公开生成矩阵。
    .expect("Message 与 Notification 应生成声明租约组件");
    // Message 必须保留稳定 key、内容和状态。
    assert!(tokens.contains("MessageDeclaration :: new (\"saved\" , \"保存成功\")"));
    // 动态秒数必须进入运行时有限性校验入口。
    assert!(tokens.contains("duration_seconds ((message_seconds) as f64)"));
    // 关闭能力必须显式保存最终布尔值。
    assert!(tokens.contains("closable (true)"));
    // @close 必须注册类型化关闭事实回调。
    assert!(tokens.contains("on_close (move | __uix_message_closed |"));
    // View 协调 key 必须与声明租约 key 一致。
    assert!(tokens.contains("key (:: std :: format ! (\"{}\" , \"saved\"))"));
    // Notification 必须映射标题、正文与默认配置。
    assert!(
        tokens.contains(
            "NotificationDeclaration :: new (\"sync\" , \"同步完成\" , \"全部数据已更新\")"
        )
    );
}

// 验证必需属性、稳定身份、状态和叶节点诊断。
#[test]
fn rejects_invalid_feedback_declaration_contracts() {
    // 缺失 Message key 必须失败。
    let missing_key = generate(r#"<Message content="保存成功" />"#)
        // 必需身份不能由运行时猜测。
        .expect_err("Message 缺少 key 必须失败");
    // 诊断必须点名 key。
    assert!(missing_key.message.contains("缺少必需的 key"));
    // 空 Notification key 必须失败。
    let empty_key = generate(r#"<Notification key=" " title="标题" content="正文" />"#)
        // 空白身份不能建立租约。
        .expect_err("Notification 空 key 必须失败");
    // 诊断必须说明非空约束。
    assert!(empty_key.message.contains("key 必须是非空字符串"));
    // 非法状态关键字必须失败。
    let status = generate(r#"<Message key="saved" content="完成" type="primary" />"#)
        // 不得静默回退为 info。
        .expect_err("非法 Message type 必须失败");
    // 诊断必须列出有限状态集合。
    assert!(status.message.contains("只支持 info"));
    // 静态负时长必须失败。
    let duration = generate(r#"<Message key="saved" content="完成" duration="-1" />"#)
        // 负时长没有合法生命周期语义。
        .expect_err("负 duration 必须失败");
    // 诊断必须说明有限非负秒数。
    assert!(duration.message.contains("有限非负秒数"));
    // 声明节点不能承载子树。
    let child = generate(
        r#"<Notification key="n" title="标题" content="正文"><Text>额外</Text></Notification>"#,
    )
    // 实际内容由 Host 绘制。
    .expect_err("Notification 子节点必须失败");
    // 诊断必须说明叶节点形状。
    assert!(child.message.contains("不接受子节点"));
}
