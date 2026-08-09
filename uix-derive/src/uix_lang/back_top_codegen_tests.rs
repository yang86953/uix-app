// 引入文档解析与 View 生成入口。
use super::{generate_view, parse_document};

// 生成稳定令牌文本。
fn generate(source: &str) -> Result<String, super::Diagnostic> {
    // 解析单根文档。
    let document = parse_document(source)?;
    // 生成并规范化令牌文本。
    Ok(generate_view(&document.root)?.to_string())
}

// 验证 BackTop 阈值、状态绑定与公共属性映射。
#[test]
fn generates_back_top_contract() {
    // 构造完整文档属性组合。
    let tokens = generate(r#"<BackTop threshold={limit} scrollY={scroll_y} margin="8px" />"#)
        // 合法契约必须生成成功。
        .expect("BackTop 文档契约应生成");
    // 必须复用公开组件构造器与阈值方法。
    assert!(tokens.contains("BackTop :: new") && tokens.contains("visibility_height (limit)"));
    // 必须把 State 句柄借给公开双向绑定入口。
    assert!(tokens.contains("scroll_state (& (scroll_y))"));
    // 公共样式必须继续走统一 View 映射。
    assert!(tokens.contains("margin"));
}

// 验证 BackTop 叶形状与状态绑定诊断。
#[test]
fn validates_back_top_contract_errors() {
    // 缺少滚动状态必须失败。
    let missing = generate(r#"<BackTop />"#).expect_err("缺少 scrollY 必须失败");
    // 修复建议必须给出绑定写法。
    assert!(missing.message.contains("scrollY") && missing.suggestion.contains("scrollY={"));
    // 字面量不能表达 State 所有权。
    let literal = generate(r#"<BackTop scrollY="450" />"#).expect_err("字面 scrollY 必须失败");
    // 诊断必须明确状态类型。
    assert!(literal.message.contains("State<f32>"));
    // 可见子节点不得被静默丢弃。
    let child = generate(r#"<BackTop scrollY={scroll_y}><Text>A</Text></BackTop>"#)
        // 提取叶组件诊断。
        .expect_err("BackTop 子节点必须失败");
    // 诊断必须指出叶组件形状。
    assert!(child.message.contains("不接受子节点"));
}
