// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Image 来源、尺寸、文字与运行时开关的完整生成契约。
#[test]
// 声明完整 Image 生成测试。
fn generates_image_contract() {
    // 生成覆盖动态字符串、尺寸、圆角、布尔开关与自动化标识的图片。
    let snapshot = generate(
        // 使用 image-codecs capability 下的公开 src 构建器。
        r#"<Image src={image_src} alt={image_alt} fallback={image_fallback} width={image_width} height="88px" radius={image_radius} preview={can_preview} fit="false" lazy automationId="hero-image" />"#,
    )
    // 合法图片必须成功生成。
    .expect("文档属性应映射到公开 Image API");
    // 图片必须从公开构造器取得动态宽度和静态高度。
    assert!(snapshot.contains("Image :: new (image_width , 88.0)"));
    // 来源必须只在 src 构建器调用期间借用。
    assert!(snapshot.contains("src (& * (image_src))"));
    // 替代文本必须进入公开 alt 构建器。
    assert!(snapshot.contains("alt (& * (image_alt))"));
    // 失败文本必须进入公开 fallback 构建器。
    assert!(snapshot.contains("fallback (& * (image_fallback))"));
    // 动态圆角必须进入运行时 radius 构建器。
    assert!(snapshot.contains("radius (image_radius)"));
    // 动态预览开关必须进入运行时 preview 构建器。
    assert!(snapshot.contains("preview (can_preview)"));
    // 静态 false 必须关闭等比 fit。
    assert!(snapshot.contains("fit (false)"));
    // 布尔简写必须启用延迟加载。
    assert!(snapshot.contains("lazy (true)"));
    // 自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("automation_id"));
}

// 验证 Image 最小声明使用确定的固有尺寸与运行时开关默认值。
#[test]
// 声明 Image 默认值测试。
fn generates_image_documented_defaults() {
    // 生成只包含必需来源的最小图片。
    let snapshot = generate(r#"<Image src="assets/hero.png" />"#)
        // 最小合法图片必须成功生成。
        .expect("缺省 Image 应使用文档固有尺寸与运行时开关默认值");
    // 缺省固有尺寸必须固定为公开 Rust 示例的 128 x 88。
    assert!(snapshot.contains("Image :: new (128.0 , 88.0)"));
    // 字面量来源必须进入公开 src 构建器。
    assert!(snapshot.contains("assets/hero.png"));
    // 省略替代文本时不得生成额外构建器调用。
    assert!(!snapshot.contains(". alt"));
    // 省略失败文本时不得生成额外构建器调用。
    assert!(!snapshot.contains(". fallback"));
    // 省略圆角时必须保留运行时 6px 默认值。
    assert!(!snapshot.contains(". radius"));
    // 省略预览开关时必须保留运行时 true 默认值。
    assert!(!snapshot.contains(". preview"));
    // 省略 fit 时必须保留运行时 true 默认值。
    assert!(!snapshot.contains(". fit"));
    // 省略 lazy 时必须保留运行时 false 默认值。
    assert!(!snapshot.contains(". lazy"));
}

// 验证 Image 必需来源、叶节点与静态数值边界。
#[test]
// 声明 Image 基础拒绝测试。
fn rejects_missing_source_children_and_invalid_numbers() {
    // 缺失 src 时没有稳定的运行时资源身份。
    let missing = generate(r#"<Image />"#)
        // 缺失必需属性必须失败。
        .expect_err("缺少 src 必须被拒绝");
    // 诊断必须点名 src。
    assert!(missing.message.contains("src"));
    // Image 自身绘制加载状态，不能接受任意 View 子树。
    let child = generate(r#"<Image src="a.png"><Text>非法</Text></Image>"#)
        // 嵌套元素必须失败。
        .expect_err("Image 子节点必须被拒绝");
    // 诊断必须点明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
    // 零宽度会让图片永远不可见，必须在宏展开期拒绝。
    let width = generate(r#"<Image src="a.png" width="0" />"#)
        // 非正静态宽度必须失败。
        .expect_err("零 Image width 必须被拒绝");
    // 诊断必须说明正值约束。
    assert!(width.message.contains("width 必须大于 0"));
    // 负高度会被运行时夹取为零，必须在宏展开期拒绝。
    let height = generate(r#"<Image src="a.png" height="-1px" />"#)
        // 非正静态高度必须失败。
        .expect_err("负 Image height 必须被拒绝");
    // 诊断必须说明正值约束。
    assert!(height.message.contains("height 必须大于 0"));
    // 负圆角会被运行时夹取，必须在宏展开期拒绝。
    let radius = generate(r#"<Image src="a.png" radius="-1" />"#)
        // 负静态圆角必须失败。
        .expect_err("负 Image radius 必须被拒绝");
    // 诊断必须说明非负约束。
    assert!(radius.message.contains("不能为负数"));
}

// 验证 Image 未登记属性和非法布尔值不能穿过公共映射。
#[test]
// 声明 Image 属性拒绝测试。
fn rejects_unknown_image_attribute_and_invalid_boolean() {
    // 拼写错误的来源属性不能被静默忽略。
    let unknown = generate(r#"<Image src="a.png" source="b.png" />"#)
        // 未登记属性必须失败。
        .expect_err("未知 Image 属性必须被拒绝");
    // 诊断必须包含具体属性名。
    assert!(unknown.message.contains("source"));
    // 文档外的布尔字面量不能被静默解释。
    let preview = generate(r#"<Image src="a.png" preview="yes" />"#)
        // 非法布尔字面量必须失败。
        .expect_err("非法 Image preview 必须被拒绝");
    // 诊断必须说明布尔值要求。
    assert!(preview.message.contains("布尔值"));
}
