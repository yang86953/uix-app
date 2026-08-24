// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
    // 结束测试生成辅助函数。
}

// 验证 QRCode 内容、尺寸、纠错等级与公共属性的完整生成契约。
#[test]
// 声明完整 QRCode 生成测试。
fn generates_qrcode_contract() {
    // 生成覆盖动态内容、像素尺寸、Q 级纠错和公共属性的二维码。
    let snapshot = generate(
        r#"<QRCode value={qr_value} size="192px" errorLevel="Q" automationId="download-code" />"#,
    )
    // 合法二维码必须成功生成。
    .expect("文档属性应映射到公开 QRCode API");
    // 二维码必须从公开构造器开始并临时借用内容。
    assert!(snapshot.contains("QRCode :: new (& (qr_value))"));
    // 像素尺寸必须进入运行时 size 构建器。
    assert!(snapshot.contains("size (192.0)"));
    // Q 级必须映射为运行时编码 2。
    assert!(snapshot.contains("error_level (2u8)"));
    // 生成器必须进入 QRCode 自己的 UIX 根声明，不能直接构造运行时叶节点。
    assert!(!snapshot.contains("ViewNode :: leaf"), "{snapshot}");
    // 自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("automation_id"));
    // 结束完整 QRCode 生成测试。
}

// 验证 QRCode 文档默认尺寸与纠错等级。
#[test]
// 声明 QRCode 默认值测试。
fn generates_qrcode_documented_defaults() {
    // 生成只包含必需字面量内容的二维码。
    let snapshot = generate(r#"<QRCode value="https://example.com" />"#)
        // 最小合法二维码必须成功生成。
        .expect("缺省配置应使用文档默认值");
    // 文档 128px 默认值必须覆盖运行时 160px 默认值。
    assert!(snapshot.contains("size (128.0)"));
    // 文档 M 级默认值必须显式映射为运行时编码 1。
    assert!(snapshot.contains("error_level (1u8)"));
    // 字面量内容必须进入公开构造器。
    assert!(snapshot.contains("https://example.com"));
    // 结束默认值测试。
}

// 验证 QRCode 必需内容、叶节点与尺寸诊断。
#[test]
// 声明 QRCode 基础错误测试。
fn rejects_invalid_qrcode_shape_and_size() {
    // 缺失 value 时没有可编码内容。
    let missing = generate(r#"<QRCode />"#)
        // 缺失必需属性必须失败。
        .expect_err("缺少 value 必须被拒绝");
    // 诊断必须点名 value。
    assert!(missing.message.contains("value"));
    // QRCode 自身绘制矩阵，不能接受 View 子树。
    let child = generate(r#"<QRCode value="x"><Text>非法</Text></QRCode>"#)
        // 嵌套元素必须失败。
        .expect_err("QRCode 子节点必须被拒绝");
    // 诊断必须点明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
    // 非数值尺寸不能进入 f32 运行时契约。
    let size = generate(r#"<QRCode value="x" size="wide" />"#)
        // 非数值 size 必须失败。
        .expect_err("非法 size 必须被拒绝");
    // 诊断必须说明 f32 数值映射要求。
    assert!(size.message.contains("f32"));
    // 结束基础错误测试。
}

// 验证 QRCode 纠错关键字与专有属性边界。
#[test]
// 声明 QRCode 属性错误测试。
fn rejects_invalid_qrcode_error_level_and_attributes() {
    // 文档外的纠错等级不能被静默归一化。
    let level = generate(r#"<QRCode value="x" errorLevel="X" />"#)
        // 未登记关键字必须失败。
        .expect_err("非法 errorLevel 必须被拒绝");
    // 诊断必须包含非法值与完整合法集合。
    assert!(level.message.contains("X") && level.suggestion.contains("L、M、Q 或 H"));
    // 关键字选择不能推迟到运行时表达式。
    let dynamic = generate(r#"<QRCode value="x" errorLevel={level} />"#)
        // 动态 errorLevel 必须失败。
        .expect_err("动态纠错等级必须被拒绝");
    // 诊断必须说明字面量要求。
    assert!(dynamic.message.contains("字符串字面量"));
    // 未登记属性不能被公共映射静默忽略。
    let unknown = generate(r#"<QRCode value="x" level="H" />"#)
        // 拼写错误的属性必须失败。
        .expect_err("未知 QRCode 属性必须被拒绝");
    // 诊断必须包含具体未知属性名。
    assert!(unknown.message.contains("level"));
    // 结束属性错误测试。
}
