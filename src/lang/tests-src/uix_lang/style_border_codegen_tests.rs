// 引入核心 View 生成与文档解析入口。
use super::{generate_view, parse_document};

// 验证 borderStyle 五种文档值映射到公开 UI 枚举。
#[test]
fn generates_border_style_runtime_mapping() {
    // 覆盖完整文档值与一一对应的运行时变体。
    for (source, variant) in [
        // 显式关闭边框绘制。
        ("none", "None"),
        // 显式实线。
        ("solid", "Solid"),
        // 连续相位虚线。
        ("dashed", "Dashed"),
        // 圆点线。
        ("dotted", "Dotted"),
        // 双线。
        ("double", "Double"),
    ] {
        // 构造只包含目标边框线型的元素文档。
        let document = parse_document(&format!(
            // 保留关键字源码供编译期映射。
            r#"<Text style="borderStyle: {source};">边框</Text>"#
        ))
        // 语法层必须接受规范样式值。
        .expect("borderStyle 语法应合法");
        // 生成真实 Style 字段更新令牌。
        let tokens = generate_view(&document.root)
            // 已映射线型不得继续返回规划中诊断。
            .expect("borderStyle 应映射到运行时线型")
            // 规范化令牌用于断言公开契约。
            .to_string();
        // 生成结果必须显式更新可选字段。
        assert!(tokens.contains("border_style"));
        // 枚举必须来自预lude 公开 BorderStyle 契约。
        assert!(tokens.contains(&format!("BorderStyle :: {variant}")));
    }
}

// 验证未知 borderStyle 值产生确定诊断。
#[test]
fn rejects_unknown_border_style_value() {
    // 构造 CSS 之外的未登记关键字。
    let document = parse_document(r#"<Text style="borderStyle: groove;">边框</Text>"#)
        // 样式语法层保留原始关键字供映射层诊断。
        .expect("borderStyle 原始值应完成语法解析");
    // 代码生成必须返回支持边界诊断。
    let error = generate_view(&document.root).expect_err("未知 borderStyle 必须失败");
    // 诊断必须包含文档允许值。
    assert!(error.message.contains("borderStyle 只支持"));
}

// 验证 borderRadius 单值保持旧标量字段生成物（S4 回归锚点）。
#[test]
fn generates_border_radius_single_value_scalar_field() {
    let document = parse_document(
        r#"<Text style="borderRadius: 8px;">边框</Text>"#,
    )
    .expect("borderRadius 单值语法应合法");
    let tokens = generate_view(&document.root)
        .expect("单值圆角应生成标量字段")
        .to_string();
    // 单值必须继续写既有标量字段。
    assert!(tokens.contains("border_radius = 8"), "{tokens}");
    // 单值同时清除四角形式（值清 None + 声明显式清角 Some(None)）。
    assert!(
        tokens.contains("border_radius_corners = :: std :: option :: Option :: None"),
        "{tokens}"
    );
    assert!(
        tokens.contains("border_radius_corners = :: std :: option :: Option :: Some"),
        "{tokens}"
    );
}

// 验证 borderRadius 四值展开写入四角字段并登记声明差异。
#[test]
fn generates_border_radius_four_values_corner_field() {
    let document = parse_document(
        r#"<Text style="borderRadius: 10px 20px 30px 40px;">边框</Text>"#,
    )
    .expect("borderRadius 四值语法应合法");
    let tokens = generate_view(&document.root)
        .expect("四值圆角应生成四角字段")
        .to_string();
    // 四角按 tl/tr/br/bl 顺序生成公开 CornerRadii 构造。
    assert!(
        tokens.contains("CornerRadii :: new (10.0 , 20.0 , 30.0 , 40.0)"),
        "{tokens}"
    );
    // 值样式与声明差异都持有四角形式（声明为 Some(Some(..))）。
    let corner_assignments = tokens
        .matches("border_radius_corners = :: std :: option :: Option :: Some")
        .count();
    assert!(corner_assignments >= 2, "{tokens}");
    assert!(
        tokens.contains("Option :: Some (:: std :: option :: Option :: Some"),
        "{tokens}"
    );
}

// 验证 borderRadius 两值与三值按 CSS 展开规则映射。
#[test]
fn generates_border_radius_two_and_three_value_expansion() {
    let two = parse_document(r#"<Text style="borderRadius: 20px 8px;">a</Text>"#)
        .expect("两值语法应合法");
    let tokens = generate_view(&two.root).expect("两值应生成四角").to_string();
    // 两值：tl=br=20、tr=bl=8。
    assert!(
        tokens.contains("CornerRadii :: new (20.0 , 8.0 , 20.0 , 8.0)"),
        "{tokens}"
    );
    let three = parse_document(r#"<Text style="borderRadius: 10px 20px 30px;">a</Text>"#)
        .expect("三值语法应合法");
    let tokens = generate_view(&three.root).expect("三值应生成四角").to_string();
    // 三值：tl=10、tr/bl=20、br=30。
    assert!(
        tokens.contains("CornerRadii :: new (10.0 , 20.0 , 30.0 , 20.0)"),
        "{tokens}"
    );
}

// 验证超过四个分量与负值半径产生确定诊断。
#[test]
fn rejects_invalid_border_radius_values() {
    let five = parse_document(r#"<Text style="borderRadius: 1px 2px 3px 4px 5px;">a</Text>"#)
        .expect("语法层保留原始值");
    let error = generate_view(&five.root).expect_err("五分量必须失败");
    assert!(
        error.message.contains("borderRadius 最多接受四个分量"),
        "{}",
        error.message
    );
    let negative =
        parse_document(r#"<Text style="borderRadius: 8px -4px;">a</Text>"#).expect("保留负值");
    let error = generate_view(&negative.root).expect_err("负值半径必须失败");
    assert!(
        error.message.contains("无法映射为 f32") || error.message.contains("负"),
        "{}",
        error.message
    );
}
