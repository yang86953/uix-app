//! 格式化的具体字节、结构、命令写入和实际组件文本合同。
#![cfg(feature = "lang-tools")]
use uix_app::lang::compiler::{CompilerSystem, component_source};

fn formatted(source: &str) -> String {
    let output = CompilerSystem::new()
        .format_inline(source, "untitled:中文😀")
        .unwrap();
    assert_eq!(output.cst.reconstruct(), output.formatted);
    assert_eq!(output.cst.source_id(), output.source_id);
    let second = CompilerSystem::new()
        .format_inline(&output.formatted, "untitled:中文😀")
        .unwrap();
    assert!(!second.changed, "{}", second.formatted);
    output.formatted
}

#[test]
fn code_blocks_and_type_expressions_have_canonical_layout() {
    let source = r#"import{Text}from"native";export component Main( title :String = "x" ){state count :Int=0;return <Text>{ title }</Text> ;}"#;
    assert_eq!(
        formatted(source),
        concat!(
            "import { Text } from \"native\";\n",
            "export component Main(title: String = \"x\") {\n",
            "  state count: Int = 0;\n",
            "  return <Text>{title}</Text>;\n",
            "}\n",
        )
    );
    let nested = "type Pair={left:Int;right:Array<Result<Int,String>>}; export component Main(a:Int=1){let f=(value:Int=3)=>{if(value>0){return value+a;}else{return - -value;}};let data={a};return <Text>{(f(a)).toString()}</Text>;}";
    let result = formatted(nested);
    assert!(result.contains("type Pair = { left: Int; right: Array<Result<Int, String>> };"));
    assert!(result.contains("if (value > 0) {\n      return value + a;\n    } else {"));
    assert!(result.contains("return - -value;"));
}

#[test]
fn comments_literals_and_parenthesized_jsx_keep_their_exact_payloads() {
    let source = r#"/* 顶部😀 */
export component Main(){
// SourceSpan { start: 42 } 不属于源区间
let text="Span { start: 1, end: 2 }\uD83D\uDE00 // /* end */"; // 尾部  
return (<Text>  原文 // " quote ' )  
  {text} {/* 子内容注释 */}尾部  </Text>);
}"#;
    let output = formatted(source);
    for payload in [
        "/* 顶部😀 */",
        "// SourceSpan { start: 42 } 不属于源区间",
        "// 尾部  ",
        r#""Span { start: 1, end: 2 }\uD83D\uDE00 // /* end */""#,
        "  原文 // \" quote ' )  \n  ",
        "{/* 子内容注释 */}尾部  ",
    ] {
        assert!(output.contains(payload), "丢失 {payload:?}: {output}");
    }
}

#[test]
fn component_fixtures_and_syntax_variants_round_trip_without_native_registration() {
    for source in [
        include_str!("fixtures/components/card.uix"),
        include_str!("fixtures/components/counter.uix"),
        include_str!("fixtures/components/editable_list.uix"),
        include_str!("fixtures/components/pruning.uix"),
        "export component Main(){return <Panel disabled onClick={()=>{}} value={1+2} />;}",
        "export component Main(){return <><A/>{/*hello*/}<B x={'literal'}/></>;}",
        "type F=(x:Int)=>Array<Int>; export component Main(){let a=[1,2]; return <T value={(a[0]+a[1])*2}/>;}",
        "export component Main(){return <Text>{ // 注释必须留在表达式内\n '中文' }</Text>;}",
        "export component Main(){return <Text data={/*a*/'value'/*b*/}>line\r\n  raw</Text>;}",
    ] {
        formatted(source);
    }
    // 参数前缀与括号表达式不能通过扫描整份 JSX 文本猜测箭头。
    for source in [
        "export component Main(){return (<Text>it's raw</Text>);}",
        "export component Main(){let f=(x:View=<Text>it's ) raw</Text>)=>x; return f();}",
        "export component Main(){return (true ? <Text>it's</Text> : <Text>other</Text>);}",
    ] {
        formatted(source);
    }
    let deep_valid = format!(
        "export function f():Int{{return {}1{};}}",
        "(".repeat(50),
        ")".repeat(50)
    );
    formatted(&deep_valid);
    for (open, close) in [("[", "]"), ("!", ""), ("()=>", "")] {
        formatted(&format!(
            "export component Main(){{return {}0{};}}",
            open.repeat(50),
            close.repeat(50)
        ));
        let too_deep = format!(
            "export component Main(){{return {}0{};}}",
            open.repeat(100),
            close.repeat(100)
        );
        assert_eq!(
            component_source::format(&too_deep, "depth.uix")
                .unwrap_err()
                .code,
            "component-depth-limit"
        );
    }
}

