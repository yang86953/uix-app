// 引入文档解析与核心 View 生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 解析单根文档并生成稳定令牌文本。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 生成根 View 令牌并规范为空白稳定文本。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证空白与注释不打断相邻分支，且生成单次短路链。
#[test]
fn generates_adjacent_short_circuit_conditional_chain() {
    // 构造带空白和注释间隔的三分支条件链。
    let source = r#"
        <Container>
            <If {primary}><Text>A</Text></If>
            /* 条件分支间注释 */
            <ElseIf {secondary}><Text>B</Text></ElseIf>
            // 兜底分支前注释
            <Else><Text>C</Text></Else>
        </Container>
    "#;
    // 生成条件链令牌。
    let tokens = generate(source).expect("相邻 If/ElseIf/Else 应生成成功");
    // 条件必须按源码顺序进入嵌套短路结构。
    assert!(tokens.contains("if primary") && tokens.contains("else if secondary"));
    // 无条件尾部分支必须保留。
    assert!(tokens.contains("else {") && tokens.contains("\"C\""));
    // 三个分支内容都只生成一次。
    assert_eq!(tokens.matches("\"A\"").count(), 1);
    // 第二分支内容也只生成一次。
    assert_eq!(tokens.matches("\"B\"").count(), 1);
    // 兜底内容也只生成一次。
    assert_eq!(tokens.matches("\"C\"").count(), 1);
}

// 验证没有前导 If 的尾部分支返回定向诊断。
#[test]
fn rejects_isolated_else_if_and_else() {
    // 孤立 ElseIf 必须失败。
    let else_if = generate(
        // 构造缺少前导 If 的条件分支。
        r#"<Container><ElseIf {ready}><Text>A</Text></ElseIf></Container>"#,
    )
    // 提取预期诊断。
    .expect_err("孤立 ElseIf 必须失败");
    // 诊断必须说明相邻前导 If 要求。
    assert!(else_if.message.contains("孤立") && else_if.suggestion.contains("紧接"));
    // 孤立 Else 必须失败。
    let r#else = generate(r#"<Container><Else><Text>B</Text></Else></Container>"#)
        // 提取预期诊断。
        .expect_err("孤立 Else 必须失败");
    // 诊断必须同样说明缺少前导 If。
    assert!(r#else.message.contains("孤立") && r#else.message.contains("Else"));
}

// 验证其他可渲染节点会打断配对，且 Else 只能出现一次。
#[test]
fn rejects_interrupted_and_duplicate_else_branches() {
    // 在 If 与 Else 之间插入普通元素。
    let interrupted = generate(
        // 构造被普通 Text 打断的分支。
        r#"<Container><If {ready}>A</If><Text>gap</Text><Else>B</Else></Container>"#,
    )
    // 提取预期孤立分支诊断。
    .expect_err("普通元素间隔必须打断条件链");
    // 诊断必须指向孤立 Else。
    assert!(interrupted.message.contains("孤立 <Else>"));
    // 同一条件链声明两个 Else。
    let duplicate = generate(
        // 构造重复尾部分支。
        r#"<Container><If {ready}>A</If><Else>B</Else><Else>C</Else></Container>"#,
    )
    // 提取预期重复诊断。
    .expect_err("重复 Else 必须失败");
    // 诊断必须明确条件链只能有一个尾部分支。
    assert!(duplicate.message.contains("只能有一个尾部分支"));
}

// 验证 ElseIf 必须带条件且 Else 不接受属性。
#[test]
fn validates_conditional_branch_shapes() {
    // 缺少条件的 ElseIf 在解析期失败。
    let missing = parse_document(
        // 构造无条件 ElseIf。
        r#"<Container><If {ready}>A</If><ElseIf>B</ElseIf></Container>"#,
    )
    // 提取预期绑定诊断。
    .expect_err("ElseIf 缺少条件必须失败");
    // 诊断必须点名控制绑定。
    assert!(missing.message.contains("ElseIf") && missing.message.contains("缺少控制绑定"));
    // Else 普通属性在解析期失败。
    let attribute = parse_document(
        // 构造带非法属性的 Else。
        r#"<Container><If {ready}>A</If><Else visible="true">B</Else></Container>"#,
    )
    // 提取预期形状诊断。
    .expect_err("Else 属性必须失败");
    // 诊断必须说明 Else 不接受属性。
    assert!(attribute.message.contains("Else 不接受"));
}
