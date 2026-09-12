//! 公开索引验证语义身份，而非把所有同名 token 当作引用。
#![cfg(feature = "lang-build")]
use std::collections::BTreeMap;
use uix_app::lang::compiler::{CompilerSystem, component_source::*};
use uix_app::lang::runtime::{Effect, Type as DataType};

fn checked(source: &str) -> CheckedSource {
    let libraries = BTreeMap::from([(
        "native".into(),
        BTreeMap::from([
            (
                "Text".into(),
                NativeExport::Component(ComponentSignature {
                    parameters: vec![("children".into(), Type::Data(DataType::String))],
                    required: ["children".into()].into(),
                }),
            ),
            (
                "word".into(),
                NativeExport::Type(Type::Data(DataType::String)),
            ),
        ]),
    )]);
    CompilerSystem::new()
        .check_component_inline(source, "untitled:symbols", &libraries)
        .unwrap()
        .checked
}

#[test]
fn lexical_bindings_shadowing_captures_and_default_order_use_checked_identity() {
    let source = r#"export function future(): String { return "global"; }
export component Main(value: String = "外层😀") {
    state count: Int = 0;
    function change(step: Int = 1): Unit { count = count + step; }
    let previous: String = value;
    function choose(value: String = previous, future: String = value): String {
        if (true) { let value: String = "内层"; return value; }
        else { return (value); }
    }
    let mapped = [value].map((value) => value.length);
    return <> </>;
}"#;
    let checked = checked(source);
    let index = checked.symbol_index();
    let id = checked.source().source_graph.root();
    let target_at = |offset| index.at(id, offset).unwrap().fact.target.clone().unwrap();
    let component_input = target_at(source.find("value: String").unwrap());
    let function_input = target_at(source.find("choose(value").unwrap() + 7);
    let branch_local = target_at(source.find("let value:").unwrap() + 4);
    let lambda_input = target_at(source.find("((value)").unwrap() + 2);
    assert_ne!(component_input, function_input);
    assert_ne!(function_input, branch_local);
    assert_ne!(function_input, lambda_input);
    assert_eq!(
        target_at(source.find("= value;").unwrap() + 2),
        component_input
    );
    assert_eq!(
        target_at(source.find("= value):").unwrap() + 2),
        function_input
    );
    assert_eq!(
        target_at(source.find("return value;").unwrap() + 7),
        branch_local
    );
    let parenthesized = source.find("(value);").unwrap() + 1;
    assert_eq!(target_at(parenthesized), function_input);
    assert!(index.at(id, parenthesized - 1).is_none());
    assert!(index.at(id, parenthesized + 5).is_none());
    assert_eq!(
        target_at(source.find("=> value").unwrap() + 3),
        lambda_input
    );
    let method = index.at(id, source.find(".map").unwrap() + 1).unwrap();
    assert!(method.fact.target.is_none());
    assert!(index.hover(method).unwrap().contains("Function"));
    let change = index
        .symbols()
        .values()
        .find(|symbol| symbol.name == "change")
        .unwrap();
    assert!(index.describe(&change.target).unwrap().contains("Command"));
    let count = index
        .symbols()
        .values()
        .find(|symbol| symbol.name == "count")
        .unwrap();
    assert_eq!(index.references(&count.target, false).len(), 2);
    assert_eq!(index.references(&count.target, true).len(), 3);
    assert_eq!(
        checked
            .functions()
            .values()
            .filter(|sig| sig.effect == Effect::Command)
            .count(),
        1
    );
}

