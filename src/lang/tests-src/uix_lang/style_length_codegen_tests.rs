// 引入核心 View 生成与文档解析入口。
use super::{generate_view, parse_document};

// 生成只含一条尺寸约束的元素并返回令牌文本。
fn tokens_for(property: &str, source: &str) -> Result<String, super::Diagnostic> {
    let document = parse_document(&format!(
        r#"<Container style="{property}: {source};"></Container>"#
    ))
    .expect("尺寸约束语法应合法");
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证四个约束属性把 px、百分比、关键字与 calc 保留为运行时 StyleLength。
#[test]
fn generates_size_bounds_with_preserved_units() {
    for (property, field) in [
        ("minWidth", "min_width"),
        ("maxWidth", "max_width"),
        ("minHeight", "min_height"),
        ("maxHeight", "max_height"),
    ] {
        let px = tokens_for(property, "240px").expect("px 约束应映射");
        assert!(px.contains(field), "{property} 应更新 {field}");
        assert!(
            px.contains("StyleLength :: Px (240"),
            "{property} 应保留 px：{px}"
        );

        let unitless = tokens_for(property, "16").expect("无单位数值按 px 映射");
        assert!(unitless.contains("StyleLength :: Px (16"));

        let percent = tokens_for(property, "50%").expect("百分比约束应映射");
        assert!(percent.contains("StyleLength :: Percent (50"), "{percent}");

        for keyword in ["auto", "none"] {
            let unconstrained = tokens_for(property, keyword).expect("auto/none 应映射");
            assert!(
                unconstrained.contains("StyleLength :: Auto"),
                "{keyword}: {unconstrained}"
            );
        }

        let calc = tokens_for(property, "calc(100% - 32px)").expect("有限 calc 应映射");
        assert!(calc.contains("StyleLength :: Calc"), "{calc}");
        assert!(calc.contains("px : - 32"), "px 项应折叠为 -32：{calc}");
        assert!(
            calc.contains("percent : 100"),
            "百分比项应折叠为 100：{calc}"
        );
    }
}

// 验证 calc 允许多项加减与主题数值 token 作为 px 项。
#[test]
fn calc_folds_literals_and_keeps_theme_tokens_as_px_terms() {
    let folded = tokens_for("maxWidth", "calc(50% + 8px - 2px + 10%)").expect("多项应折叠");
    assert!(folded.contains("px : 6"), "8-2 应折叠为 6：{folded}");
    assert!(
        folded.contains("percent : 60"),
        "50+10 应折叠为 60：{folded}"
    );

    let with_token = tokens_for("minWidth", "calc(100% - #padding)").expect("token 项应生效");
    assert!(
        with_token.contains("padding ()"),
        "token 应在运行期读取：{with_token}"
    );
    assert!(with_token.contains("percent : 100"), "{with_token}");
}

// 验证非法、非有限与不支持语法产生确定诊断而不是静默当 0。
#[test]
fn rejects_invalid_and_unsupported_size_bounds() {
    for (source, expect) in [
        ("-10px", "不能为负数"),
        ("-5%", "不能为负数"),
        ("NaN", "必须有限"),
        ("inf", "必须有限"),
        ("10em", "无法映射"),
        ("calc()", "不能为空"),
        ("calc(100% * 2)", "不支持乘除"),
        ("calc(100% / 2)", "不支持乘除"),
        ("calc(min(1px, 2px))", "不支持乘除"),
        ("calc(100% -32px)", "两侧带空格"),
        ("calc(100% -)", "缺少最后一项"),
        ("calc(100% 32px)", "两侧带空格"),
        ("fit-content", "无法映射"),
    ] {
        let error = tokens_for("maxWidth", source).expect_err(&format!("{source} 必须失败"));
        assert!(
            error.message.contains(expect),
            "{source} 的诊断应包含 {expect:?}，实际 {:?}",
            error.message
        );
    }
}

// 验证 width/height 仍保持 px/auto 集合，百分比与 calc 给出确定拒绝而非静默。
#[test]
fn width_and_height_keep_px_auto_support_set() {
    let error = tokens_for("width", "50%").expect_err("width 百分比应拒绝");
    assert!(error.message.contains("百分比"), "{:?}", error.message);
    let error = tokens_for("height", "calc(100% - 8px)").expect_err("height calc 应拒绝");
    assert!(error.message.contains("无法映射"), "{:?}", error.message);
}
