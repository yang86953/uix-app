// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Image 来源、尺寸、文字、运行时开关与命名状态 View 的完整生成契约。
#[test]
// 声明完整 Image 生成测试。
fn generates_image_contract() {
    // 生成覆盖动态字符串、尺寸、开关、命名插槽与自动化标识的图片。
    let snapshot = generate(
        // 使用静态直接容器声明加载与失败状态 View。
        r#"<Image src={image_src} alt={image_alt} fallback={image_fallback} width={image_width} height="88px" radius={image_radius} preview={can_preview} fit="false" lazy automationId="hero-image"><Container slot="placeholder"><Text>正在加载</Text></Container><Container slot="error"><Text>加载失败</Text></Container></Image>"#,
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
    // 加载插槽必须进入公开 placeholder 构建器。
    assert!(snapshot.contains("placeholder (:: uix_app :: prelude :: View :: build"));
    // 错误插槽必须进入每次重新生成 View 的公开工厂。
    assert!(snapshot.contains("on_error (move | _error |"));
    // 两个插槽的文案必须保留在各自生成子树中。
    assert!(snapshot.contains("正在加载") && snapshot.contains("加载失败"));
    // 编译期归位属性不能泄漏到子 View 公共属性映射。
    assert!(!snapshot.contains("slot"));
    // 自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("automation_id"));
    // 生成器必须进入 Image 自己的 UIX 根声明，不能直接构造运行时叶节点。
    assert!(!snapshot.contains("ViewNode :: leaf"), "{snapshot}");
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
    // 最小声明也必须经过 Image 的 UIX 根。
    assert!(!snapshot.contains("ViewNode :: leaf"), "{snapshot}");
}

// 验证 Image 必需来源、默认插槽拒绝与静态数值边界。
#[test]
// 声明 Image 基础拒绝测试。
fn rejects_missing_source_children_and_invalid_numbers() {
    // 缺失 src 时没有稳定的运行时资源身份。
    let missing = generate(r#"<Image />"#)
        // 缺失必需属性必须失败。
        .expect_err("缺少 src 必须被拒绝");
    // 诊断必须点名 src。
    assert!(missing.message.contains("src"));
    // Image 不声明默认插槽，普通直接 View 必须被拒绝。
    let child = generate(r#"<Image src="a.png"><Text>非法</Text></Image>"#)
        // 嵌套元素必须失败。
        .expect_err("未命名 Image 子节点必须被拒绝");
    // 诊断必须点明命名插槽要求。
    assert!(child.message.contains("命名 slot"));
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

// 验证 Image 命名插槽拒绝动态名称、未知名称、重复目标和不稳定直接节点。
#[test]
// 声明 Image 插槽形状拒绝测试。
fn rejects_invalid_image_slots() {
    // 动态 slot 名称不能在编译期确定运行时构建器。
    let dynamic = generate(
        // 使用表达式模拟运行时归位名称。
        r#"<Image src="a.png"><Container slot={slot_name} /></Image>"#,
    )
    // 动态名称必须失败。
    .expect_err("动态 Image slot 必须被拒绝");
    // 诊断必须要求字符串字面量。
    assert!(dynamic.message.contains("字符串字面量"));
    // 未登记名称不能被静默丢弃。
    let unknown = generate(r#"<Image src="a.png"><Container slot="content" /></Image>"#)
        // 未知目标必须失败。
        .expect_err("未知 Image slot 必须被拒绝");
    // 诊断必须点名未知目标。
    assert!(unknown.message.contains("content"));
    // 同一状态只能声明一个直接 View。
    let duplicate = generate(
        // 声明两个加载占位以触发唯一性诊断。
        r#"<Image src="a.png"><Text slot="placeholder">A</Text><Text slot="placeholder">B</Text></Image>"#,
    )
    // 重复目标必须失败。
    .expect_err("重复 Image slot 必须被拒绝");
    // 诊断必须说明重复声明。
    assert!(duplicate.message.contains("重复声明"));
    // 直接 If 会让状态 View 身份不稳定。
    let control = generate(
        // 控制流即使内部只有一个 View 也不能直接占据命名插槽。
        r#"<Image src="a.png"><If {show}><Text slot="placeholder">加载</Text></If></Image>"#,
    )
    // 直接控制流必须失败。
    .expect_err("直接 If Image slot 必须被拒绝");
    // 诊断必须说明静态直接 View 约束。
    assert!(control.message.contains("静态直接 View"));
    // 裸插值不能形成命名状态 View。
    let interpolation = generate(r#"<Image src="a.png">{status}</Image>"#)
        // 未声明默认插槽的插值必须失败。
        .expect_err("Image 裸插值必须被拒绝");
    // 诊断必须说明默认内容不存在。
    assert!(interpolation.message.contains("默认插槽"));
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
