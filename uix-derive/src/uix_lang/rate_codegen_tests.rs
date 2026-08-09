// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Rate 星数、半星、状态绑定与公共属性生成。
#[test]
fn generates_bound_rate_contract() {
    // 生成覆盖完整静态契约的评分组件。
    let snapshot = generate(r#"<Rate value={rating} count="7" allowHalf width="180px" />"#)
        // 合法评分必须成功生成。
        .expect("文档属性应映射到公开 Rate API");
    // 星数必须先应用。
    let count = snapshot.find("count (7usize").expect("应生成 usize 星数");
    // 半星模式必须存在。
    let half = snapshot.find("allow_half ()").expect("应生成半星配置");
    // 状态绑定必须存在。
    let value = snapshot.find("value (& (rating))").expect("应生成状态绑定");
    // 配置顺序必须确保绑定初值遵守星数与半星上限。
    assert!(count < half && half < value);
    // 公共宽度必须继续映射。
    assert!(snapshot.contains("width (180.0)"));
}

// 验证动态星数与半星布尔值保留公开 Rust 类型检查。
#[test]
fn generates_dynamic_rate_values() {
    // 生成动态星数与半星配置。
    let snapshot = generate(r#"<Rate value={rating} count={stars} allowHalf={half} />"#)
        // 动态配置必须成功生成。
        .expect("动态评分配置应生成");
    // 星数表达式必须进入公开构建器。
    assert!(snapshot.contains("count (stars)"));
    // 动态半星必须生成同类型条件分支。
    assert!(snapshot.contains("if half") && snapshot.contains("allow_half ()"));
    // 条件配置仍必须先于状态绑定。
    assert!(
        snapshot.find("if half").expect("应生成半星分支")
            < snapshot.find("value (& (rating))").expect("应生成状态绑定")
    );
}

// 验证静态 false 不启用半星模式。
#[test]
fn omits_static_false_half_mode() {
    // 显式 false 应保留默认整星语义。
    let snapshot = generate(r#"<Rate value={rating} allowHalf="false" />"#)
        // 合法 false 配置必须成功。
        .expect("false 半星配置应生成");
    // 静态 false 不应调用启用半星的构建器。
    assert!(!snapshot.contains("allow_half"));
}

// 验证 Rate 拒绝无法兑现的状态、整数与叶形状。
#[test]
fn rejects_invalid_rate_contracts() {
    // 数字字面量不具备 State<u32> 双向绑定所有权。
    let literal = generate(r#"<Rate value="3" />"#)
        // 字面量绑定必须失败。
        .expect_err("字面量绑定必须失败");
    // 诊断必须明确状态类型。
    assert!(literal.message.contains("State<u32>"));
    // 小数星数不能映射为 usize。
    let count = generate(r#"<Rate count="4.5" />"#)
        // 非整数星数必须失败。
        .expect_err("非整数星数必须失败");
    // 诊断必须说明整数类型。
    assert!(count.message.contains("usize"));
    // 非布尔半星值必须由共享布尔规则拒绝。
    let half = generate(r#"<Rate allowHalf="yes" />"#)
        // 非布尔值必须失败。
        .expect_err("非布尔半星值必须失败");
    // 诊断必须说明布尔要求。
    assert!(half.message.contains("布尔"));
    // Rate 子节点不能被生成器丢弃。
    let child = generate(r#"<Rate><Text>lost</Text></Rate>"#)
        // 子树形状必须失败。
        .expect_err("Rate 子节点必须失败");
    // 诊断必须说明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}
