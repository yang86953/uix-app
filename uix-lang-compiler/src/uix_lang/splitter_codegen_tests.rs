// 引入文档解析与 View 生成入口。
use super::{generate_view, parse_document};

// 生成稳定令牌文本。
fn generate(source: &str) -> Result<String, super::Diagnostic> {
    // 解析单根文档。
    let document = parse_document(source)?;
    // 生成并规范化令牌文本。
    Ok(generate_view(&document.root)?.to_string())
}

// 验证 Splitter 方向、动态比例、双面板顺序与公共属性映射。
#[test]
fn generates_splitter_contract() {
    // 构造覆盖全部专有属性与两个不同内容面板的文档。
    let tokens = generate(
        r#"<Splitter direction="vertical" defaultRatio={ratio} height="480px"><Container><Text>First</Text></Container><Container><Text>Second</Text></Container></Splitter>"#,
    )
    // 合法契约必须成功生成。
    .expect("Splitter 文档契约应生成");
    // 必须复用公开运行时构造器与纵向轴构建器。
    assert!(tokens.contains("Splitter :: new") && tokens.contains("vertical (true)"));
    // 动态比例必须映射到运行时归一化入口。
    assert!(tokens.contains("default_ratio (ratio)"));
    // 两个内容面板必须通过公开 ViewNode 组合。
    assert!(tokens.contains("ViewNode :: new") && tokens.contains("View :: build"));
    // 来源顺序必须稳定保留。
    assert!(tokens.find("First") < tokens.find("Second"));
    // 公共高度属性仍由统一 View 映射处理。
    assert!(tokens.contains("height (480"));
}

// 验证 Splitter 的双面板形状、方向枚举与字面比例诊断。
#[test]
fn validates_splitter_contract_errors() {
    // 单面板不能满足双面板契约。
    let one = generate(r#"<Splitter><Text>A</Text></Splitter>"#)
        // 提取预期结构诊断。
        .expect_err("单面板 Splitter 必须失败");
    // 诊断必须报告两个面板要求与实际数量。
    assert!(one.message.contains("恰好") && one.message.contains("1 个"));
    // 三个直接面板不得被静默截断。
    let three = generate(
        // 构造三个可渲染直接子节点。
        r#"<Splitter><Text>A</Text><Text>B</Text><Text>C</Text></Splitter>"#,
    )
    // 提取预期结构诊断。
    .expect_err("三面板 Splitter 必须失败");
    // 诊断必须保留实际数量。
    assert!(three.message.contains("3 个"));
    // 未登记方向不能被猜测为任一轴向。
    let direction =
        generate(r#"<Splitter direction="diagonal"><Text>A</Text><Text>B</Text></Splitter>"#)
            // 提取预期方向诊断。
            .expect_err("非法 Splitter 方向必须失败");
    // 诊断与建议必须列出合法方向。
    assert!(direction.message.contains("horizontal") && direction.suggestion.contains("vertical"));
    // 越界字面比例必须在宏展开时失败。
    let ratio = generate(r#"<Splitter defaultRatio="1.2"><Text>A</Text><Text>B</Text></Splitter>"#)
        // 提取预期比例诊断。
        .expect_err("越界 Splitter 比例必须失败");
    // 诊断必须说明闭区间边界。
    assert!(ratio.message.contains("0~1"));
}
