// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Avatar 字符串借用、形状、尺寸与公共属性生成。
#[test]
fn generates_avatar_contract() {
    // 生成覆盖图片、回退文字、方形裁剪、动态尺寸与自动化标识的头像。
    let snapshot = generate(
        r#"<Avatar text={avatar_text} src={avatar_src} shape="square" size={avatar_size} automationId="account-avatar" />"#,
    )
    // 合法头像必须成功生成。
    .expect("文档属性应映射到公开 Avatar API");
    // 回退文字必须只在构造期间借用。
    assert!(snapshot.contains("Avatar :: new (& * (avatar_text))"));
    // 图片来源必须只在 src 构建器调用期间借用。
    assert!(snapshot.contains("src (& * (avatar_src))"));
    // square 关键字必须映射为 true。
    assert!(snapshot.contains("square (true)"));
    // 动态边长必须进入运行时 size 构建器。
    assert!(snapshot.contains("size (avatar_size)"));
    // 自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("automation_id"));
    // 生成器必须进入 Avatar 自己的 UIX 根声明，不能直接构造运行时叶节点。
    assert!(!snapshot.contains("ViewNode :: leaf"), "{snapshot}");
}

// 验证 Avatar 文档默认形状与运行时默认尺寸边界。
#[test]
fn generates_avatar_defaults_without_image_capability_call() {
    // 生成没有专有属性的最小头像。
    let snapshot = generate(r#"<Avatar />"#)
        // 空回退头像仍是合法运行时组件。
        .expect("缺省 Avatar 应使用运行时默认值");
    // 默认文字必须为空字符串。
    assert!(snapshot.contains("Avatar :: new (& * (\"\"))"));
    // 默认形状必须显式映射为 circle。
    assert!(snapshot.contains("square (false)"));
    // 省略 src 时不得引入 image-codecs 专属方法。
    assert!(!snapshot.contains(". src"));
    // 省略 size 时必须保留运行时 32px 默认值。
    assert!(!snapshot.contains(". size"));
    // 生成字面量文字与像素边长，覆盖文档的静态写法。
    let literal = generate(r#"<Avatar text="AL" size="40px" />"#)
        // 正有限像素尺寸必须成功生成。
        .expect("Avatar 应接受字符串与 px 字面量");
    // 字面量文字必须进入公开构造器。
    assert!(literal.contains("AL"));
    // 像素边长必须映射为 f32 构建器参数。
    assert!(literal.contains("size (40.0)"));
}

// 验证 Avatar 拒绝非法形状、静态尺寸与子节点。
#[test]
fn rejects_invalid_avatar_shape_size_and_children() {
    // 文档外的形状不能被静默归一化。
    let shape = generate(r#"<Avatar shape="rounded" />"#)
        // 未登记关键字必须失败。
        .expect_err("非法 Avatar shape 必须被拒绝");
    // 诊断必须给出完整合法集合。
    assert!(shape.suggestion.contains("circle 或 square"));
    // 动态形状无法确定映射到哪个公开布尔值。
    let dynamic_shape = generate(r#"<Avatar shape={avatar_shape} />"#)
        // 动态 shape 必须失败。
        .expect_err("动态 Avatar shape 必须被拒绝");
    // 诊断必须说明字符串字面量要求。
    assert!(dynamic_shape.message.contains("字符串字面量"));
    // 零边长会被运行时回退，必须在宏展开期拒绝。
    let size = generate(r#"<Avatar size="0" />"#)
        // 非正静态尺寸必须失败。
        .expect_err("非正 Avatar size 必须被拒绝");
    // 诊断必须说明正值约束。
    assert!(size.message.contains("大于 0"));
    // 非有限边长不能进入运行时尺寸契约。
    let nonfinite = generate(r#"<Avatar size="NaN" />"#)
        // 非有限静态尺寸必须失败。
        .expect_err("非有限 Avatar size 必须被拒绝");
    // 诊断必须沿用共享有限数值约束。
    assert!(nonfinite.message.contains("不是有限值"));
    // Avatar 不接受可渲染子树。
    let child = generate(r#"<Avatar><Text>非法</Text></Avatar>"#)
        // 子节点必须失败。
        .expect_err("Avatar 子节点必须被拒绝");
    // 诊断必须点明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}

// 验证 Avatar 未登记属性不能穿过公共映射。
#[test]
fn rejects_unknown_avatar_attribute() {
    // 拼写错误的来源属性不能被静默忽略。
    let unknown = generate(r#"<Avatar source="avatar.png" />"#)
        // 未登记属性必须失败。
        .expect_err("未知 Avatar 属性必须被拒绝");
    // 诊断必须包含具体属性名。
    assert!(unknown.message.contains("source"));
}
