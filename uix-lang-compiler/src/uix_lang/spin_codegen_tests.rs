// 引入与公开宏一致的完整文档测试生成入口。
use super::generate_test_document_view as generate;

// 验证 Spin 动态配置、提示文字与公共属性的完整生成契约。
#[test]
// 声明独立加载指示器生成测试。
fn generates_standalone_spin_contract() {
    // 生成覆盖动态布尔值、动态提示与公共属性的加载指示器。
    let snapshot = generate(
        r#"<Spin spinning={is_loading} text={loading_text} width="240px" automationId="page-loading" />"#,
    )
    // 合法加载指示器必须成功生成。
    .expect("文档属性应映射到公开 Spin API");
    // 组件必须从公开默认构造器开始。
    assert!(snapshot.contains("Spin :: new ()"));
    // 动态 spinning 必须保留 bool 类型检查。
    assert!(snapshot.contains("spinning (is_loading)"));
    // 动态提示文字必须以临时借用进入复制内容的构建器。
    assert!(snapshot.contains("tip (& * (loading_text))"));
    // 无子树时不得启用遮罩布局模式。
    assert!(!snapshot.contains("wrapper_mode ()"));
    // 公共宽度与自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("width (240.0)") && snapshot.contains("automation_id"));
    // 生成器必须进入 Spin 自己的 UIX 根声明，不能直接构造运行时节点。
    assert!(snapshot.contains("build_view_with_children"));
    assert!(!snapshot.contains("ViewNode :: new"));
}

// 验证 Spin 包裹模式保留普通、If 与 For 有序子树。
#[test]
// 声明加载遮罩子树生成测试。
fn preserves_wrapper_children_and_control_flow() {
    // 生成同时包含静态、条件与循环子项的加载遮罩。
    let snapshot = generate(r#"<Spin><Text>静态</Text><If {show_more}><Button>详情</Button></If><For {item} in {items}><Text>{item}</Text></For></Spin>"#).expect("Spin 应保留完整有序子树");
    // 存在可生成子树时必须启用公开遮罩模式。
    assert!(snapshot.contains("wrapper_mode ()"));
    // 拥有型子树必须经同一 UIX 根桥接移交，不能增加包装节点。
    assert!(snapshot.contains("build_view_with_children"));
    assert!(!snapshot.contains("ViewNode :: new"));
    // 静态文本必须保留在生成子树中。
    assert!(snapshot.contains("\"静态\""));
    // 条件分支必须保留为 Rust if。
    assert!(snapshot.contains("if show_more"));
    // 循环分支必须保留内部位置枚举与作者绑定。
    assert!(
        snapshot.contains("__uix_for_ordinal")
            && snapshot.contains("enumerate")
            && snapshot.contains("item")
    );
}

// 验证 Spin 缺省值和布尔简写映射。
#[test]
// 声明加载状态默认值测试。
fn preserves_defaults_and_boolean_shorthand() {
    // 生成不覆盖任何运行时默认值的最小指示器。
    let defaults = generate(r#"<Spin />"#).expect("最小 Spin 应保持可构造");
    // 缺省 spinning=true 应由运行时构造器独占定义。
    assert!(!defaults.contains("spinning ("));
    // 未提供提示文字时不应生成空 tip 覆盖。
    assert!(!defaults.contains("tip ("));

    // 布尔简写必须生成显式 true 配置。
    let shorthand = generate(r#"<Spin spinning />"#).expect("spinning 简写应映射为 true");
    // 简写必须通过公开构建器传入 true。
    assert!(shorthand.contains("spinning (true)"));
    // 显式 false 必须保留关闭加载动画的声明意图。
    let disabled =
        generate(r#"<Spin spinning="false" />"#).expect("spinning=false 应映射到公开构建器");
    // false 字面量不得被优化掉。
    assert!(disabled.contains("spinning (false)"));
}

// 验证 Spin 非法布尔值、未知属性与未知事件不会静默降级。
#[test]
// 声明加载指示器拒绝路径测试。
fn rejects_invalid_spin_contracts() {
    // spinning 只接受布尔字面量或表达式。
    let invalid_boolean =
        generate(r#"<Spin spinning="yes" />"#).expect_err("非法 spinning 必须被拒绝");
    // 诊断必须点明布尔值要求。
    assert!(invalid_boolean.message.contains("布尔"));
    // 文档未登记 size，不能静默暴露运行时构建器。
    let unknown = generate(r#"<Spin size="large" />"#).expect_err("未登记 size 必须被拒绝");
    // 未知属性诊断必须包含具体名称。
    assert!(unknown.message.contains("size"));
    // Spin 当前没有专有完成事件。
    let event =
        generate(r#"<Spin @complete="on_complete" />"#).expect_err("未登记 @complete 必须被拒绝");
    // 未知事件诊断必须包含具体名称。
    assert!(event.message.contains("@complete"));
}
