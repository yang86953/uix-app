// 引入待验证的文档解析与 View 生成入口。
use super::{generate_view, parse_document};

// 解析单根 UIX 文档并返回稳定令牌文本。
fn generate(source: &str) -> Result<String, super::Diagnostic> {
    // 解析输入文档。
    let document = parse_document(source)?;
    // 生成根 View 令牌。
    let tokens = generate_view(&document.root)?;
    // 使用过程宏令牌的稳定空白格式返回快照。
    Ok(tokens.to_string())
}

// 验证 Affix 复用公开组件、状态快照与唯一内容 View。
#[test]
fn generates_affix_contract() {
    // 构造覆盖动态偏移、State 绑定、公共尺寸与嵌套内容的 Affix。
    let source = r#"<Affix offsetTop={toolbar_height} scrollY={scroll_y} width="320px"><Container><Text>Toolbar</Text></Container></Affix>"#;
    // 生成确定性令牌快照。
    let tokens = generate(source).expect("Affix 文档契约应生成 Rust View");
    // 必须复用公开 Affix 构造器。
    assert!(tokens.contains("Affix :: new (toolbar_height)"));
    // scrollY 必须读取 State 当前快照，而不是复制状态所有权。
    assert!(tokens.contains("scroll_y ((scroll_y) . get ())"));
    // 唯一内容必须通过公开 ViewNode 组合。
    assert!(tokens.contains("ViewNode :: new") && tokens.contains("Toolbar"));
    // 公共宽度属性仍由统一 View 映射处理。
    assert!(tokens.contains("width (320"));
    // 生成物不得包含第二套 Affix 或运行时标签解析器。
    assert!(!tokens.contains("parse_affix"));
    // 缺省 offsetTop 必须使用文档规定的零值。
    let default = generate(r#"<Affix scrollY={scroll_y}><Text>Default</Text></Affix>"#)
        // 默认偏移路径也必须成功生成。
        .expect("Affix 默认 offsetTop 应可生成");
    // 默认值必须直接进入公开构造器。
    assert!(default.contains("Affix :: new (0.0_f32)"));
}

// 验证 Affix 在生成期拒绝缺失状态、非法绑定与不明确子树。
#[test]
fn validates_affix_contract_errors() {
    // 缺少 scrollY 时无法观察滚动事实。
    let missing_scroll = generate(r#"<Affix><Text>A</Text></Affix>"#)
        // 提取预期缺失绑定诊断。
        .expect_err("缺少 Affix scrollY 必须失败");
    // 诊断必须指向必需状态绑定写法。
    assert!(
        missing_scroll.message.contains("scrollY")
            && missing_scroll.suggestion.contains("scrollY={")
    );
    // 字符串不能表达 State<f32> 所有权。
    let literal_scroll = generate(r#"<Affix scrollY="12"><Text>A</Text></Affix>"#)
        // 提取预期绑定形状诊断。
        .expect_err("Affix scrollY 字面量必须失败");
    // 诊断必须明确要求 State<f32>。
    assert!(literal_scroll.message.contains("State<f32>"));
    // 空 Affix 没有可固钉的内容占位。
    let empty = generate(r#"<Affix scrollY={scroll_y} />"#)
        // 提取预期缺失内容诊断。
        .expect_err("空 Affix 必须失败");
    // 诊断必须说明唯一可渲染子节点要求。
    assert!(empty.message.contains("一个可渲染直接子节点"));
    // 多个直接内容节点不得被宏静默包裹。
    let multiple = generate(
        // 构造两个直接子节点。
        r#"<Affix scrollY={scroll_y}><Text>A</Text><Text>B</Text></Affix>"#,
    )
    // 提取预期父子形状诊断。
    .expect_err("多子节点 Affix 必须失败");
    // 修复建议必须要求显式布局容器。
    assert!(multiple.message.contains("只能包含一个") && multiple.suggestion.contains("Container"));
    // 未登记属性必须继续走统一公共属性拒绝路径。
    let unknown = generate(r#"<Affix scrollY={scroll_y} mystery="value"><Text>A</Text></Affix>"#)
        // 提取预期未知属性诊断。
        .expect_err("Affix 未登记属性必须失败");
    // 诊断必须保留具体属性名。
    assert!(unknown.message.contains("mystery"));
}
