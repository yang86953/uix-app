// 引入文档解析与 View 生成入口。
use super::{generate_view, parse_document};

// 生成稳定令牌文本。
fn generate(source: &str) -> Result<String, super::Diagnostic> {
    // 解析单根文档。
    let document = parse_document(source)?;
    // 生成并规范化令牌文本。
    Ok(generate_view(&document.root)?.to_string())
}

// 验证完整应用布局壳、布尔简写与有序嵌套生成。
#[test]
fn generates_app_layout_shell_contract() {
    // 构造文档第六节的可执行横向应用壳。
    let tokens = generate(
        r#"<Layout direction="row"><Sider collapsible><Text>Nav</Text></Sider><Layout><Header><Text>Top</Text></Header><Content><Text>Main</Text></Content><Footer><Text>Bottom</Text></Footer></Layout></Layout>"#,
    )
    // 合法布局壳必须生成成功。
    .expect("应用布局壳文档契约应生成");
    // 外层 Layout 必须映射公开横向方向。
    assert!(tokens.contains("Layout :: new") && tokens.contains("FlexDirection :: Row"));
    // Sider 布尔简写必须生成 true。
    assert!(tokens.contains("Sider :: default") && tokens.contains("collapsible (true)"));
    // 四类内容区域必须全部复用公开运行时组件。
    assert!(
        tokens.contains("Header :: default")
            && tokens.contains("Content :: new")
            && tokens.contains("Footer :: default")
    );
    // 导航、顶栏、内容与底栏必须保持来源顺序。
    let nav = tokens.find("Nav").expect("应生成导航内容");
    // 取得顶栏位置。
    let top = tokens.find("Top").expect("应生成顶栏内容");
    // 取得主内容位置。
    let main = tokens.find("Main").expect("应生成主内容");
    // 取得底栏位置。
    let bottom = tokens.find("Bottom").expect("应生成底栏内容");
    // 断言来源顺序没有被宏重排。
    assert!(nav < top && top < main && main < bottom);
}

// 验证布局壳方向、布尔值与专有属性边界。
#[test]
fn validates_app_layout_shell_errors() {
    // 未登记方向不能进入运行时。
    let direction = generate(r#"<Layout direction="diagonal" />"#)
        // 提取预期方向诊断。
        .expect_err("非法 Layout 方向必须失败");
    // 诊断必须列出 row 与 column。
    assert!(direction.message.contains("row") && direction.suggestion.contains("column"));
    // collapsible 非布尔字面量必须失败。
    let collapsible = generate(r#"<Sider collapsible="sometimes" />"#)
        // 提取预期布尔诊断。
        .expect_err("非法 collapsible 值必须失败");
    // 诊断必须明确布尔值要求。
    assert!(collapsible.message.contains("布尔值"));
    // Header 不拥有 Layout 的 direction 专有属性。
    let wrong_owner = generate(r#"<Header direction="row" />"#)
        // 提取统一未知属性诊断。
        .expect_err("Header direction 必须失败");
    // 诊断必须保留错误属性名。
    assert!(wrong_owner.message.contains("direction"));
}
