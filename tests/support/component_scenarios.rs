// 实验组件执行，不创建窗口；原生组件绑定仅为明确的投影合同。
use std::{collections::BTreeMap, fs, path::PathBuf};
use uix_app::lang::runtime::{self, Cancellation, Effect, ErrorKind, components::*};
fn data(value: runtime::Value) -> Value {
    Value::data(value)
}
fn int(value: i64) -> Value {
    data(runtime::Value::Int(value))
}
fn string(value: &str) -> Value {
    data(runtime::Value::String(value.into()))
}
fn function(parameters: Vec<Type>, returns: Type, effect: Effect) -> FunctionSignature {
    FunctionSignature {
        minimum_arguments: parameters.len(),
        parameters,
        returns: Box::new(returns),
        effect,
    }
}
fn natives() -> NativeBindings {
    let data = |ty| Type::Data(ty);
    let unit = data(runtime::Type::Unit);
    let string = data(runtime::Type::String);
    [
        ("Column", vec![("children", Type::View)], vec![]),
        ("Text", vec![("children", string.clone())], vec!["children"]),
        (
            "Button",
            vec![
                ("children", string.clone()),
                (
                    "onClick",
                    Type::Function(function(vec![], unit.clone(), Effect::Command)),
                ),
            ],
            vec![],
        ),
        (
            "Input",
            vec![
                ("value", string.clone()),
                (
                    "onChange",
                    Type::Function(function(vec![string], unit, Effect::Command)),
                ),
            ],
            vec!["value"],
        ),
    ]
    .into_iter()
    .map(|(name, parameters, required)| {
        (
            ExportKey::new("sample-native", name),
            NativeBinding::Component(ComponentSignature {
                parameters: parameters
                    .into_iter()
                    .map(|(name, ty)| (name.into(), ty))
                    .collect(),
                required: required.into_iter().map(str::to_string).collect(),
            }),
        )
    })
    .collect()
}
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/components")
        .join(name)
}
fn engine(name: &str, entry: &str, inputs: BTreeMap<String, Value>) -> Engine {
    let natives = natives();
    Engine::new(
        compile(&fixture(name), entry, &natives),
        natives,
        inputs,
        ComponentLimits::default(),
    )
    .unwrap()
}
fn inline(text: &str, entry: &str, natives: NativeBindings) -> Engine {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/component-runtime-projects")
        .join(format!(
            "{}-{}.uix",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, text).unwrap();
    let program = compile(&path, entry, &natives);
    Engine::new(
        program,
        natives,
        BTreeMap::new(),
        ComponentLimits::default(),
    )
    .unwrap()
}
fn nodes(engine: &Engine) -> Vec<&NativeNode> {
    fn values<'a>(value: &'a ProjectedValue, result: &mut Vec<&'a NativeNode>) {
        match value {
            ProjectedValue::View(nodes) => {
                for node in nodes {
                    walk(node, result)
                }
            }
            ProjectedValue::Array(items) => {
                for item in items {
                    values(item, result)
                }
            }
            ProjectedValue::Record(fields) => {
                for value in fields.values() {
                    values(value, result)
                }
            }
            _ => {}
        }
    }
    fn walk<'a>(node: &'a NativeNode, result: &mut Vec<&'a NativeNode>) {
        result.push(node);
        for value in node.properties.values() {
            values(value, result)
        }
    }
    let mut result = Vec::new();
    for node in &engine.snapshot().roots {
        walk(node, &mut result)
    }
    result
}
fn texts(engine: &Engine) -> Vec<String> {
    nodes(engine)
        .into_iter()
        .filter(|node| node.export.name == "Text")
        .map(|node| match &node.properties["children"] {
            ProjectedValue::Data(value) => value.as_str().unwrap().into(),
            _ => panic!(),
        })
        .collect()
}
fn events(engine: &Engine, name: &str, property: &str) -> Vec<EventToken> {
    nodes(engine)
        .into_iter()
        .filter(|node| node.export.name == name)
        .map(|node| match node.properties[property] {
            ProjectedValue::Event(token) => token,
            _ => panic!(),
        })
        .collect()
}
fn click(engine: &mut Engine, index: usize) {
    let token = events(engine, "Button", "onClick")[index];
    engine
        .dispatch(token, vec![], &Cancellation::default())
        .unwrap();
}

