// 引入组件声明联合、文档生成与解析入口。
use super::{Declaration, Diagnostic, generate_document_view, parse_document};

// 解析完整文档并生成稳定令牌文本。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 解析组件声明与调用根。
    let document = parse_document(source)?;
    // 展开组件并规范令牌空白。
    generate_document_view(&document).map(|tokens| tokens.to_string())
}

// 验证字面量与数据构造默认值会在调用省略时生成。
#[test]
fn generates_literal_and_data_constructor_defaults() {
    // 构造包含字符串与日期默认值的组件。
    let source = r#"
        <Widget name="Defaults" props="label: String = '确定', date: Date = Date(2026, 8, 15), required: bool">
            <Column><Text>{label}</Text><Text>{date.year}</Text><Text>{required}</Text></Column>
        </Widget>
        <Defaults required="true" />
    "#;
    // 生成省略可选 prop 的组件调用。
    let tokens = generate(source).expect("字面量与数据构造默认值应生成成功");
    // 字符串默认值必须成为拥有所有权的 String。
    assert!(tokens.contains("String :: from (\"确定\")"));
    // 日期默认值必须复用已登记数据构造映射。
    assert!(tokens.contains("Date :: new") && tokens.contains("2026"));
    // 无默认值的必填布尔仍使用调用方显式值。
    assert!(tokens.contains("let __uix_prop") && tokens.contains("true"));
}

// 验证显式调用参数覆盖默认值，且 AST 保留可选事实。
#[test]
fn explicit_prop_overrides_default_and_ast_tracks_optional() {
    // 构造带默认 label 且显式覆盖的组件。
    let source = r#"
        <Widget name="Greeting" props="label: String = '默认'"><Text>{label}</Text></Widget>
        <Greeting label="显式" />
    "#;
    // 解析文档以检查声明事实。
    let document = parse_document(source).expect("默认 prop 声明应解析成功");
    // 提取首个组件声明。
    let Declaration::Widget(widget) = &document.declarations[0] else {
        // 非组件声明表示解析路由错误。
        panic!("首个声明应为组件");
    };
    // 默认表达式必须保存在对应 prop 上。
    assert!(widget.props[0].default.is_some());
    // 生成显式覆盖调用。
    let tokens = generate(source).expect("显式覆盖默认值应生成成功");
    // 生成物只应包含显式值，不应求值默认值。
    assert!(tokens.contains("显式") && !tokens.contains("默认"));
}

// 验证无默认值 prop 仍为必填。
#[test]
fn rejects_missing_required_prop() {
    // 省略没有默认值的 required。
    let error = generate(
        // 构造一个可选和一个必填 prop。
        r#"<Widget name="Mixed" props="label: String = '默认', required: bool"><Text>{label}</Text></Widget><Mixed />"#,
    )
    // 提取预期必填诊断。
    .expect_err("缺失无默认值 prop 必须失败");
    // 诊断必须只点名 required。
    assert!(error.message.contains("必需 prop required"));
}

// 验证默认表达式白名单与所有权类别。
#[test]
fn rejects_dynamic_state_and_callback_defaults() {
    // 普通标识符不能作为默认表达式。
    let dynamic = parse_document(
        // 构造读取外部名称的默认值。
        r#"<Widget name="Bad" props="label: String = externalValue"><Text>{label}</Text></Widget><Bad />"#,
    )
    // 提取预期白名单诊断。
    .expect_err("动态默认值必须失败");
    // 诊断必须指出字面量或数据构造边界。
    assert!(dynamic.message.contains("字面量或已登记数据类型构造"));
    // State 句柄不能用普通值伪造默认值。
    let state = parse_document(
        // 构造非法 State 默认值。
        r#"<Widget name="Bad" props="value: State<String> = 'x'"><Text>A</Text></Widget><Bad />"#,
    )
    // 提取预期所有权诊断。
    .expect_err("State 默认值必须失败");
    // 诊断必须点名 State 类别。
    assert!(state.message.contains("State 或回调类型不能声明默认值"));
    // 回调同样必须由调用方显式提供。
    let callback = parse_document(
        // 构造非法回调默认值。
        r#"<Widget name="Bad" props="onClick: () = true"><Text>A</Text></Widget><Bad />"#,
    )
    // 提取预期所有权诊断。
    .expect_err("回调默认值必须失败");
    // 诊断必须沿用同一所有权规则。
    assert!(callback.message.contains("State 或回调类型不能声明默认值"));
}
