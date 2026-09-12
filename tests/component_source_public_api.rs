//! 新组件源码的公开前端合同；这些测试不把解析成功当作可执行或类型受检。
#![cfg(feature = "lang-build")]

use uix_app::lang::compiler::{
    DiagnosticPhase,
    component_source::{self as source, *},
    source_graph::SourceId,
};

const COUNTER: &str = include_str!("fixtures/components/counter.uix");
const CARD: &str = include_str!("fixtures/components/card.uix");
const LIST: &str = include_str!("fixtures/components/editable_list.uix");

fn returned(parsed: &ParsedSource, name: &str) -> Expr {
    let declaration = parsed
        .declarations
        .iter()
        .find(|item| item.name.text == name)
        .unwrap();
    let statement = match &declaration.kind {
        DeclarationKind::Function(function) => function.body.statements.last().unwrap(),
        DeclarationKind::Component { body, .. } => {
            let ComponentMember::Statement(statement) = body.last().unwrap() else {
                panic!("expected return");
            };
            statement
        }
        _ => panic!("expected callable"),
    };
    let StatementKind::Return(Some(value)) = &statement.kind else {
        panic!("expected return value");
    };
    value.clone()
}

#[test]
fn reusable_components_are_declarations_with_typed_state_callbacks_and_view_children() {
    let counter = source::parse(COUNTER, "counter.uix").unwrap();
    assert_eq!(counter.source_graph.files()[0].source, COUNTER);
    assert_eq!(
        counter.source_graph.root(),
        SourceId::from_source_name("counter.uix")
    );
    assert_eq!(counter.declarations.len(), 1); // 不要求另放一个 Counter 根实例。
    let DeclarationKind::Component { parameters, body } = &counter.declarations[0].kind else {
        panic!();
    };
    assert_eq!(parameters[0].name.text, "initial");
    assert!(matches!(
        parameters[0].default.as_ref().unwrap().kind,
        ExprKind::Integer(0)
    ));
    let ComponentMember::State { name, initial, .. } = &body[0] else {
        panic!();
    };
    assert_eq!(name.text, "count");
    assert!(matches!(&initial.kind, ExprKind::Name(name) if name.text == "initial"));
    let ComponentMember::Function { function, .. } = &body[1] else {
        panic!();
    };
    assert!(matches!(
        function.body.statements[0].kind,
        StatementKind::Assign { .. }
    ));
    let ExprKind::Element(root) = returned(&counter, "Counter").kind else {
        panic!();
    };
    let button = root
        .children
        .iter()
        .find_map(|child| match child {
            ViewChild::Expression(Expr {
                kind: ExprKind::Element(element),
                ..
            }) if element.name[0].text == "Button" => Some(element),
            _ => None,
        })
        .unwrap();
    assert!(
        matches!(&button.attributes[0].value.kind, ExprKind::Name(name) if name.text == "increment")
    );

    let card = source::parse(CARD, "card.uix").unwrap();
    assert_eq!(
        card.declarations
            .iter()
            .filter(|item| item.exported)
            .count(),
        2
    );
    let DeclarationKind::Component { parameters, .. } = &card.declarations[0].kind else {
        panic!();
    };
    assert!(
        matches!(&parameters[1].ty.as_ref().unwrap().kind, TypeKind::Named { path, .. } if path[0].text == "View")
    );

    let list = source::parse(LIST, "editable_list.uix").unwrap();
    assert_eq!(list.imports[2].names[0].imported.text, "Counter");
    assert_eq!(list.imports[2].names[0].local.text, "Count");
    assert_eq!(
        list.declarations
            .iter()
            .map(|item| (item.name.text.as_str(), item.exported))
            .collect::<Vec<_>>(),
        [("Item", true), ("Row", false), ("EditableList", true)]
    );
    let DeclarationKind::Component { parameters, body } = &list.declarations[2].kind else {
        panic!();
    };
    assert!(matches!(
        parameters[0].ty.as_ref().unwrap().kind,
        TypeKind::Array(_)
    ));
    let ComponentMember::Function { function, .. } = &body[1] else {
        panic!();
    };
    let StatementKind::Assign { value, .. } = &function.body.statements[0].kind else {
        panic!();
    };
    let ExprKind::Call { arguments, .. } = &value.kind else {
        panic!();
    };
    assert!(
        matches!(&arguments[0].kind, ExprKind::Lambda { body: LambdaBody::Expression(expr), .. } if matches!(expr.kind, ExprKind::Conditional { .. }))
    );
}