fn counter_state_input_defaults_cache_and_close() {
    let mut engine = engine("counter.uix", "Counter", BTreeMap::new());
    assert_eq!(texts(&engine), ["0"]);
    let old = events(&engine, "Button", "onClick")[0];
    let result = engine
        .dispatch(old, vec![], &Cancellation::default())
        .unwrap();
    assert_eq!(result.stats.rendered_instances, 1);
    assert_eq!(texts(&engine), ["1"]);
    assert_eq!(
        engine
            .dispatch(old, vec![], &Cancellation::default())
            .unwrap_err()
            .kind,
        ErrorKind::Conflict
    );
    engine
        .update(
            BTreeMap::from([("initial".into(), int(99))]),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(texts(&engine), ["1"]);
    let stats = engine
        .update(
            BTreeMap::from([("initial".into(), int(99))]),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(stats.rendered_instances, 0);
    assert_eq!(stats.reused_instances, 1);
    let token = events(&engine, "Button", "onClick")[0];
    engine.close();
    engine.close();
    assert_eq!(engine.snapshot().instances, 0);
    assert!(engine.snapshot().roots.is_empty());
    assert_eq!(
        engine
            .dispatch(token, vec![], &Cancellation::default())
            .unwrap_err()
            .kind,
        ErrorKind::Closed
    );
}
fn editable_list_imports_owner_callbacks_and_local_rerender() {
    let item = |id, title| {
        Value::record(BTreeMap::from([
            ("id".into(), int(id)),
            ("title".into(), string(title)),
        ]))
    };
    let mut engine = engine(
        "editable_list.uix",
        "EditableList",
        BTreeMap::from([(
            "initial".into(),
            Value::array(vec![item(1, "中文一"), item(2, "中文二")]),
        )]),
    );
    assert_eq!(texts(&engine), ["中文一", "1", "中文二", "2"]);
    assert_eq!(engine.snapshot().instances, 7);
    let token = events(&engine, "Input", "onChange")[0];
    let result = engine
        .dispatch(token, vec![string("改过的标题")], &Cancellation::default())
        .unwrap();
    assert_eq!(result.stats.rendered_instances, 2);
    assert_eq!(texts(&engine), ["中文一", "1", "中文二", "2"]);
    click(&mut engine, 0);
    assert_eq!(texts(&engine), ["改过的标题", "1", "中文二", "2"]);
    click(&mut engine, 1);
    assert_eq!(texts(&engine), ["改过的标题", "2", "中文二", "2"]);
}
fn keys_preserve_state_across_reorder_and_unmount_retires_events() {
    let path = PathBuf::from("../../tests/fixtures/components/counter.uix");
    let mut engine = inline(
        &format!(
            r#"import {{Column,Button}} from "sample-native";import {{Counter}} from {:?};export component Main() {{state ids:Int[]=[1,2];return <Column><Button onClick={{()=>{{ids=[2,1];}}}}>换序</Button><Button onClick={{()=>{{ids=[2];}}}}>移除</Button>{{ids.map((id)=><Counter key={{id}} initial={{id}}/>)}}</Column>;}}"#,
            path.to_string_lossy()
        ),
        "Main",
        natives(),
    );
    assert_eq!(texts(&engine), ["1", "2"]);
    click(&mut engine, 2);
    assert_eq!(texts(&engine), ["2", "2"]);
    let old = events(&engine, "Button", "onClick")[2];
    click(&mut engine, 0);
    assert_eq!(texts(&engine), ["2", "2"]);
    click(&mut engine, 2);
    assert_eq!(texts(&engine), ["3", "2"]);
    let remove = events(&engine, "Button", "onClick")[1];
    let result = engine
        .dispatch(remove, vec![], &Cancellation::default())
        .unwrap();
    assert_eq!(result.stats.unmounted_instances, 1);
    assert_eq!(texts(&engine), ["3"]);
    assert_eq!(
        engine
            .dispatch(old, vec![], &Cancellation::default())
            .unwrap_err()
            .kind,
        ErrorKind::Conflict
    );
}
fn failed_event_rolls_back_state_and_keeps_current_token() {
    let mut engine = inline(
        r#"import {Column,Text,Button} from "sample-native"; export component Main(){state value:Int=0;return <Column><Text>{value.toString()}</Text><Button onClick={()=>{value=value+1;let bad:Int=1/0;}}>失败</Button></Column>;}"#,
        "Main",
        natives(),
    );
    let revision = engine.snapshot().revision;
    let token = events(&engine, "Button", "onClick")[0];
    let error = engine
        .dispatch(token, vec![], &Cancellation::default())
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Arithmetic);
    assert!(error.location.source.ends_with(".uix"));
    assert!(error.location.column > 1);
    assert_eq!(engine.snapshot().revision, revision);
    assert_eq!(texts(&engine), ["0"]);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert_eq!(
        engine
            .dispatch(token, vec![], &cancellation)
            .unwrap_err()
            .kind,
        ErrorKind::Cancelled
    );
    assert_eq!(texts(&engine), ["0"]);
    assert_eq!(
        engine
            .dispatch(token, vec![], &Cancellation::default())
            .unwrap_err()
            .kind,
        ErrorKind::Arithmetic
    );
}
fn nested_closures_freeze_locals_and_lift_transitive_captures() {
    let mut engine = inline(
        r#"import {Column,Text,Button} from "sample-native";export component Main(){state n:Int=0;let a:Int=4;function inner(x:Int=2):Int{return a+x;}function outer():Int{return inner();}let callback:()=>Unit=()=>{n=outer();};a=99;return <Column><Text>{n.toString()}</Text><Button onClick={callback}>设置</Button></Column>;}"#,
        "Main",
        natives(),
    );
    click(&mut engine, 0);
    assert_eq!(texts(&engine), ["6"]);
}
fn empty_views_key_failure_and_remount() {
    let mut engine = inline(
        r#"import {Column,Text,Button} from "sample-native";
component Row(id:Int){state n:Int=id;return <Button onClick={()=>{n=n+1;}}>{n.toString()}</Button>;}
export component Main(){state ids:Int[]=[1,2];let empty:View[]=[];return <Column>{empty}<Button onClick={()=>{ids=[];}}>清空</Button><Button onClick={()=>{ids=[1,1];}}>重复</Button><Button onClick={()=>{ids=[1,2];}}>恢复</Button>{ids.map((id)=><Row key={id} id={id}/>)}</Column>;}
"#,
        "Main",
        natives(),
    );
    let token = events(&engine, "Button", "onClick")[1];
    let revision = engine.snapshot().revision;
    assert_eq!(
        engine
            .dispatch(token, vec![], &Cancellation::default())
            .unwrap_err()
            .kind,
        ErrorKind::InvalidComponent
    );
    assert_eq!(engine.snapshot().revision, revision);
    assert_eq!(engine.snapshot().instances, 3);
    click(&mut engine, 3);
    click(&mut engine, 0);
    assert_eq!(engine.snapshot().instances, 1);
    click(&mut engine, 2);
    assert_eq!(engine.snapshot().instances, 3);
    let buttons = nodes(&engine)
        .into_iter()
        .filter(|node| node.export.name == "Button")
        .map(|node| match &node.properties["children"] {
            ProjectedValue::Data(value) => value.as_str().unwrap().to_string(),
            _ => panic!(),
        })
        .collect::<Vec<_>>();
    assert_eq!(&buttons[3..], ["1", "2"]);
}
fn native_error_panic_limits_and_release() {
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let retained = std::sync::Arc::new(());
    let weak = std::sync::Arc::downgrade(&retained);
    let mut bindings = natives();
    let count = calls.clone();
    bindings.insert(
        ExportKey::new("service", "write"),
        NativeBinding::Function {
            signature: function(vec![], Type::Data(runtime::Type::Unit), Effect::Command),
            call: std::sync::Arc::new(move |_| {
                let _retained = &retained;
                count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                panic!("controlled host failure")
            }),
        },
    );
    let mut engine = inline(
        r#"import {Column,Text,Button} from "sample-native";import {write} from "service";export component Main(){state n:Int=0;return <Column><Text>{n.toString()}</Text><Button onClick={()=>{n=8;write();}}>失败</Button></Column>;}"#,
        "Main",
        bindings,
    );
    let token = events(&engine, "Button", "onClick")[0];
    let error = engine
        .dispatch(token, vec![], &Cancellation::default())
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::HostFailure);
    assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 1);
    assert_eq!(texts(&engine), ["0"]);
    engine.close();
    assert!(weak.upgrade().is_none());
    let natives = natives();
    let program = compile(&fixture("counter.uix"), "Counter", &natives);
    let mut limits = ComponentLimits::default();
    limits.evaluation.steps = 1;
    assert_eq!(
        Engine::new(program.clone(), natives.clone(), BTreeMap::new(), limits)
            .err()
            .unwrap()
            .kind,
        ErrorKind::Quota
    );
    let mut missing = natives;
    missing.remove(&ExportKey::new("sample-native", "Text"));
    assert_eq!(
        Engine::new(
            program,
            missing,
            BTreeMap::new(),
            ComponentLimits::default()
        )
        .err()
        .unwrap()
        .kind,
        ErrorKind::CapabilityDenied
    );
}

