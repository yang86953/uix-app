//! 通过公开装载、AOT、实例与编译工具合同消费多文件模块，不访问私有实现。
#![cfg(feature = "uix-dynamic")]

use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use uix::app::modules::*;
use uix_lang_compiler::modules;

const ROOT: &str = include_str!("../examples/modules/composed/main.uix");
const COUNTER: &str = include_str!("../examples/modules/composed/counter.uix");
const TEXT: &str = include_str!("../examples/modules/composed/text.uix");

struct Package(PathBuf);
impl Package {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "uix-module-package-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        let package = Self(path);
        package.write("main.uix", ROOT);
        package.write("counter.uix", COUNTER);
        package.write("text.uix", TEXT);
        package
    }
    fn write(&self, name: &str, source: &str) {
        std::fs::write(self.0.join(name), source).unwrap();
    }
    fn root(&self) -> PathBuf {
        self.0.join("main.uix")
    }
}
impl Drop for Package {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn instance(module: Module, log: Arc<Mutex<Vec<String>>>) -> Instance {
    let mut sync = HostPorts::new();
    let mut asynchronous = AsyncHostPorts::new();
    for signature in &module.ports {
        let name = signature.name.clone();
        if signature.asynchronous {
            asynchronous.insert(
                name.clone(),
                AsyncHostPort {
                    signature: signature.clone(),
                    start: Arc::new(move |call| {
                        call.completion.complete(Ok(Value::String(name.clone())))
                    }),
                },
            );
        } else {
            let log = log.clone();
            sync.insert(
                name.clone(),
                HostPort {
                    signature: signature.clone(),
                    callback: Arc::new(move |arguments| {
                        log.lock()
                            .unwrap()
                            .push(format!("{name}:{}", arguments[0].as_str()?));
                        Ok(Value::Unit)
                    }),
                },
            );
        }
    }
    Instance::new_with_async_ports(module, sync, asynchronous, Limits::default()).unwrap()
}

#[test]
fn aot_and_dynamic_link_transitive_private_calls_types_and_isolated_state() {
    let package = Package::new();
    let native = uix::uix_module!("examples/modules/composed/main.uix");
    let dynamic = load_module_file(&package.root()).unwrap();
    let mut observations = vec![];
    for module in [native, dynamic] {
        assert_eq!(module.dependencies.len(), 4);
        let log = Arc::new(Mutex::new(vec![]));
        let mut app = instance(module, log.clone());
        let value = app
            .call("run", &[Value::String(" hello  世界 ".into())])
            .unwrap();
        assert_eq!(
            value,
            Value::Record(BTreeMap::from([
                ("text".into(), Value::String("hello 世界".into())),
                ("count".into(), Value::Int(2))
            ]))
        );
        assert_eq!(app.state()["left.count"], Value::Int(1));
        assert_eq!(app.state()["right.count"], Value::Int(0));
        app.call("other", &[Value::String("other".into())]).unwrap();
        assert_eq!(app.call("total", &[]).unwrap(), Value::Int(2));
        assert_eq!(
            app.call("left.current", &[]).unwrap_err().kind,
            ErrorKind::UnknownCommand
        );
        let before = (app.state(), app.revision(), app.view().unwrap());
        let error = app.call("rollback", &[]).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Arithmetic);
        assert!(error.location.source.ends_with("counter.uix"));
        assert_eq!((app.state(), app.revision(), app.view().unwrap()), before);
        app.call("record", &[]).unwrap();
        observations.push((app.state(), app.revision(), log.lock().unwrap().clone()));
    }
    assert_eq!(observations[0], observations[1]);
    assert_eq!(observations[0].2, ["left.audit:1"]);
}

#[test]
fn imported_async_event_uses_qualified_host_port_and_child_state_in_both_modes() {
    let package = Package::new();
    for module in [
        uix::uix_module!("examples/modules/composed/main.uix"),
        load_module_file(&package.root()).unwrap(),
    ] {
        let owner =
            ModuleWorker::spawn(instance(module, Arc::new(Mutex::new(vec![]))), 2, || {}).unwrap();
        let handle = owner.handle();
        let generation = handle.snapshot().generation;
        assert_eq!(
            handle
                .event("fetch", generation, vec![])
                .unwrap()
                .wait(Duration::from_secs(3))
                .unwrap(),
            Value::String("left.read".into())
        );
        assert_eq!(
            handle.snapshot().state["left.saved"],
            Value::String("left.read".into())
        );
        assert_eq!(
            handle.snapshot().state["right.saved"],
            Value::String(String::new())
        );
        owner.close(Duration::from_secs(3)).unwrap();
    }
}

