// 集中验证 backgroundSize 关键字、轴尺寸与拒绝路径。

// 引入核心 View 生成与文档解析入口。
use super::{generate_view, parse_document};

// 验证三个关键字映射到公开 BackgroundSize 枚举。
#[test]
fn generates_background_size_keywords() {
    for (source, variant) in [
        ("auto", "Auto"),
        ("cover", "Cover"),
        ("contain", "Contain"),
    ] {
        let document = parse_document(&format!(
            r#"<Text style="backgroundSize: {source};">a</Text>"#
        ))
        .expect("关键字语法应合法");
        let tokens = generate_view(&document.root)
            .expect("关键字应映射到 BackgroundSize")
            .to_string();
        assert!(
            tokens.contains(&format!("BackgroundSize :: {variant}")),
            "{tokens}"
        );
    }
}

// 验证显式像素、百分比与单轴 auto 组合映射到逐轴尺寸。
#[test]
fn generates_background_size_axis_values() {
    // 逐条断言（展开写避免元组形状歧义）。
    let document = parse_document(r#"<Text style="backgroundSize: 60px 40px;">a</Text>"#)
        .expect("两轴像素应合法");
    let tokens = generate_view(&document.root).expect("应生成显式尺寸").to_string();
    assert!(
        tokens.contains("BackgroundSize :: Explicit") && tokens.contains("Px (60.0)") && tokens
            .contains("Px (40.0)"),
        "{tokens}"
    );
    let document = parse_document(r#"<Text style="backgroundSize: 50% auto;">a</Text>"#)
        .expect("百分比与 auto 混用应合法");
    let tokens = generate_view(&document.root).expect("应生成混合轴").to_string();
    assert!(
        tokens.contains("Percent (0.5)") && tokens.contains("AxisSize :: Auto"),
        "{tokens}"
    );
    let document = parse_document(r#"<Text style="backgroundSize: 100px;">a</Text>"#)
        .expect("单轴尺寸应合法");
    let tokens = generate_view(&document.root).expect("应生成单轴显式").to_string();
    assert!(
        tokens.contains("Px (100.0)") && tokens.contains("AxisSize :: Auto"),
        "{tokens}"
    );
}

// 验证非法分量（关键字混用、三分量、负值、未知单位）产生确定诊断。
#[test]
fn rejects_invalid_background_size_values() {
    let invalid_sources = [
        "cover contain",
        "50px 40px 30px",
        "-50%",
        "50em",
        "50",
    ];
    for source in invalid_sources {
        let document = parse_document(&format!(
            r#"<Text style="backgroundSize: {source};">a</Text>"#
        ))
        .expect("语法层保留原始值");
        let error =
            generate_view(&document.root).expect_err("backgroundSize 必须被拒绝");
        assert!(
            error.message.contains("backgroundSize"),
            "{}",
            error.message
        );
    }
}
