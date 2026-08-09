// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 InputNumber 全部文档属性与泛型状态绑定生成。
#[test]
fn generates_bound_input_number_contract() {
    // 生成覆盖范围、步长、精度、绑定和公共尺寸的 InputNumber。
    let snapshot = generate(
        r#"<InputNumber value={amount} min="0" max="100" step="0.25" precision="2" width="160px" />"#,
    )
    // 合法数值输入必须成功生成。
    .expect("文档属性应映射到公开 InputNumber API");
    // 范围必须在值绑定前建立。
    let minimum = snapshot
        // 查找公开最小值构建器。
        .find("min (0f64")
        // 失败时输出完整令牌便于定位格式漂移。
        .unwrap_or_else(|| panic!("应生成最小值：{snapshot}"));
    // 最大值必须存在。
    let maximum = snapshot.find("max (100f64").expect("应生成最大值");
    // 步长必须存在。
    let step = snapshot.find("step (0.25f64").expect("应生成步长");
    // 精度必须存在。
    let precision = snapshot.find("precision (2u8").expect("应生成 u8 精度");
    // 状态绑定必须存在。
    let value = snapshot.find("value (& (amount))").expect("应生成状态绑定");
    // 配置顺序必须确保绑定初值遵守完整约束。
    assert!(minimum < maximum && maximum < step && step < precision && precision < value);
    // 公共宽度必须继续映射。
    assert!(snapshot.contains("width (160.0)"));
}

// 验证动态数值与精度表达式保持公开 Rust 类型检查。
#[test]
fn generates_dynamic_input_number_values() {
    // 生成全部动态配置表达式。
    let snapshot = generate(
        r#"<InputNumber value={amount} min={minimum} max={maximum} step={step} precision={precision} />"#,
    )
    // 动态配置必须成功生成。
    .expect("动态数值配置应生成");
    // 每个表达式必须进入对应公开构建器。
    assert!(
        snapshot.contains("min (minimum)")
            && snapshot.contains("max (maximum)")
            && snapshot.contains("step (step)")
            && snapshot.contains("precision (precision)")
    );
}

// 验证 InputNumber 拒绝无法兑现的静态契约。
#[test]
fn rejects_invalid_input_number_contracts() {
    // 字符串 value 不具备 State<T> 双向绑定所有权。
    let literal = generate(r#"<InputNumber value="12" />"#)
        // 字面量绑定必须失败。
        .expect_err("字面量绑定必须失败");
    // 诊断必须明确泛型状态类型。
    assert!(literal.message.contains("State<T"));
    // 反向静态范围必须在编译期失败。
    let range = generate(r#"<InputNumber min="10" max="1" />"#)
        // 反向范围必须失败。
        .expect_err("反向范围必须失败");
    // 诊断必须说明 min/max 顺序。
    assert!(range.message.contains("不能大于"));
    // 零步长不能被运行时静默替换。
    let step = generate(r#"<InputNumber step="0" />"#)
        // 零步长必须失败。
        .expect_err("零步长必须失败");
    // 诊断必须说明正数约束。
    assert!(step.message.contains("大于 0"));
    // 超过 f64 上限的静态精度必须失败。
    let precision = generate(r#"<InputNumber precision="16" />"#)
        // 越界精度必须失败。
        .expect_err("越界精度必须失败");
    // 诊断必须说明十五位上限。
    assert!(precision.message.contains("超过 15"));
    // InputNumber 子节点不能被生成器丢弃。
    let child = generate(r#"<InputNumber><Text>lost</Text></InputNumber>"#)
        // 子树形状必须失败。
        .expect_err("InputNumber 子节点必须失败");
    // 诊断必须说明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}