#[test]
fn frozen_source_graph_symbols_and_overlays_keep_dependency_provenance() {
    let package = Package::new();
    let old = modules::check_file(&package.root()).unwrap();
    assert_eq!(old.source_graph.files().len(), 3);
    assert_eq!(old.source_graph.imports().len(), 3);
    let symbol = old.symbols.iter().find(|s| s.name == "left.step").unwrap();
    let source = old.source_graph.file(symbol.source_id).unwrap();
    assert!(source.path.ends_with("counter.uix"));
    assert!(source.source[symbol.start..symbol.end].contains("name=\"step\""));
    assert!(!old.symbols.iter().any(|s| s.name == "left.text.analyze"));
    let changed = TEXT.replace("input.words().join(' ')", "input.toUpperCase()");
    let overlays = BTreeMap::from([(package.0.join("text.uix"), changed)]);
    let new = modules::check_file_with_overlays(&package.root(), &overlays).unwrap();
    assert_ne!(
        old.source_graph.dependency_hash(),
        new.source_graph.dependency_hash()
    );
    assert!(old.source_graph.files().iter().any(|f| f.source == TEXT));
    assert_eq!(
        std::fs::read_to_string(package.0.join("text.uix")).unwrap(),
        TEXT
    );
    let generated = modules::compile_file(&package.root()).unwrap().to_string();
    assert!(generated.contains("Native"));
    for name in ["main.uix", "counter.uix", "text.uix"] {
        assert!(generated.contains(name));
    }
    package.write(
        "text.uix",
        &TEXT.replace("normalize(input)", "missing(input)"),
    );
    let error = modules::check_file(&package.root()).unwrap_err();
    assert!(error.source_name.ends_with("text.uix"));
    assert!(error.line > 1 && error.end > error.start);
    let mut frozen = instance(old.module, Arc::new(Mutex::new(vec![])));
    assert!(
        frozen
            .call("run", &[Value::String("still frozen".into())])
            .is_ok()
    );
}

#[test]
fn private_names_versions_types_effects_and_cycles_are_rejected_before_replacement() {
    let package = Package::new();
    let mut app = instance(
        load_module_file(&package.root()).unwrap(),
        Arc::new(Mutex::new(vec![])),
    );
    app.call("run", &[Value::String("retained".into())])
        .unwrap();
    let before = (app.generation(), app.state(), app.revision());
    for candidate in [
        ROOT.replace("left.step(input)", "left.text.normalize(input)"),
        ROOT.replace("left.step(input)", "left.step(42)"),
        ROOT.replace("left.Stats", "left.Private"),
        ROOT.replace("as=\"right\"", "as=\"left\""),
        ROOT.replace("as=\"left\" version=\"1\"", "as=\"left\" version=\"99\""),
        ROOT.replace("<Query name=\"total\"", "<Function name=\"total\""),
        ROOT.replace("./counter.uix", "https://example.invalid/counter.uix"),
        ROOT.replace("./counter.uix", "/tmp/counter.uix"),
    ] {
        package.write("main.uix", &candidate);
        let dynamic = modules::check_file(&package.root()).unwrap_err();
        let native = modules::compile_file(&package.root()).unwrap_err();
        assert_eq!(dynamic, native);
        assert_eq!((app.generation(), app.state(), app.revision()), before);
    }
    package.write("main.uix", ROOT);
    package.write("counter.uix", &COUNTER.replace("./text.uix", "./main.uix"));
    assert!(
        modules::check_file(&package.root())
            .unwrap_err()
            .message
            .contains("循环")
    );
    assert!(
        load_module(ROOT, "main.uix")
            .unwrap_err()
            .message
            .contains("内嵌")
    );
}