#[test]
fn syntax_and_format_budget_failures_never_return_a_candidate() {
    assert!(
        component_source::format("export component Main(){return <Text>broken;}", "bad.uix")
            .is_err()
    );
    let full = format!(
        "export component Main(){{return <Text>{}</Text>;}}",
        "a".repeat(1_048_570)
    );
    assert_eq!(
        component_source::format(&full, "large.uix")
            .unwrap_err()
            .code,
        "component-source-limit"
    );
    let prefix = "export component Main(){return <Text>";
    let suffix = "</Text>;}";
    let exact = format!(
        "{prefix}{}{suffix}",
        "x".repeat(1_048_576 - prefix.len() - suffix.len())
    );
    assert!(component_source::parse(&exact, "boundary.uix").is_ok());
    assert_eq!(
        component_source::format(&exact, "boundary.uix")
            .unwrap_err()
            .code,
        "component-format-limit"
    );
    let comments = format!(
        "{}export component Main(){{return <> </>;}}",
        "/*x*/".repeat(131_073)
    );
    assert_eq!(
        component_source::format(&comments, "tokens.uix")
            .unwrap_err()
            .code,
        "component-token-limit"
    );
}

#[cfg(feature = "uix-dynamic")]
#[test]
fn formatted_components_render_the_same_string_children_before_and_after_updates() {
    use std::collections::BTreeMap;
    use uix_app::lang::runtime::{self, components::*};
    let signature = ComponentSignature {
        parameters: vec![("children".into(), Type::Data(runtime::Type::String))],
        required: ["children".into()].into(),
    };
    let libraries = BTreeMap::from([(
        "native".into(),
        BTreeMap::from([("Text".into(), NativeExport::Component(signature.clone()))]),
    )]);
    let source = "import{Text}from'native';export component Main(value:String='初始'){return (<Text>  原文 it's ) //\n  {value} 尾部  </Text>);}";
    for source in [source.to_owned(), formatted(source)] {
        let checked = CompilerSystem::new()
            .check_component_inline(&source, "memory:render", &libraries)
            .unwrap()
            .checked;
        let program = component_source::lower(&checked, "Main").unwrap();
        let natives = BTreeMap::from([(
            ExportKey::new("native", "Text"),
            NativeBinding::Component(signature.clone()),
        )]);
        let mut engine = Engine::new(
            program,
            natives,
            BTreeMap::new(),
            ComponentLimits::default(),
        )
        .unwrap();
        for value in ["初始", "更新😀"] {
            if value != "初始" {
                engine
                    .update(
                        BTreeMap::from([(
                            "value".into(),
                            Value::data(runtime::Value::String(value.into())),
                        )]),
                        &runtime::Cancellation::default(),
                    )
                    .unwrap();
            }
            let ProjectedValue::Data(actual) = &engine.snapshot().roots[0].properties["children"]
            else {
                panic!()
            };
            assert_eq!(
                actual.as_ref(),
                &runtime::Value::String(format!("  原文 it's ) //\n  {value} 尾部  "))
            );
        }
        engine.close();
        assert!(engine.snapshot().roots.is_empty());
    }
}
