// 引入解析、核心生成与诊断入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证零子节点 Badge 保持独立叶行为并映射三项登记属性。
#[test]
// 测试名称陈述数字、圆点、文字和公共身份生成契约。
fn generates_standalone_badge_contract() {
    // 生成覆盖动态计数、圆点、文字与自动化标识的独立 Badge。
    let snapshot = generate(
        r#"<Badge count={unread_count} dot={show_dot} text={badge_text} automationId="unread" />"#,
    )
    // 合法独立 Badge 必须成功生成。
    .expect("文档属性应映射到公开 Badge API");
    // 动态计数必须进入公开 count 构建器。
    assert!(
        snapshot.contains("count")
            && snapshot.contains("unread_count")
            && snapshot.contains("as i32"),
        "{snapshot}"
    );
    // 动态圆点必须进入同类型 dot_when 构建器。
    assert!(snapshot.contains("dot_when (show_dot)"), "{snapshot}");
    // 动态文字只在构造期间借用。
    assert!(snapshot.contains("text (& * (badge_text))"), "{snapshot}");
    // 自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("automation_id"), "{snapshot}");
    // 零子节点不得生成组合 child 调用。
    assert!(!snapshot.contains(". child"), "{snapshot}");
    // 生成器必须进入 Badge 自己的 UIX 根声明，不能直接构造运行时叶节点。
    assert!(!snapshot.contains("ViewNode :: leaf"), "{snapshot}");
}

// 验证唯一静态真实子 View 进入 Badge 组合生命周期。
#[test]
// 测试名称陈述 Icon、Button 与内部控制流组合覆盖。
fn generates_single_static_child_badge_contract() {
    // 生成 Icon 子节点数字徽章。
    let icon = generate(r#"<Badge count="5"><Icon name="bell" /></Badge>"#)
        // 唯一静态 Icon 必须成功生成。
        .expect("Badge 应接受唯一 Icon 子 View");
    // 运行时 owner 必须接收完整 Icon ViewNode。
    assert!(icon.contains("child") && icon.contains("Icon"), "{icon}");

    // 生成带业务按钮和内部条件内容的静态容器。
    let container = generate(
        r#"<Badge dot><Container><If {show_button}><Button>通知</Button></If></Container></Badge>"#,
    )
    // 控制流位于唯一静态容器内部时基数稳定。
    .expect("静态容器内部应保留普通控制流");
    // 外层必须生成唯一 child 调用。
    assert!(container.contains("child"), "{container}");
    // 组合 Badge 同样必须经 UIX 根声明，且不增加外层包装节点。
    assert!(!container.contains("ViewNode :: leaf"), "{container}");
    // 内部条件必须保留为 Rust 控制流。
    assert!(container.contains("if show_button"), "{container}");
}

// 验证多子节点、直接控制流、裸文本与插值不会被静默降级。
#[test]
// 测试名称覆盖全部直接子形状拒绝路径。
fn rejects_invalid_badge_child_shapes() {
    // 多个直接子节点会产生不确定装饰锚点。
    let multiple = generate(r#"<Badge><Icon name="bell" /><Avatar text="AL" /></Badge>"#)
        // 多子节点必须失败。
        .expect_err("Badge 多子节点必须被拒绝");
    // 诊断必须说明唯一基数并给出容器修复路径。
    assert!(
        multiple.message.contains("最多只能包含一个")
            && multiple.suggestion.contains("Container 或 Row"),
        "{multiple:?}"
    );

    // 直接 If 会让子基数依赖运行时条件。
    let dynamic = generate(r#"<Badge><If {show}><Icon name="bell" /></If></Badge>"#)
        // 直接控制流必须失败。
        .expect_err("Badge 直接 If 必须被拒绝");
    // 诊断必须点名 If 或 For 边界。
    assert!(dynamic.message.contains("不能是 If 或 For"), "{dynamic:?}");

    // 可见裸文本没有真实组件身份。
    let text = generate(r#"<Badge>消息</Badge>"#)
        // 裸文本必须失败。
        .expect_err("Badge 裸文本子节点必须被拒绝");
    // 诊断必须给出显式 Label 修复路径。
    assert!(text.suggestion.contains("Label"), "{text:?}");

    // 直接插值同样没有稳定组件身份。
    let interpolation = generate(r#"<Badge>{content}</Badge>"#)
        // 插值必须失败。
        .expect_err("Badge 插值子节点必须被拒绝");
    // 诊断必须点明插值形状。
    assert!(interpolation.message.contains("插值"), "{interpolation:?}");
}

// 验证计数类型和未登记属性由编译期门禁拒绝。
#[test]
// 测试名称覆盖非法整数与超前公开能力。
fn rejects_invalid_count_and_unregistered_attributes() {
    // 小数字面量不满足公开 i32 契约。
    let count = generate(r#"<Badge count="1.5" />"#)
        // 非整数必须失败。
        .expect_err("Badge 小数 count 必须被拒绝");
    // 诊断必须点明 i32 类型。
    assert!(count.message.contains("i32"), "{count:?}");

    // max 尚未在当前 UIX 文档登记。
    let unknown = generate(r#"<Badge max="99" />"#)
        // 未登记能力必须由公共属性门禁拒绝。
        .expect_err("Badge 未登记 max 必须被拒绝");
    // 诊断必须保留具体属性名。
    assert!(unknown.message.contains("max"), "{unknown:?}");
}
