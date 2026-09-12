// 引入文档解析与完整 View 生成入口。
use super::{generate_view, parse_document};

// 验证内联 boxShadow 同时支持旧语法与有符号 spread。
#[test]
fn generates_inline_box_shadow_spread_contract() {
    // 解析旧四段内联 boxShadow。
    let legacy_document = parse_document(
        // 保留既有语法作为兼容基线。
        r#"<Text style="boxShadow: 0 2px 4px #000;">Legacy</Text>"#,
    )
    // 旧语法必须继续完成解析。
    .expect("boxShadow 旧语法应合法");
    // 生成旧语法对应的 Rust 令牌。
    let legacy_tokens = generate_view(&legacy_document.root)
        // 旧语法不得因新增 spread 失效。
        .expect("boxShadow 旧语法应生成 Rust 令牌")
        // 规范化令牌便于断言。
        .to_string();
    // 旧语法必须显式保持零 spread。
    assert!(
        legacy_tokens.contains("with_spread (0.0)"),
        "{legacy_tokens}"
    );
    // 解析带负 spread 的内联 boxShadow。
    let spread_document = parse_document(
        // 使用五段语法覆盖可选有符号 spread。
        r#"<Text style="boxShadow: 0 2px 4px -1px #000;">Spread</Text>"#,
    )
    // 样式语法层必须接受 spread 分量。
    .expect("boxShadow spread 语法应合法");
    // 生成带完整 spread 的 Rust 令牌。
    let spread_tokens = generate_view(&spread_document.root)
        // 已登记 spread 不得返回规划中诊断。
        .expect("boxShadow spread 应生成 Rust 令牌")
        // 规范化令牌便于精确断言。
        .to_string();
    // 生成物必须保存负 spread。
    assert!(
        spread_tokens.contains("with_spread (- 1.0)"),
        "{spread_tokens}"
    );
}
