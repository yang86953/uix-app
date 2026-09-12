// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Steps 数据、受控 current、方向与公共属性生成。
#[test]
// 声明完整 Steps 生成测试。
fn generates_bound_steps_contract() {
    // 生成覆盖类型化数据、状态、方向、动态点击、圆点与公共属性的步骤条。
    let snapshot = generate(
        r#"<Steps current={step} items={step_items} direction="vertical" clickable={allow_step_change} dot width="320px" automationId="checkout-steps" />"#,
    )
    // 合法 Steps 必须成功生成。
    .expect("文档属性应映射到公开 Steps API");
    // 数据表达式必须通过拥有所有权的 IntoIterator 统一收集。
    assert!(snapshot.contains("IntoIterator :: into_iter ((step_items) . clone ())"));
    // 收集目标必须锁定公开 Step 类型。
    assert!(snapshot.contains("Vec < :: uix_app :: prelude :: Step >"));
    // current 必须借用 State<usize> 句柄。
    assert!(snapshot.contains("current_state (& (step))"));
    // 垂直方向必须调用公开构建器。
    assert!(snapshot.contains("vertical ()"));
    // 动态点击配置必须保留调用方 bool 类型检查。
    assert!(snapshot.contains("clickable (allow_step_change)"));
    // 圆点布尔简写必须生成显式 true。
    assert!(snapshot.contains("dot (true)"));
    // 公共宽度与自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("width (320.0)") && snapshot.contains("automation_id"));
}

// 验证 Steps 缺省方向固定为 horizontal。
#[test]
// 声明 Steps 默认方向测试。
fn generates_horizontal_steps_by_default() {
    // 生成只含必需属性的最小步骤条。
    let snapshot = generate(r#"<Steps current={step} items={step_items} />"#)
        // 最小属性集合必须成功生成。
        .expect("缺省方向应使用文档默认值");
    // 缺省值必须显式调用水平构建器。
    assert!(snapshot.contains("horizontal ()"));
}

// 验证 Steps 必需数据与状态绑定诊断。
#[test]
// 声明 Steps 数据和状态错误测试。
fn rejects_invalid_steps_data_and_state() {
    // 缺失 items 时没有运行时步骤来源。
    let missing_items = generate(r#"<Steps current={step} />"#)
        // 缺失必需属性必须失败。
        .expect_err("缺少 items 必须被拒绝");
    // 诊断必须点名 items。
    assert!(missing_items.message.contains("items"));
    // 字符串不能伪装成类型化 Step 集合。
    let literal_items = generate(r#"<Steps current={step} items="steps" />"#)
        // 非表达式 items 必须失败。
        .expect_err("字符串 items 必须被拒绝");
    // 诊断必须说明 Step 集合要求。
    assert!(literal_items.message.contains("Step"));
    // 缺失 current 时无法兑现双向绑定。
    let missing_current = generate(r#"<Steps items={step_items} />"#)
        // 缺失状态必须失败。
        .expect_err("缺少 current 必须被拒绝");
    // 诊断必须点名 current。
    assert!(missing_current.message.contains("current"));
    // 数字字面量不能提供 State<usize> 所有权。
    let literal_current = generate(r#"<Steps current="1" items={step_items} />"#)
        // 字面量 current 必须失败。
        .expect_err("字面量 current 必须被拒绝");
    // 诊断必须明确 State<usize> 类型。
    assert!(literal_current.message.contains("State<usize>"));
}

// 验证 Steps 叶节点、方向与专有属性边界。
#[test]
// 声明 Steps 形状错误测试。
fn rejects_invalid_steps_shape_and_attributes() {
    // Steps 自身绘制步骤数据，不能接受 View 子树。
    let child = generate(r#"<Steps current={step} items={step_items}><Text>非法</Text></Steps>"#)
        // 嵌套元素必须失败。
        .expect_err("Steps 子节点必须被拒绝");
    // 诊断必须点明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
    // 未知方向不能回退到默认水平布局。
    let direction = generate(r#"<Steps current={step} items={step_items} direction="diagonal" />"#)
        // 非法方向必须失败。
        .expect_err("非法 direction 必须被拒绝");
    // 诊断必须列出允许的两个关键字。
    assert!(direction.message.contains("horizontal") && direction.message.contains("vertical"));
    // 动态方向没有登记为运行时契约。
    let dynamic = generate(r#"<Steps current={step} items={step_items} direction={layout} />"#)
        // 动态方向必须失败。
        .expect_err("动态 direction 必须被拒绝");
    // 动态方向使用同一有限关键字诊断。
    assert!(dynamic.message.contains("horizontal"));
    // 未登记属性不能被公共映射静默忽略。
    let unknown = generate(r#"<Steps current={step} items={step_items} status="process" />"#)
        // 文档外属性必须失败。
        .expect_err("未知 Steps 属性必须被拒绝");
    // 诊断必须包含具体未知属性名。
    assert!(unknown.message.contains("status"));
}