#[test]
fn dependency_identity_schema_and_declared_types_guard_atomic_migration() {
    let package = Package::new();
    let mut app = instance(
        load_module_file(&package.root()).unwrap(),
        Arc::new(Mutex::new(vec![])),
    );
    app.call("run", &[Value::String("hello".into())]).unwrap();
    let before = (app.generation(), app.state(), app.revision());
    for counter in [
        COUNTER.replace("schema=\"1\"", "schema=\"2\""),
        COUNTER.replace("CounterLibrary", "DifferentLibrary"),
    ] {
        package.write("counter.uix", &counter);
        assert_eq!(
            app.replace(load_module_file(&package.root()).unwrap(), before.0)
                .unwrap_err()
                .kind,
            ErrorKind::Conflict
        );
        assert_eq!((app.generation(), app.state(), app.revision()), before);
    }
    package.write(
        "counter.uix",
        &COUNTER.replace("version=\"1\" schema", "version=\"2\" schema"),
    );
    package.write(
        "main.uix",
        &ROOT.replace("version=\"1\" />", "version=\"2\" />"),
    );
    app.replace(load_module_file(&package.root()).unwrap(), before.0)
        .unwrap();
    assert_eq!(app.generation(), before.0 + 1);
    assert_eq!(app.state(), before.1);
    let arrays = r#"<Module name="Types" version="1" schema="1"><State name="items" type="Array&lt;Int&gt;" value={[]} /></Module>"#.replace("&lt;", "<").replace("&gt;", ">");
    let mut arrays_app = Instance::new(
        load_module(&arrays, "array.uix").unwrap(),
        HostPorts::new(),
        Limits::default(),
    )
    .unwrap();
    assert_eq!(
        arrays_app
            .replace(
                load_module(&arrays.replace("Int", "String"), "array.uix").unwrap(),
                1
            )
            .unwrap_err()
            .kind,
        ErrorKind::Type
    );
}

#[test]
fn explicit_bindings_and_source_and_type_budgets_are_enforced() {
    let package = Package::new();
    assert_eq!(
        Instance::new(
            load_module_file(&package.root()).unwrap(),
            HostPorts::new(),
            Limits::default()
        )
        .err()
        .unwrap()
        .kind,
        ErrorKind::CapabilityDenied
    );
    let mut source = "<Module name=\"Bomb\" version=\"1\" schema=\"1\"><Data name=\"T0\" fields=\"value: Int\" />".to_string();
    for index in 1..30 {
        source.push_str(&format!(
            "<Data name=\"T{index}\" fields=\"a: T{}, b: T{}\" />",
            index - 1,
            index - 1
        ));
    }
    source.push_str("</Module>");
    assert!(
        load_module(&source, "type-budget.uix")
            .unwrap_err()
            .message
            .contains("4096")
    );
    let mut main = "<Module name=\"Many\" version=\"1\" schema=\"1\">".to_string();
    for index in 0..63 {
        main.push_str(&format!(
            "<Import from=\"./counter.uix\" as=\"c{index}\" version=\"1\" />"
        ));
    }
    main.push_str("</Module>");
    package.write("main.uix", &main);
    assert!(
        modules::check_file(&package.root())
            .unwrap_err()
            .message
            .contains("64")
    );
}

#[cfg(unix)]
#[test]
fn dependency_symlink_cannot_escape_the_explicit_package() {
    let package = Package::new();
    let outside = Package::new();
    std::os::unix::fs::symlink(outside.0.join("counter.uix"), package.0.join("escape.uix"))
        .unwrap();
    package.write("main.uix", &ROOT.replace("./counter.uix", "./escape.uix"));
    assert!(
        modules::check_file(&package.root())
            .unwrap_err()
            .message
            .contains("包目录")
    );
}

