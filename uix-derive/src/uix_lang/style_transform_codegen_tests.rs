// 引入核心 View 生成与文档解析入口。
use super::{generate_view, parse_document};

// 验证 transformOrigin 的关键字、百分比、像素与零深度映射。
#[test]
fn generates_transform_origin_runtime_mapping() {
    // 解析垂直关键字前置和带零深度的混合原点。
    for source in ["top left", "25% 12px 0"] {
        // 构造只包含目标原点的元素文档。
        let document = parse_document(&format!(
            // 保留原点源码供编译期映射。
            r#"<Text style="transformOrigin: {source};">原点</Text>"#
        ))
        // 语法层必须接受规范样式值。
        .expect("transformOrigin 语法应合法");
        // 生成真实 View 调用令牌。
        let tokens = generate_view(&document.root)
            // 已映射原点不得继续返回规划中诊断。
            .expect("transformOrigin 应映射到运行时原点")
            // 规范化令牌用于断言公开契约。
            .to_string();
        // 最终节点必须通过唯一公开原点入口更新。
        assert!(tokens.contains("transform_origin"));
        // 原点必须由 UI System 公开值类型构造。
        assert!(tokens.contains("TransformOrigin :: new"));
    }
}

// 验证 transformOrigin 拒绝轴冲突、非零深度与未知单位。
#[test]
fn rejects_invalid_transform_origin_values() {
    // 覆盖重复水平轴、三维能力差距、单位和分量数量。
    for (value, expected) in [
        // 两个水平关键字无法确定垂直轴。
        ("left right", "水平与垂直"),
        // 二维运行时不接受非零 Z 轴。
        ("center center 1px", "Z 轴"),
        // 未登记 em 单位不能静默换算。
        ("10em 20px", "有效数值"),
        // 四分量超过文档契约。
        ("left top 0 0", "一到三个"),
    ] {
        // 构造单一待拒绝原点。
        let source = format!(r#"<Text style="transformOrigin: {value};">原点</Text>"#);
        // 样式语法层保留原始值供映射层诊断。
        let document = parse_document(&source).expect("原点原始值应完成语法解析");
        // 代码生成必须返回确定诊断。
        let error = generate_view(&document.root).expect_err("非法原点必须失败");
        // 诊断必须命中对应失败原因。
        assert!(error.message.contains(expected), "{}", error.message);
    }
}
