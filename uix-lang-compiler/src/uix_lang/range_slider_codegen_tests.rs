// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 RangeSlider 范围、步长、双状态与公共属性生成。
#[test]
fn generates_bound_range_slider_contract() {
    // 生成覆盖完整静态契约的区间滑块。
    let snapshot = generate(
        r#"<RangeSlider value={{ start: lower, end: upper }} min="0" max="100" step="5" width="260px" />"#,
    )
    // 合法区间滑块必须成功生成。
    .expect("文档属性应映射到公开 RangeSlider API");
    // 范围构造器必须先建立。
    let range = snapshot
        // 查找公开 RangeSlider 构造器。
        .find("RangeSlider :: new")
        // 失败时输出完整令牌便于定位格式漂移。
        .unwrap_or_else(|| panic!("应生成 RangeSlider 范围：{snapshot}"));
    // 步长必须存在。
    let step = snapshot.find("step (5f64").expect("应生成步长");
    // 起点状态绑定必须存在。
    let start = snapshot
        // 查找公开起点构建器。
        .find("start (& (lower))")
        // 失败表示对象字段未正确消费。
        .expect("应生成起点状态绑定");
    // 终点状态绑定必须存在。
    let end = snapshot
        // 查找公开终点构建器。
        .find("end (& (upper))")
        // 失败表示对象字段未正确消费。
        .expect("应生成终点状态绑定");
    // 配置顺序必须确保两个绑定初值遵守完整约束。
    assert!(range < step && step < start && start < end);
    // 公共宽度必须继续映射。
    assert!(snapshot.contains("width (260.0)"));
}

// 验证动态范围与对象字段顺序保持确定映射。
#[test]
fn generates_dynamic_range_slider_values() {
    // 故意反转对象字段源码顺序以验证语义顺序稳定。
    let snapshot = generate(
        r#"<RangeSlider value={{ end: upper, start: lower }} min={minimum} max={maximum} step={step} />"#,
    )
    // 动态配置必须成功生成。
    .expect("动态区间滑块配置应生成");
    // 动态范围与步长必须进入公开契约。
    assert!(
        snapshot.contains("minimum")
            && snapshot.contains("maximum")
            && snapshot.contains("step (step)")
    );
    // 生成器始终先绑定 start 再绑定 end。
    assert!(
        snapshot.find("start (& (lower))").expect("应生成 start")
            < snapshot.find("end (& (upper))").expect("应生成 end")
    );
}

// 验证 RangeSlider 拒绝不完整或越界的结构绑定。
#[test]
fn rejects_invalid_range_slider_value_shapes() {
    // 普通状态表达式没有两个具名所有权句柄。
    let plain = generate(r#"<RangeSlider value={range} />"#)
        // 普通绑定必须失败。
        .expect_err("普通表达式绑定必须失败");
    // 诊断必须明确对象契约。
    assert!(plain.message.contains("对象"));
    // 缺少 end 字段必须失败。
    let missing = generate(r#"<RangeSlider value={{ start: lower }} />"#)
        // 不完整区间必须失败。
        .expect_err("缺失字段必须失败");
    // 诊断必须点名缺失字段。
    assert!(missing.message.contains("缺少 end"));
    // 额外字段不能被静默忽略。
    let extra = generate(r#"<RangeSlider value={{ start: lower, end: upper, middle: center }} />"#)
        // 未登记字段必须失败。
        .expect_err("额外字段必须失败");
    // 诊断必须点名非法字段。
    assert!(extra.message.contains("middle"));
}

// 验证 RangeSlider 拒绝无法兑现的静态范围与叶形状。
#[test]
fn rejects_invalid_range_slider_numeric_contracts() {
    // 反向静态范围必须在编译期失败。
    let range = generate(r#"<RangeSlider min="10" max="1" />"#)
        // 反向范围必须失败。
        .expect_err("反向范围必须失败");
    // 诊断必须说明 min/max 顺序。
    assert!(range.message.contains("不能大于"));
    // 零步长不能被运行时静默替换。
    let step = generate(r#"<RangeSlider step="0" />"#)
        // 零步长必须失败。
        .expect_err("零步长必须失败");
    // 诊断必须说明正数约束。
    assert!(step.message.contains("大于 0"));
    // RangeSlider 子节点不能被生成器丢弃。
    let child = generate(r#"<RangeSlider><Text>lost</Text></RangeSlider>"#)
        // 子树形状必须失败。
        .expect_err("RangeSlider 子节点必须失败");
    // 诊断必须说明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}