fn named_slots_mount_shared_blueprints_as_distinct_instances() {
    let mut bindings = natives();
    bindings.insert(
        ExportKey::new("sample-native", "Pane"),
        NativeBinding::Component(ComponentSignature {
            parameters: vec![("first".into(), Type::View), ("second".into(), Type::View)],
            required: ["first".into(), "second".into()].into(),
        }),
    );
    let mut engine = inline(
        r#"import {Pane} from "sample-native";import {Counter} from "../../tests/fixtures/components/counter.uix";export component Main(){let content:View=<Counter/>;return <Pane first={content} second={content}/>;}"#,
        "Main",
        bindings,
    );
    assert_eq!(texts(&engine), ["0", "0"]);
    assert_eq!(engine.snapshot().instances, 3);
    click(&mut engine, 0);
    assert_eq!(texts(&engine), ["1", "0"]);
    click(&mut engine, 1);
    assert_eq!(texts(&engine), ["1", "1"]);
}
fn budget_matrix() -> String {
    let natives = natives();
    let program = compile(&fixture("counter.uix"), "Counter", &natives);
    let mut results = Vec::new();
    for (steps, depth) in (1..=120)
        .map(|steps| (steps, 64))
        .chain((1..=20).map(|depth| (10000, depth)))
    {
        let mut limits = ComponentLimits::default();
        limits.evaluation.steps = steps;
        limits.evaluation.depth = depth;
        let result = match Engine::new(program.clone(), natives.clone(), BTreeMap::new(), limits) {
            Err(error) => format!(
                "mount:{:?}:{}:{}",
                error.kind, error.location.line, error.location.column
            ),
            Ok(mut engine) => {
                let token = events(&engine, "Button", "onClick")[0];
                let result = engine.dispatch(token, vec![], &Cancellation::default());
                format!(
                    "event:{:?}:{}:{:?}",
                    result
                        .map(|result| (
                            result.stats.rendered_instances,
                            result.stats.reused_instances
                        ))
                        .map_err(|error| (error.kind, error.location.line, error.location.column)),
                    engine.snapshot().revision,
                    texts(&engine)
                )
            }
        };
        results.push(format!("{steps}/{depth}={result}"));
    }
    results.join("\n")
}
pub fn run() -> String {
    counter_state_input_defaults_cache_and_close();
    editable_list_imports_owner_callbacks_and_local_rerender();
    keys_preserve_state_across_reorder_and_unmount_retires_events();
    failed_event_rolls_back_state_and_keeps_current_token();
    nested_closures_freeze_locals_and_lift_transitive_captures();
    empty_views_key_failure_and_remount();
    native_error_panic_limits_and_release();
    named_slots_mount_shared_blueprints_as_distinct_instances();
    budget_matrix()
}
