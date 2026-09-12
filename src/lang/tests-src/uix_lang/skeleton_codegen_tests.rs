// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Skeleton 形状、尺寸与公共属性生成。
#[test]
fn generates_skeleton_contract() {
    // 生成覆盖完整静态契约的文字骨架屏。
    let snapshot =
        generate(r#"<Skeleton shape="text" width="240px" height="48" automationId="loading" />"#)
            // 合法骨架屏必须成功生成。
            .expect("文档属性应映射到公开 Skeleton API");
    // 形状必须映射到公开枚举。
    assert!(snapshot.contains("SkeletonShape :: Text"));
    // 宽度必须应用到 Skeleton 自身。
    assert!(snapshot.contains("width (240.0)"));
    // 高度必须应用到 Skeleton 自身。
    assert!(snapshot.contains("height (48.0)"));
    // 自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("automation_id"));
    // 生成器必须进入 Skeleton 自己的 View/UIX 声明边界，不能直接绕过为叶节点。
    assert!(!snapshot.contains("ViewNode :: leaf"));
}

// 验证动态尺寸保留公开 Rust f32 类型检查。
#[test]
fn generates_dynamic_skeleton_dimensions() {
    // 生成动态宽高配置。
    let snapshot =
        generate(r#"<Skeleton width={placeholder_width} height={placeholder_height} />"#)
            // 受限表达式必须成功生成。
            .expect("动态 Skeleton 尺寸应生成");
    // 宽度表达式必须进入公开构建器。
    assert!(snapshot.contains("width (placeholder_width)"));
    // 高度表达式必须进入公开构建器。
    assert!(snapshot.contains("height (placeholder_height)"));
}

// 验证 Skeleton 拒绝未登记形状、动态形状、非法尺寸与子节点。
#[test]
fn rejects_invalid_skeleton_contracts() {
    // 未登记形状不能被静默降级为默认矩形。
    let shape = generate(r#"<Skeleton shape="capsule" />"#)
        // 非法关键字必须失败。
        .expect_err("未登记形状必须失败");
    // 诊断必须给出合法集合。
    assert!(shape.suggestion.contains("rect、circle 或 text"));
    // 动态形状无法确定映射到哪个公开枚举。
    let dynamic_shape = generate(r#"<Skeleton shape={loading_shape} />"#)
        // 动态关键字必须失败。
        .expect_err("动态形状必须失败");
    // 诊断必须说明字符串字面量要求。
    assert!(dynamic_shape.message.contains("字符串字面量"));
    // 非有限宽度不能进入运行时尺寸契约。
    let width = generate(r#"<Skeleton width="NaN" />"#)
        // 非有限字面量必须失败。
        .expect_err("非有限宽度必须失败");
    // 诊断必须说明有限值要求。
    assert!(width.message.contains("不是有限值"));
    // Skeleton 子节点不能被生成器丢弃。
    let child = generate(r#"<Skeleton><Text>lost</Text></Skeleton>"#)
        // 子树形状必须失败。
        .expect_err("Skeleton 子节点必须失败");
    // 诊断必须说明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}
