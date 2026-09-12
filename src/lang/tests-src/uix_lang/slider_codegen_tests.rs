// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Slider 范围、步长、状态绑定与公共属性生成。
#[test]
fn generates_bound_slider_contract() {
    // 生成覆盖完整静态契约的单值滑块。
    let snapshot =
        generate(r#"<Slider value={volume} min="0" max="100" step="5" width="240px" />"#)
            // 合法单值滑块必须成功生成。
            .expect("文档属性应映射到公开 Slider API");
    // 范围构造器必须先建立。
    let range = snapshot
        // 查找公开 Slider 构造器。
        .find("Slider :: new")
        // 失败时输出完整令牌便于定位格式漂移。
        .unwrap_or_else(|| panic!("应生成 Slider 范围：{snapshot}"));
    // 最小值必须进入构造范围。
    assert!(snapshot.contains("0f64"));
    // 最大值必须进入构造范围。
    assert!(snapshot.contains("100f64"));
    // 步长必须存在。
    let step = snapshot.find("step (5f64").expect("应生成步长");
    // 状态绑定必须存在。
    let value = snapshot.find("value (& (volume))").expect("应生成状态绑定");
    // 配置顺序必须确保绑定初值遵守完整约束。
    assert!(range < step && step < value);
    // 公共宽度必须继续映射。
    assert!(snapshot.contains("width (240.0)"));
}

// 验证动态范围与步长保留公开 Rust 类型检查。
#[test]
fn generates_dynamic_slider_values() {
    // 生成全部动态数值配置表达式。
    let snapshot = generate(r#"<Slider value={volume} min={minimum} max={maximum} step={step} />"#)
        // 动态配置必须成功生成。
        .expect("动态滑块配置应生成");
    // 每个表达式必须进入对应公开契约。
    assert!(
        snapshot.contains("minimum")
            && snapshot.contains("maximum")
            && snapshot.contains("step (step)")
    );
}

// 验证 Slider 拒绝无法兑现的静态契约。
#[test]
fn rejects_invalid_slider_contracts() {
    // 数字字面量不具备 State<f64> 双向绑定所有权。
    let literal = generate(r#"<Slider value="50" />"#)
        // 字面量绑定必须失败。
        .expect_err("字面量绑定必须失败");
    // 诊断必须明确状态类型。
    assert!(literal.message.contains("State<f64>"));
    // 反向静态范围必须在编译期失败。
    let range = generate(r#"<Slider min="10" max="1" />"#)
        // 反向范围必须失败。
        .expect_err("反向范围必须失败");
    // 诊断必须说明 min/max 顺序。
    assert!(range.message.contains("不能大于"));
    // 零步长不能被运行时静默替换。
    let step = generate(r#"<Slider step="0" />"#)
        // 零步长必须失败。
        .expect_err("零步长必须失败");
    // 诊断必须说明正数约束。
    assert!(step.message.contains("大于 0"));
    // Slider 子节点不能被生成器丢弃。
    let child = generate(r#"<Slider><Text>lost</Text></Slider>"#)
        // 子树形状必须失败。
        .expect_err("Slider 子节点必须失败");
    // 诊断必须说明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}
