//! 共同源闭包的受检语义；不以检查成功冒称 AOT/动态执行已接通。
#![cfg(feature = "lang-build")]
use std::{collections::BTreeMap, fs, path::PathBuf};
use uix_app::lang::{
    compiler::{CompilerDiagnostic, DiagnosticPhase, component_source::*},
    runtime::{Effect, Type as DataType, Value},
};

fn data(ty: DataType) -> Type {
    Type::Data(ty)
}
fn function(parameters: Vec<Type>, returns: Type, effect: Effect) -> Type {
    Type::Function(FunctionSignature {
        minimum_arguments: parameters.len(),
        parameters,
        returns: Box::new(returns),
        effect,
    })
}
fn component(parameters: Vec<(&str, Type)>, required: &[&str]) -> NativeExport {
    NativeExport::Component(ComponentSignature {
        parameters: parameters
            .into_iter()
            .map(|(name, ty)| (name.into(), ty))
            .collect(),
        required: required.iter().map(|name| (*name).into()).collect(),
    })
}
fn natives() -> NativeLibraries {
    BTreeMap::from([(
        "sample-native".into(),
        BTreeMap::from([
            (
                "Column".into(),
                component(vec![("children", Type::View)], &[]),
            ),
            (
                "Text".into(),
                component(vec![("children", data(DataType::String))], &["children"]),
            ),
            (
                "Button".into(),
                component(
                    vec![
                        ("children", data(DataType::String)),
                        (
                            "onClick",
                            function(vec![], data(DataType::Unit), Effect::Command),
                        ),
                    ],
                    &[],
                ),
            ),
            (
                "Input".into(),
                component(
                    vec![
                        ("value", data(DataType::String)),
                        (
                            "onChange",
                            function(
                                vec![data(DataType::String)],
                                data(DataType::Unit),
                                Effect::Command,
                            ),
                        ),
                    ],
                    &["value"],
                ),
            ),
        ]),
    )])
}
struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/component-semantic-projects")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, text).unwrap();
        path
    }
    fn check(&self, text: &str) -> Result<CheckedSource, CompilerDiagnostic> {
        let path = self.write("main.uix", text);
        check(link_file(&path)?, &natives())
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn the_same_counter_card_and_editable_list_have_typed_interfaces_effects_and_captures() {
    let project = Project::new();
    project.write("card.uix", include_str!("fixtures/components/card.uix"));
    let counter = project.write(
        "counter.uix",
        include_str!("fixtures/components/counter.uix"),
    );
    let main = project.write(
        "editable_list.uix",
        include_str!("fixtures/components/editable_list.uix"),
    );
    let checked = check(link_file(&main).unwrap(), &natives()).unwrap();
    assert_eq!(checked.components().len(), 4);
    let source = &checked.source().source_graph;
    let counter_source = source
        .files()
        .iter()
        .find(|file| file.path == counter.to_str().unwrap())
        .unwrap();
    let increment = checked
        .functions()
        .iter()
        .find(|(id, _)| {
            id.source == counter_source.id
                && counter_source.source[id.start..id.end].starts_with("function increment")
        })
        .unwrap();
    assert_eq!(increment.1.effect, Effect::Command);
    let captures = &checked.captures()[increment.0];
    assert_eq!(captures.len(), 1);
    let state = &checked.bindings()[captures.first().unwrap()];
    assert_eq!(
        (state.name.as_str(), state.kind, &state.ty),
        ("count", BindingKind::State, &data(DataType::Int))
    );
    let caption = checked
        .functions()
        .iter()
        .find(|(id, _)| {
            source.file(id.source).unwrap().source[id.start..id.end]
                .starts_with("export function caption")
        })
        .unwrap();
    assert_eq!(caption.1.effect, Effect::Pure);
    assert!(
        checked
            .expressions()
            .values()
            .any(|fact| fact.ty == Type::array(Type::View))
    );
    assert!(checked.expressions().values().any(|fact| matches!(fact.resolution, Some(ResolvedName::NativeComponent { ref package, ref name }) if package == "sample-native" && name == "Input")));
    assert!(
        checked
            .expressions()
            .values()
            .any(|fact| matches!(fact.resolution, Some(ResolvedName::Component(_))))
    );
}

#[test]
fn render_rejects_indirect_state_writes_through_forward_calls_and_branch_aliases() {
    let project = Project::new();
    for body in [
        "function label(): String { return mutate(); } function mutate(): String { count = count + 1; return count.toString(); } return <Text>{label()}</Text>;",
        "function harmless(): String { return count.toString(); } function mutate(): String { count = count + 1; return count.toString(); } let handler = harmless; if (flag) { handler = mutate; } return <Text>{handler()}</Text>;",
        "function mutate(): String { count = count + 1; return count.toString(); } let handler = flag ? mutate : (() => \"ok\"); return <Text>{handler()}</Text>;",
        "count = count + 1; return <Text>{count.toString()}</Text>;",
    ] {
        let text = format!(
            "import {{ Text }} from 'sample-native'; export component Main(flag: Bool) {{ state count: Int = 0; {body} }}"
        );
        let error = project.check(&text).unwrap_err();
        assert_eq!(
            (error.code, error.phase),
            ("component-effect", DiagnosticPhase::Semantic),
            "{error:?}"
        );
        assert!(error.start < error.end && error.end <= text.len());
    }
    project.check("import { Text } from 'sample-native'; export component Main() { state count: Int = 0; function label(): String { return count.toString(); } return <Text>{label()}</Text>; }").unwrap();
}

#[test]
fn readonly_inputs_and_captures_cannot_become_accidental_shared_mutable_state() {
    let project = Project::new();
    for (body, code) in [
        (
            "function set(): Unit { initial = 1; } return <Button onClick={set}/>;",
            "component-readonly",
        ),
        (
            "let local = initial; return <Button onClick={() => { local = 1; }}/>;",
            "component-readonly-capture",
        ),
        (
            "state callback: () => Unit = () => {}; return <Button/>;",
            "component-state-type",
        ),
        (
            "state count: Int = missing; return <Button/>;",
            "component-name",
        ),
    ] {
        let text = format!(
            "import {{ Button }} from 'sample-native'; export component Main(initial: Int) {{ {body} }}"
        );
        assert_eq!(project.check(&text).unwrap_err().code, code);
    }
    project.check("import { Button } from 'sample-native'; export component Main(initial: Int) { state count: Int = initial; return <Button onClick={() => { count = count + 1; }}/>; }").unwrap();
}

#[test]
fn component_inputs_callbacks_children_and_keys_are_checked_against_selected_exports() {
    let project = Project::new();
    project.check("import { Text } from 'sample-native'; export component Space() { return <Text> </Text>; }").unwrap();
    for (view, code) in [
        ("<Input value={true}/>", "component-type"),
        (
            "<Input value=\"ok\" onChange={(number: Int) => {}}/>",
            "component-type",
        ),
        ("<Input/>", "component-required-property"),
        ("<Input value=\"ok\" unknown={1}/>", "component-property"),
        ("<Input value=\"ok\" key={false}/>", "component-key"),
        ("<Text><Input value=\"ok\"/></Text>", "component-type"),
        ("<Column>implicit text</Column>", "component-view-text"),
        (
            "<Column children={<Text>x</Text>}><Text>y</Text></Column>",
            "component-children",
        ),
        ("<Button onClick={() => 1}/>", "component-type"),
    ] {
        let text = format!(
            "import {{ Input, Text, Column, Button }} from 'sample-native'; export component Main() {{ return {view}; }}"
        );
        assert_eq!(project.check(&text).unwrap_err().code, code, "{view}");
    }
}

#[test]
fn aliases_collections_and_numeric_boundaries_share_portable_data_types() {
    let project = Project::new();
    let checked = project.check("type Item = { id: Int, label: String }; export function titles(items: Item[]): String { return items.map((item) => item.label.trim()).join(\",\"); } export function min(): Int { return -9223372036854775808; }").unwrap();
    assert!(
        checked
            .expressions()
            .values()
            .any(|fact| fact.constant == Some(Value::Int(i64::MIN)))
    );
    for (text, code) in [
        (
            "export function positive(): Int { return 9223372036854775808; }",
            "component-integer",
        ),
        (
            "export function wrong(): Int { return 1 + true; }",
            "component-binary",
        ),
        (
            "export function wrong(): Int { return 1.0; }",
            "component-type",
        ),
        (
            "export function wrong(items: Int[]): Int[] { return items.map((item) => item.toString()); }",
            "component-type",
        ),
        ("type A = B; type B = A;", "component-type-cycle"),
        (
            "export function missing(flag: Bool): String { if (flag) { return \"yes\"; } }",
            "component-return",
        ),
    ] {
        assert_eq!(project.check(text).unwrap_err().code, code);
    }
    let mut source = "type T0 = Int;\n".to_string();
    for index in 1..20 {
        source += &format!(
            "type T{index} = {{ left: T{}, right: T{} }};\n",
            index - 1,
            index - 1
        );
    }
    assert!(matches!(
        project.check(&source).unwrap_err().code,
        "component-type-limit" | "component-analysis-limit"
    ));
}

#[test]
fn dependency_errors_keep_the_true_file_and_original_chinese_range() {
    let project = Project::new();
    let dependency = "import { Text } from 'sample-native'; export component Card(标题: String) { return <Text>{未知标题}</Text>; }";
    project.write("card.uix", dependency);
    let error = project.check("import { Card } from './card.uix'; export component Main() { return <Card 标题=\"中文\"/>; }").unwrap_err();
    assert!(error.source_name.ends_with("card.uix"));
    assert_eq!(&dependency[error.start..error.end], "未知标题");
    assert_eq!(error.code, "component-name");
}

#[test]
fn native_functions_have_explicit_effects_and_missing_exports_never_get_placeholders() {
    let project = Project::new();
    assert_eq!(
        project
            .check("import { Unknown } from 'sample-native';")
            .unwrap_err()
            .code,
        "component-native-export"
    );
    let mut libraries = natives();
    libraries.get_mut("sample-native").unwrap().insert(
        "request".into(),
        NativeExport::Function(FunctionSignature {
            minimum_arguments: 0,
            parameters: vec![],
            returns: Box::new(data(DataType::String)),
            effect: Effect::Command,
        }),
    );
    let path = project.write("main.uix", "import { Text, request } from 'sample-native'; export component Main() { return <Text>{request()}</Text>; }");
    assert_eq!(
        check(link_file(&path).unwrap(), &libraries)
            .unwrap_err()
            .code,
        "component-effect"
    );
}

#[test]
fn defaults_bind_left_to_right_and_function_arity_matches_optional_trailing_arguments() {
    let project = Project::new();
    project.check("import { Text } from 'sample-native'; function label(value: Int = 1, second: Int = value + 1): String { return second.toString(); } export component Main(initial: Int = 0, next: Int = initial + 1) { return <Text>{label()}</Text>; }").unwrap();
    for text in [
        "function second(): Int { return 1; } export function f(first: Int = second(), second: () => Int): Int { return first; }",
        "export component Main(first: Int = second, second: Int = 1) { return <> </>; }",
        "export function f(value: Int = value): Int { return value; }",
    ] {
        assert_eq!(
            project.check(text).unwrap_err().code,
            "component-parameter-order"
        );
    }
    project.check("export function f(callback: (second: Int) => Int = (second) => second, second: Int = 1): Int { return callback(second); }").unwrap();
    for text in [
        "function f(value: Int, optional: Int = 1): Int { return value + optional; } export function bad(): Int { return f(); }",
        "function f(value: Int = 1): Int { return value; } export function bad(): Int { return f(1, 2); }",
    ] {
        assert_eq!(
            project.check(text).unwrap_err().code,
            "component-call-arity"
        );
    }
}

#[test]
fn selected_native_interfaces_are_validated_normalized_and_frozen() {
    let project = Project::new();
    let path = project.write("main.uix", "import { identity } from 'third-party'; export function test(): Int[] { return identity([1, 2]); }");
    let raw_array = Type::Array(Box::new(data(DataType::Int)));
    let mut libraries = BTreeMap::from([(
        "third-party".into(),
        BTreeMap::from([(
            "identity".into(),
            NativeExport::Function(FunctionSignature {
                minimum_arguments: 1,
                parameters: vec![raw_array.clone()],
                returns: Box::new(raw_array),
                effect: Effect::Pure,
            }),
        )]),
    )]);
    let checked = check(link_file(&path).unwrap(), &libraries).unwrap();
    libraries.clear();
    let NativeExport::Function(signature) = &checked.native_imports()["third-party"]["identity"]
    else {
        panic!();
    };
    assert_eq!(
        *signature.returns,
        data(DataType::Array(Box::new(DataType::Int)))
    );
    let entry = project.write("main.uix", "import { Bad } from 'bad';");
    for export in [
        component(
            vec![
                ("value", data(DataType::Int)),
                ("value", data(DataType::Int)),
            ],
            &[],
        ),
        component(vec![("key", data(DataType::Int))], &[]),
        component(vec![], &["missing"]),
        NativeExport::Function(FunctionSignature {
            minimum_arguments: 1,
            parameters: vec![],
            returns: Box::new(data(DataType::Unit)),
            effect: Effect::Pure,
        }),
    ] {
        let libraries = BTreeMap::from([("bad".into(), BTreeMap::from([("Bad".into(), export)]))]);
        assert_eq!(
            check(link_file(&entry).unwrap(), &libraries)
                .unwrap_err()
                .code,
            "component-native-signature"
        );
    }
}