#[test]
fn tags_properties_type_aliases_and_native_names_have_precise_targets_not_text_matches() {
    let source = r#"import { Text as Label, word as Word } from 'native';
type Caption = Word;
component Card(title: Caption, children: View = <></>) {
    return <><Label>{title}</Label>{children}</>;
}
export component Main() {
    // Card title Label Word are not references here.
    let title: Caption = "Card title Label Word";
    return <Card title={title}><Label>Card title Label Word</Label></Card>;
}"#;
    let checked = checked(source);
    let index = checked.symbol_index();
    let id = checked.source().source_graph.root();
    let at = |offset| index.at(id, offset).unwrap();
    let card = at(source.find("component Card").unwrap() + 10)
        .fact
        .target
        .as_ref()
        .unwrap();
    assert_eq!(index.references(card, false).len(), 2); // opening and closing names, no raw text/comment/string.
    for offset in [
        source.find("// Card").unwrap() + 3,
        source.find("\"Card").unwrap() + 1,
        source.find(">Card title").unwrap() + 1,
    ] {
        assert!(index.at(id, offset).is_none());
    }
    let input = at(source.find("Card(title").unwrap() + 5)
        .fact
        .target
        .as_ref()
        .unwrap();
    let property = at(source.find("<Card title").unwrap() + 6);
    assert_eq!(property.fact.target.as_ref(), Some(input));
    assert_eq!(
        index.definition(input).unwrap().start,
        source.find("Card(title").unwrap() + 5
    );
    let alias = at(source.find("type Caption").unwrap() + 5)
        .fact
        .target
        .as_ref()
        .unwrap();
    assert_eq!(index.references(alias, false).len(), 2);
    let native = at(source.find("Text as").unwrap())
        .fact
        .target
        .as_ref()
        .unwrap();
    assert_eq!(
        at(source.find("as Label").unwrap() + 3)
            .fact
            .target
            .as_ref(),
        Some(native)
    );
    assert!(index.definition(native).is_none());
    assert_eq!(index.references(native, true).len(), 6); // two import names + two paired tags.
    assert!(index.describe(native).unwrap().contains("native"));
    assert!(
        index
            .at(id, source.find("Card(title").unwrap() + 4)
            .is_none()
    );
}

#[test]
fn bounded_checked_deep_sources_and_large_type_previews_remain_queryable() {
    let source = format!(
        "export function deep(value: Int): Int {{ return {}value{}; }}",
        "(".repeat(50),
        ")".repeat(50)
    );
    let checked = checked(&source);
    let index = checked.symbol_index();
    let binding = index
        .symbols()
        .values()
        .find(|symbol| symbol.name == "value")
        .unwrap();
    assert_eq!(index.references(&binding.target, false).len(), 1);
    let fields = (0..1000)
        .map(|n| format!("field_{n}: String"))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!("export type Huge = {{{fields}}};");
    let checked = self::checked(&source);
    let index = checked.symbol_index();
    let alias = index
        .symbols()
        .values()
        .find(|symbol| symbol.name == "Huge")
        .unwrap();
    let preview = index.describe(&alias.target).unwrap();
    assert!(preview.ends_with('…'));
    assert!(preview.len() <= 8195);
    let source = format!("export type {} = String;", "Long".repeat(3000));
    let checked = self::checked(&source);
    let index = checked.symbol_index();
    let preview = index
        .describe(&index.symbols().values().next().unwrap().target)
        .unwrap();
    assert!(preview.ends_with('…'));
    assert!(preview.len() <= 8195);
}

#[test]
fn native_properties_and_call_site_effects_do_not_invent_nominal_definitions() {
    let source = r#"import {Text} from 'native';
type Int = String;
export function identity(value: Int): Int { return value; }
export component Main() { let read = () => 7; let value = read(); return <Text children="中文😀"/>; }"#;
    let checked = checked(source);
    let index = checked.symbol_index();
    let source_id = checked.source().source_graph.root();
    let alias = index
        .symbols()
        .values()
        .find(|symbol| symbol.name == "Int")
        .unwrap();
    // 内建 Int 在类型位置优先；不能因为顶层恰好有同名声明就产生错误跳转。
    assert_eq!(index.references(&alias.target, false).len(), 0);
    let property = index
        .at(source_id, source.find("children=").unwrap())
        .unwrap();
    assert!(
        index
            .definition(property.fact.target.as_ref().unwrap())
            .is_none()
    );
    let hover = index.hover(property).unwrap();
    assert!(hover.contains("String") && hover.contains("required"));
    let call = index
        .at(source_id, source.find("read();").unwrap())
        .unwrap();
    let hover = index.hover(call).unwrap();
    assert!(
        hover.contains("Pure") && !hover.contains("Command"),
        "{hover}"
    );
}
