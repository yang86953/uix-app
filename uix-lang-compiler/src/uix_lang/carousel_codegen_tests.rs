// 引入与公开宏一致的完整文档测试生成入口。
use super::generate_test_document_view as generate;

// 验证 Carousel 动态自动播放、有序子树与公共属性生成。
#[test]
// 声明完整 Carousel 生成测试。
fn generates_dynamic_carousel_contract() {
    // 生成覆盖静态、If、For、动态 autoplay 与公共属性的轮播器。
    let snapshot = generate(
        r#"<Carousel autoplay={auto_play} width="320px" automationId="hero-carousel"><Text>首屏</Text><If {show_extra}><Text>条件页</Text></If><For {slide} in {slides}><Text>{slide}</Text></For></Carousel>"#,
    )
    // 合法 Carousel 必须成功生成。
    .expect("Carousel 应保留动态自动播放与完整有序子树");
    // 动态布尔值必须只决定是否调用三秒计时器构建器。
    assert!(
        snapshot.contains("if auto_play")
            && snapshot.contains("autoplay (:: std :: time :: Duration :: from_secs (3_u64))")
    );
    // 子树必须经组件自己的 UIX 根声明与有序子节点向量进入运行时。
    assert!(snapshot.contains("build_view_with_children") && snapshot.contains("__uix_children"));
    assert!(!snapshot.contains("ViewNode :: new"));
    // If 与带位置身份的 For 控制流必须保持在生成代码中。
    assert!(
        snapshot.contains("if show_extra")
            && snapshot.contains("__uix_for_ordinal")
            && snapshot.contains("enumerate")
    );
    // 公共宽度与自动化标识继续由公共属性层消费。
    assert!(snapshot.contains("width (320.0)") && snapshot.contains("automation_id"));
}

// 验证 Carousel 缺省与静态布尔声明。
#[test]
// 声明 Carousel 默认值测试。
fn generates_carousel_autoplay_defaults() {
    // 缺省 Carousel 保持无计时器但仍接受子 View。
    let default = generate(r#"<Carousel><Text>单页</Text></Carousel>"#)
        // 最小轮播器必须成功生成。
        .expect("Carousel 缺省配置应生成");
    // 未声明 autoplay 时不得注册自动播放。
    assert!(!default.contains("Duration :: from_secs"));
    // 布尔简写必须启用三秒自动播放。
    let enabled = generate(r#"<Carousel autoplay><Text>自动页</Text></Carousel>"#)
        // 简写布尔属性必须成功生成。
        .expect("Carousel autoplay 简写应生成");
    // 简写值必须进入确定的 true 分支表达式。
    assert!(enabled.contains("if true") && enabled.contains("Duration :: from_secs"));
    // 显式 false 必须保留关闭分支而不伪造其他配置。
    let disabled = generate(r#"<Carousel autoplay="false" />"#)
        // false 字面量必须成功生成。
        .expect("Carousel autoplay=false 应生成");
    // 关闭配置必须生成确定的 false 条件。
    assert!(disabled.contains("if false"));
}

// 验证 Carousel 属性诊断边界。
#[test]
// 声明 Carousel 错误属性测试。
fn rejects_invalid_carousel_attributes() {
    // 非布尔 autoplay 不能决定计时器配置。
    let invalid = generate(r#"<Carousel autoplay="sometimes" />"#)
        // 非布尔字面量必须失败。
        .expect_err("Carousel 非布尔 autoplay 必须被拒绝");
    // 诊断必须点明布尔值要求。
    assert!(invalid.message.contains("布尔"));
    // 未登记 interval 不能静默覆盖三秒契约。
    let unknown = generate(r#"<Carousel interval="5" />"#)
        // 文档外属性必须失败。
        .expect_err("Carousel 未登记 interval 必须被拒绝");
    // 诊断必须包含具体未知属性名。
    assert!(unknown.message.contains("interval"));
}