#[test]
fn formatter_query_and_auto_tool_entry_share_module_facts_without_changing_ui_entry() {
    use uix_lang_compiler::{CompilerSystem, DocumentOutput, QueryEntry, QueryKind};
    let system = CompilerSystem::new();
    let package = Package::new();
    assert!(matches!(
        system.check_document_file(&package.root()).unwrap(),
        DocumentOutput::Module(_)
    ));
    assert!(matches!(
        system
            .check_document_inline("<App><Text>兼容</Text></App>", "ui.uix")
            .unwrap(),
        DocumentOutput::Ui(_)
    ));
    let compact = TEXT.lines().map(str::trim).collect::<String>();
    let source = format!("// 假标签 <App> 不改变入口\n/* 说明 */\n{compact}");
    assert!(modules::recognizes_source(&source));
    assert!(!modules::recognizes_source("// <Module>\n<App />"));
    let formatted = system.format_inline(&source, "formatted.uix").unwrap();
    assert!(formatted.changed);
    assert_eq!(
        system
            .format_inline(&formatted.formatted, "formatted.uix")
            .unwrap()
            .formatted,
        formatted.formatted
    );
    assert_eq!(formatted.cst.reconstruct(), formatted.formatted);
    let mut results = vec![];
    for source in [&source, &formatted.formatted] {
        let DocumentOutput::Module(output) =
            system.check_document_inline(source, "text.uix").unwrap()
        else {
            panic!("不能降级成 UI");
        };
        let mut instance =
            Instance::new(output.module, HostPorts::new(), Limits::default()).unwrap();
        results.push(
            instance
                .call("analyze", &[Value::String(" a  b ".into())])
                .unwrap(),
        );
    }
    assert_eq!(results[0], results[1]);
    let facts = system.query(QueryKind::parse("modules").unwrap()).entries;
    assert!(facts.iter().any(|fact| matches!(fact, QueryEntry::Module(entry) if entry.name == "dynamic-module" && entry.feature == "uix-dynamic")));
    assert!(facts.iter().any(|fact| matches!(fact, QueryEntry::Module(entry) if entry.name == "module-async" && entry.summary.contains("await"))));
    assert!(system.check_inline_auto(TEXT, "old-ui-entry.uix").is_err());
    assert!(
        load_module(
            &TEXT.replace(
                "return { text: text, count: text.words().len() };",
                "return { text: text, count: text.words().len() }"
            ),
            "missing-semicolon.uix"
        )
        .is_err()
    );
}

#[test]
fn equal_type_names_do_not_override_structural_type_identity() {
    let package = Package::new();
    let library = r#"<Module name="Library" version="1" schema="1"><Data name="Item" fields="value: Int" export="true"/></Module>"#;
    package.write("a.uix", library);
    package.write("b.uix", &library.replace("Int", "String"));
    package.write("main.uix", r#"<Module name="TypeConsumer" version="1" schema="1"><Import from="a.uix" as="a" version="1"/><Import from="b.uix" as="b" version="1"/><Function name="convert" params="value: a.Item" returns="b.Item" body="value" export="true"/></Module>"#);
    assert!(
        modules::check_file(&package.root())
            .unwrap_err()
            .message
            .contains("类型不符")
    );
    package.write("b.uix", library);
    assert!(modules::check_file(&package.root()).is_ok());
    package.write(
        "b.uix",
        &library.replace("export=\"true\"", "export=\"false\""),
    );
    assert!(
        modules::check_file(&package.root())
            .unwrap_err()
            .message
            .contains("b.Item")
    );
}

#[test]
fn decoded_multiline_body_diagnostics_and_runtime_locations_map_to_original_bytes() {
    let source = r#"<Module name="Spans" version="1" schema="1">
  <Function name="run" params="divisor: Int" returns="Int" export="true" body = "
    do { let label = '中😀\\n'; return missing(divisor); }
  " />
</Module>"#;
    let error = load_module(source, "spans.uix").unwrap_err();
    assert_eq!(error.start, source.find("missing").unwrap());
    assert_eq!(error.line, 3);
    assert_eq!(
        error.column,
        source[..error.start]
            .rsplit('\n')
            .next()
            .unwrap()
            .chars()
            .count()
            + 1
    );
    assert!(source[error.start..error.end].starts_with("missing("));
    let valid = source.replace("missing(divisor)", "1 / divisor");
    let mut instance = Instance::new(
        load_module(&valid, "spans.uix").unwrap(),
        HostPorts::new(),
        Limits::default(),
    )
    .unwrap();
    let runtime = instance.call("run", &[Value::Int(0)]).unwrap_err();
    assert_eq!(runtime.kind, ErrorKind::Arithmetic);
    let prefix = &valid[..valid.find("1 / divisor").unwrap()];
    assert_eq!(runtime.location.line, 3);
    assert_eq!(
        runtime.location.column,
        prefix.rsplit('\n').next().unwrap().chars().count() + 1
    );
}
