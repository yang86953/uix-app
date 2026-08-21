// 引入核心 View 生成与文档解析入口。
use super::{generate_view, parse_document};

// 验证裸名称、带空格名称与转义名称保持声明顺序。
#[test]
fn generates_ordered_font_family_runtime_mapping() {
    // 构造同时覆盖单引号、裸名称、通用族和转义引号的文档。
    let document = parse_document(
        // fontFamily 列表中的逗号只在单引号外分隔名称。
        r#"<Text style="fontFamily: 'Segoe UI', Arial, 'A\'Font', sans-serif;">字体</Text>"#,
    )
    // 语法层必须接受规范字体族列表。
    .expect("fontFamily 语法应合法");
    // 生成真实 Style 字段更新令牌。
    let tokens = generate_view(&document.root)
        // 已映射字体族不得继续返回规划中诊断。
        .expect("fontFamily 应映射到运行时字体族")
        // 规范化令牌用于断言公开契约和声明顺序。
        .to_string();
    // 生成结果必须显式更新可选字段。
    assert!(tokens.contains("font_family"));
    // 生成结果必须通过公开受控构造器建立列表。
    assert!(tokens.contains("FontFamily :: from_names"));
    // 读取四个名称在令牌中的位置。
    let segoe = tokens.find("Segoe UI").expect("应保留带空格名称");
    // 读取第二个裸名称位置。
    let arial = tokens.find("Arial").expect("应保留裸名称");
    // 读取转义单引号名称位置。
    let escaped = tokens.find("A'Font").expect("应解码转义单引号");
    // 读取最后一个通用族位置。
    let generic = tokens.find("sans-serif").expect("应保留通用字体族");
    // 生成顺序必须与源码回退顺序完全一致。
    assert!(segoe < arial && arial < escaped && escaped < generic);
}

// 验证空分量、错误引号形状和控制字符产生确定诊断。
#[test]
fn rejects_invalid_font_family_lists() {
    // 覆盖连续逗号、前后空分量、引号后杂项与控制字符。
    for source in [
        // 连续逗号形成中间空名称。
        "Arial,,sans-serif",
        // 开头逗号形成首个空名称。
        ",Arial",
        // 末尾逗号形成最后空名称。
        "Arial,",
        // 单引号名称后不能拼接裸文本。
        "'Segoe UI' extra, sans-serif",
        // 控制字符不能进入运行时注册表查询。
        "Arial, bad\u{0007}name",
    ] {
        // 构造保留当前非法列表的元素文档。
        let document = parse_document(&format!(
            // 把当前非法值放入统一样式入口。
            r#"<Text style="fontFamily: {source};">字体</Text>"#
        ))
        // 语法层只负责保留能够闭合的原始值。
        .expect("fontFamily 原始值应完成语法解析");
        // 代码生成必须在列表映射阶段失败。
        let error = generate_view(&document.root).expect_err("非法 fontFamily 必须失败");
        // 诊断必须明确说明非空列表形态。
        assert!(error.message.contains("fontFamily 必须是"));
    }
}
