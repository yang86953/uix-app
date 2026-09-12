// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Watermark 字符串借用、动态透明度与公共属性生成。
#[test]
// 声明完整 Watermark 生成测试。
fn generates_watermark_contract() {
    // 生成覆盖动态文字、动态透明度和公共属性的水印。
    let snapshot = generate(
        r#"<Watermark text={watermark_text} opacity={watermark_opacity} width="320px" automationId="document-watermark" />"#,
    )
    // 合法水印必须成功生成。
    .expect("文档属性应映射到公开 Watermark API");
    // 水印必须临时借用调用方字符串。
    assert!(snapshot.contains("Watermark :: new (& * (watermark_text))"));
    // 动态透明度必须进入运行时构建器。
    assert!(snapshot.contains("opacity (watermark_opacity)"));
    // 生成器必须进入 Watermark 自己的 UIX 根声明，不能直接构造运行时叶节点。
    assert!(!snapshot.contains("ViewNode :: leaf"), "{snapshot}");
    // 公共宽度与自动化标识仍由统一属性层消费。
    assert!(snapshot.contains("width") && snapshot.contains("automation_id"));
}

// 验证 Watermark 文档默认透明度与静态闭区间边界。
#[test]
// 声明 Watermark 默认值测试。
fn generates_watermark_documented_defaults_and_bounds() {
    // 生成只包含必需文字的最小水印。
    let default_snapshot = generate(r#"<Watermark text="CONFIDENTIAL" />"#)
        // 最小合法水印必须成功生成。
        .expect("缺省透明度应使用文档默认值");
    // 文档默认值必须显式映射为 0.15。
    assert!(default_snapshot.contains("opacity (0.15)"));
    // 静态闭区间端点都应被接受。
    let bounds = generate(r#"<Column><Watermark text="zero" opacity="0" /><Watermark text="one" opacity="1" /></Column>"#)
        // 合法端点必须成功生成。
        .expect("透明度闭区间端点应被接受");
    // 两个端点必须分别进入运行时构建器。
    assert!(bounds.contains("opacity (0.0)") && bounds.contains("opacity (1.0)"));
}

// 验证 Watermark 必需文字与叶节点边界。
#[test]
// 声明 Watermark 结构错误测试。
fn rejects_invalid_watermark_shape() {
    // 缺失 text 时没有可绘制内容。
    let missing = generate(r#"<Watermark />"#)
        // 缺失必需属性必须失败。
        .expect_err("缺少 text 必须被拒绝");
    // 诊断必须点名 text。
    assert!(missing.message.contains("text"));
    // Watermark 自身绘制平铺文字，不能接受 View 子树。
    let child = generate(r#"<Watermark text="x"><Text>非法</Text></Watermark>"#)
        // 嵌套元素必须失败。
        .expect_err("Watermark 子节点必须被拒绝");
    // 诊断必须点明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}

// 验证 Watermark 静态透明度诊断。
#[test]
// 声明 Watermark 透明度错误测试。
fn rejects_invalid_watermark_opacity() {
    // 负透明度不能依赖运行时静默截断。
    let negative = generate(r#"<Watermark text="x" opacity="-0.1" />"#)
        // 静态负值必须失败。
        .expect_err("负 opacity 必须被拒绝");
    // 诊断必须说明闭区间。
    assert!(negative.message.contains("0 到 1"));
    // 大于一的透明度不能依赖运行时静默截断。
    let overflow = generate(r#"<Watermark text="x" opacity="1.1" />"#)
        // 静态超界值必须失败。
        .expect_err("超界 opacity 必须被拒绝");
    // 诊断必须说明闭区间。
    assert!(overflow.message.contains("0 到 1"));
    // 非有限透明度不能进入运行时契约。
    let nonfinite = generate(r#"<Watermark text="x" opacity="NaN" />"#)
        // 静态非有限值必须失败。
        .expect_err("非有限 opacity 必须被拒绝");
    // 共享诊断必须说明有限值要求。
    assert!(nonfinite.message.contains("有限值"));
    // 比例透明度不能携带长度单位。
    let unit = generate(r#"<Watermark text="x" opacity="20px" />"#)
        // 带单位 opacity 必须失败。
        .expect_err("带单位 opacity 必须被拒绝");
    // 专用诊断必须说明无单位约束。
    assert!(unit.message.contains("无单位"));
}

// 验证 Watermark 未登记属性不能穿过公共映射。
#[test]
// 声明 Watermark 未知属性测试。
fn rejects_unknown_watermark_attribute() {
    // 拼写错误的透明度属性不能被静默忽略。
    let unknown = generate(r#"<Watermark text="x" alpha="0.2" />"#)
        // 未知属性必须失败。
        .expect_err("未知 Watermark 属性必须被拒绝");
    // 诊断必须包含具体未知属性名。
    assert!(unknown.message.contains("alpha"));
}