#[test]
fn expression_precedence_and_text_are_not_encoded_as_xml_attributes() {
    let parsed = source::parse(r#"export function value(): Int { return 1 + 2 * 3 - -4; }
export component Greeting() { return <Text enabled label={'\uD83D\uDE80'}>中文 // 原文 {"\\"}</Text>; }"#, "text.uix").unwrap();
    let ExprKind::Binary {
        op: BinaryOp::Subtract,
        left,
        right,
    } = returned(&parsed, "value").kind
    else {
        panic!();
    };
    assert!(
        matches!(left.kind, ExprKind::Binary { op: BinaryOp::Add, right, .. } if matches!(right.kind, ExprKind::Binary { op: BinaryOp::Multiply, .. }))
    );
    assert!(matches!(
        right.kind,
        ExprKind::Unary {
            op: UnaryOp::Negate,
            ..
        }
    ));
    let ExprKind::Element(element) = returned(&parsed, "Greeting").kind else {
        panic!();
    };
    assert!(matches!(
        element.attributes[0].value.kind,
        ExprKind::Bool(true)
    ));
    assert!(matches!(&element.attributes[1].value.kind, ExprKind::String(value) if value == "🚀"));
    assert!(
        matches!(&element.children[0], ViewChild::Text { value, .. } if value == "中文 // 原文 ")
    );
}

#[test]
fn errors_keep_original_utf8_range_and_scalar_line_column() {
    let invalid = "export component 页面() {\n  return <Text>中文🚀</Button>;\n}";
    let error = source::parse(invalid, "目录/错误.uix").unwrap_err();
    assert_eq!(error.code, "component-closing-tag");
    assert_eq!(error.phase, DiagnosticPhase::Syntax);
    assert_eq!(error.source_name, "目录/错误.uix");
    assert_eq!(error.start, invalid.find("Button").unwrap());
    assert_eq!(&invalid[error.start..error.end], "B");
    assert_eq!((error.line, error.column), (2, 21));
    let eof = source::parse("export component A() {", "eof").unwrap_err();
    assert_eq!(eof.start, eof.end);
}

#[test]
fn malformed_or_foreign_syntax_does_not_succeed_as_a_partial_parse() {
    for invalid in [
        "<Widget name=\"Legacy\"><Text /></Widget>",
        "export component A(value) { return <Text/>; }",
        "export component A() { state x: Int = 0 return <Text/>; }",
        "export component A() { return <Button onClick={move |_| {}}/>; }",
        "export component A() { return <Text a={1}b={2}/>; }",
        "export component A() { return <Text value={1}/>; } trailing",
        "export function f(): Float { return 1e309; }",
        "export function f(): Int { return 18446744073709551615; }",
        r#"export function f(): String { return "\uDC00"; }"#,
        "/* unfinished",
    ] {
        assert!(
            source::parse(invalid, "invalid").is_err(),
            "unexpected success: {invalid}"
        );
    }
}

#[test]
fn incomplete_editor_inputs_never_panic_and_diagnostics_stay_on_character_boundaries() {
    for text in [COUNTER, CARD, LIST] {
        for offset in (0..=text.len()).filter(|offset| text.is_char_boundary(*offset)) {
            if let Err(error) = source::parse(&text[..offset], "editor.uix") {
                assert!(error.start <= error.end && error.end <= offset);
                assert!(text.is_char_boundary(error.start) && text.is_char_boundary(error.end));
            }
        }
        // 编辑器还会留下中间缺字，而不仅仅是 EOF 截断。
        for (offset, ch) in text.char_indices().step_by(3) {
            let modified = format!("{}{}", &text[..offset], &text[offset + ch.len_utf8()..]);
            if let Err(error) = source::parse(&modified, "editor.uix") {
                assert!(error.start <= error.end && error.end <= modified.len());
                assert!(
                    modified.is_char_boundary(error.start) && modified.is_char_boundary(error.end)
                );
            }
        }
    }
}

#[test]
fn hostile_depth_width_and_size_are_rejected_before_unbounded_trees_are_built() {
    let mut nested_chain = "0".to_owned();
    for _ in 0..20 {
        nested_chain = format!("({nested_chain}){}", "+1".repeat(10));
    }
    let sources = [
        format!(
            "export function f(): Int {{ return {}0{}; }}",
            "(".repeat(200),
            ")".repeat(200)
        ),
        format!("export function f(): Int {{ return {nested_chain}; }}"),
        format!(
            "export component A() {{ return {}0{}; }}",
            "<Text>".repeat(100),
            "</Text>".repeat(100)
        ),
        format!("type X = {}Int{};", "Array<".repeat(100), ">".repeat(100)),
        format!(
            "export function f(): Unit {{ {}return;{} }}",
            "if (true) {".repeat(100),
            "}".repeat(100)
        ),
        format!(
            "export function f(): Int {{ return x{}; }}",
            ".x".repeat(300)
        ),
    ];
    for text in sources {
        assert_eq!(
            source::parse(&text, "depth").unwrap_err().code,
            "component-depth-limit"
        );
    }
    assert_eq!(
        source::parse(&" ".repeat(1_048_577), "size")
            .unwrap_err()
            .code,
        "component-source-limit"
    );
    assert_eq!(
        source::parse(&"type X = Int;\n".repeat(10_000), "width")
            .unwrap_err()
            .code,
        "component-node-limit"
    );
}
