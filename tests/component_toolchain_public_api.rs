//! 真实 CLI / stdio LSP 消费同一项目接口与 UTF-8 来源，不只测试内部适配函数。
#![cfg(feature = "lang-tools")]
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdout, Command, Output, Stdio},
};
use uix_app::lang::{
    build::Builder,
    compiler::{CompilerSystem, DocumentOutput, component_source::*},
    runtime::Type as DataType,
};

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/component-toolchain-projects")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, source).unwrap();
        path
    }
    fn setup(&self) -> (PathBuf, PathBuf, NativeLibraries) {
        let libraries = libraries(DataType::String);
        self.write("native.json", &interface::encode(&libraries).unwrap());
        // 新组件工具不得读取无关的旧描述符，即使旧 catalog 配置不再可用。
        self.write(
            "uix.json",
            r#"{"packages":[{"path":"does-not-exist"}],"component_libraries":["native.json"]}"#,
        );
        let child = self.write("child.uix", CHILD);
        let root = self.write("main.uix", "/* 注释中的 <App/> */\nimport { Child as Card } from './child.uix'; export component Main(){ return <Card/>; }");
        (root, child, libraries)
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
const CHILD: &str = "import {Text} from 'native';\nexport component Child(){let label=\"中文😀\"; return <Text>{label}</Text>;}\n";
fn libraries(children: DataType) -> NativeLibraries {
    BTreeMap::from([(
        "native".into(),
        BTreeMap::from([(
            "Text".into(),
            NativeExport::Component(ComponentSignature {
                parameters: vec![("children".into(), Type::Data(children))],
                required: ["children".into()].into(),
            }),
        )]),
    )])
}
fn cli(project: &Project, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_uix"))
        .current_dir(&project.0)
        .args(args)
        .output()
        .unwrap()
}
fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

#[test]
fn interfaces_reject_ambiguous_invalid_and_unbounded_contracts() {
    let input = libraries(DataType::String);
    let encoded = interface::encode(&input).unwrap();
    assert_eq!(interface::decode(&encoded, "native.json").unwrap(), input);
    for source in [
        r#"{"version":1,"libraries":{"native":{},"native":{}}}"#,
        r#"{"version":1,"libraries":{"native":{"T":{"Type":{"Data":{"Record":{"a":"Int","a":"String"}}}}}}}"#,
        r#"{"version":2,"libraries":{}}"#,
        r#"{"version":1,"libraries":{},"extra":true}"#,
        r#"{"version":1,"libraries":{"n":{"F":{"Function":{"minimum_arguments":2,"parameters":[],"returns":"View","effect":"Pure"}}}}}"#,
    ] {
        assert!(interface::decode(source, "bad.json").is_err(), "{source}");
    }
    assert!(interface::decode(&" ".repeat(1_048_577), "huge.json").is_err());
    let invalid = libraries(DataType::String)
        .into_iter()
        .map(|(p, mut exports)| {
            let NativeExport::Component(sig) = exports.get_mut("Text").unwrap() else {
                panic!()
            };
            sig.required.insert("absent".into());
            (p, exports)
        })
        .collect();
    assert!(interface::encode(&invalid).is_err());
    let mut ty = DataType::Int;
    for _ in 0..70 {
        ty = DataType::Array(Box::new(ty));
    }
    assert!(interface::encode(&libraries(ty)).is_err());
    let project = Project::new();
    let builder = Builder::new(&project.0, project.0.join("out"));
    let artifact = builder
        .export_component_interfaces("native.json", &input)
        .unwrap();
    assert_eq!(fs::read_to_string(&artifact).unwrap(), encoded);
    let modified = fs::metadata(&artifact).unwrap().modified().unwrap();
    builder
        .export_component_interfaces("native.json", &input)
        .unwrap();
    assert_eq!(
        fs::metadata(&artifact).unwrap().modified().unwrap(),
        modified
    );
    assert!(
        builder
            .export_component_interfaces("../escape.json", &input)
            .is_err()
    );
    assert!(
        builder
            .export_component_interfaces("native.rs", &input)
            .is_err()
    );
    assert!(
        builder
            .export_component_interfaces("native.json", &invalid)
            .is_err()
    );
    assert_eq!(fs::read_to_string(artifact).unwrap(), encoded);
    project.write("one.json", &encoded);
    project.write("two.json", &encoded);
    project.write(
        "uix.json",
        r#"{"component_libraries":["one.json","two.json"]}"#,
    );
    assert!(
        interface::load_project(&project.0)
            .unwrap_err()
            .message
            .contains("重复原生导出")
    );
    project.write(
        "uix.json",
        r#"{"component_libraries":["one.json","./one.json"]}"#,
    );
    assert_eq!(
        interface::load_project(&project.0)
            .unwrap()
            .tracked_files
            .len(),
        2
    );
    project.write(
        "uix.json",
        r#"{"component_libraries":[],"component_libraries":["one.json"]}"#,
    );
    assert!(interface::load_project(&project.0).is_err());
}

#[test]
fn cli_and_cargo_share_native_signatures_entry_selection_and_dependency_diagnostics() {
    let project = Project::new();
    let (root, child, libraries) = project.setup();
    let system = CompilerSystem::new();
    let DocumentOutput::Component(output) = system.check_document_file(&root).unwrap() else {
        panic!()
    };
    assert_eq!(output.checked.source().source_graph.files().len(), 2);
    assert_eq!(output.interface_files.len(), 2);
    for args in [
        vec!["check", "main.uix", "--json"],
        vec!["check", "--target", "component", "main.uix"],
    ] {
        let result = cli(&project, &args);
        assert!(result.status.success(), "{:?}", result);
    }
    let compiled = cli(
        &project,
        &[
            "compile",
            "--target",
            "component",
            "--entry",
            "Main",
            "main.uix",
        ],
    );
    assert!(compiled.status.success(), "{:?}", compiled);
    assert_eq!(
        stdout(&compiled).trim(),
        emit_native(&output.checked, "Main").unwrap().to_string()
    );
    assert!(!stdout(&compiled).contains("Body :: Dynamic"));
    let artifact = Builder::new(&project.0, project.0.join("out"))
        .compile_component("main.uix", "Main", &libraries)
        .unwrap();
    assert!(
        fs::read_to_string(artifact)
            .unwrap()
            .split_whitespace()
            .collect::<String>()
            .contains("Body::Native")
    );
    let query = cli(
        &project,
        &["query", "native-exports", "--project", ".", "--json"],
    );
    assert!(query.status.success(), "{:?}", query);
    let query: Value = serde_json::from_slice(&query.stdout).unwrap();
    assert_eq!(query[0]["name"], "Text");
    assert!(
        !cli(&project, &["query", "native-exports", "--project"])
            .status
            .success()
    );
    assert!(
        !cli(&project, &["query", "native-exports", "--unknown"])
            .status
            .success()
    );
    let before = fs::read_to_string(&root).unwrap();
    assert!(
        !cli(&project, &["fmt", "--check", "main.uix"])
            .status
            .success()
    );
    assert!(cli(&project, &["fmt", "main.uix"]).status.success());
    assert_eq!(
        fs::read_to_string(&root).unwrap(),
        system
            .format_inline(&before, root.display().to_string())
            .unwrap()
            .formatted
    );
    assert!(
        cli(&project, &["fmt", "--check", "main.uix"])
            .status
            .success()
    );
    let invalid = project.write("bad-format.uix", "export component Main(){");
    assert!(!cli(&project, &["fmt", "bad-format.uix"]).status.success());
    assert_eq!(
        fs::read_to_string(invalid).unwrap(),
        "export component Main(){"
    );
    assert_eq!(
        query[0]["signature"],
        serde_json::to_value(&libraries["native"]["Text"]).unwrap()
    );
    for args in [
        vec!["compile", "--target", "component", "main.uix"],
        vec![
            "compile",
            "--target",
            "component",
            "--entry",
            "Missing",
            "main.uix",
        ],
        vec![
            "compile",
            "--target",
            "component",
            "--entry",
            "Main",
            "--json",
            "main.uix",
        ],
        vec!["check", "--entry", "Main", "main.uix"],
    ] {
        assert!(!cli(&project, &args).status.success());
    }
    let broken = CHILD.replace("{label}", "{未定义}");
    fs::write(&child, &broken).unwrap();
    let error = system.check_document_file(&root).unwrap_err();
    assert_eq!(&broken[error.start..error.end], "未定义");
    let result = cli(&project, &["check", "main.uix", "--json"]);
    assert!(!result.status.success());
    let diagnostic: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(diagnostic["code"], error.code);
    assert_eq!(diagnostic["path"], error.source_name);
    assert_eq!(diagnostic["start"], error.start);
    assert_eq!(diagnostic["end"], error.end);
    // 虚拟缓冲区的同一连接器不猜测相对路径；纯声明/原生绑定可用。
    assert!(
        system
            .check_component_inline(CHILD, "untitled:中文", &libraries)
            .is_ok()
    );
    assert_eq!(
        system
            .check_component_inline("import {X} from './x.uix';", "untitled:test", &libraries)
            .unwrap_err()
            .code,
        "component-inline-import"
    );
    assert!(matches!(
        system
            .check_document_inline(
                "// hello\nexport component Empty(){return <> </>;}",
                "untitled:test"
            )
            .unwrap(),
        DocumentOutput::Component(_)
    ));
    project.write(
        "large.uix",
        &format!(
            "export component Big(){{return <> </>;}}{}",
            " ".repeat(1_048_576)
        ),
    );
    let result = cli(&project, &["check", "large.uix", "--json"]);
    assert!(!result.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&result.stdout).unwrap()["code"],
        "component-source-limit"
    );
}

struct Lsp {
    child: Child,
    output: BufReader<ChildStdout>,
    next: u64,
}
impl Lsp {
    fn new() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_uix"))
            .arg("lsp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        let mut lsp = Self {
            child,
            output,
            next: 0,
        };
        lsp.send(json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}));
        assert_eq!(lsp.read()["id"], 0);
        lsp.send(json!({"jsonrpc":"2.0","method":"initialized","params":{}}));
        lsp
    }
    fn send(&mut self, value: Value) {
        let bytes = serde_json::to_vec(&value).unwrap();
        let input = self.child.stdin.as_mut().unwrap();
        write!(input, "Content-Length: {}\r\n\r\n", bytes.len()).unwrap();
        input.write_all(&bytes).unwrap();
        input.flush().unwrap();
    }
    fn read(&mut self) -> Value {
        let mut length = None;
        loop {
            let mut line = String::new();
            assert!(self.output.read_line(&mut line).unwrap() > 0);
            if line == "\r\n" {
                break;
            }
            if let Some(value) = line.strip_prefix("Content-Length: ") {
                length = Some(value.trim().parse::<usize>().unwrap());
            }
        }
        let mut bytes = vec![0; length.unwrap()];
        self.output.read_exact(&mut bytes).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }
    fn change(&mut self, method: &str, params: Value) -> Vec<Value> {
        self.send(json!({"jsonrpc":"2.0","method":method,"params":params}));
        self.next += 1;
        let id = self.next;
        // 未知请求的 MethodNotFound 响应只作帧屏障，不重复 initialize 或依赖 sleep。
        self.send(json!({"jsonrpc":"2.0","id":id,"method":"uix/test-barrier","params":{}}));
        let mut values = Vec::new();
        loop {
            let value = self.read();
            if value["id"] == id {
                break;
            }
            values.push(value);
        }
        values
    }
}
impl Drop for Lsp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn uri(path: &Path) -> String {
    format!("file://{}", path.display())
}
fn diagnostics(messages: &[Value], target: &str) -> Vec<Value> {
    messages
        .iter()
        .rev()
        .find(|value| value["params"]["uri"] == target)
        .and_then(|value| value["params"]["diagnostics"].as_array())
        .cloned()
        .unwrap()
}

#[test]
fn stdio_lsp_rechecks_dependency_overlays_and_interface_watches_with_utf16_ranges() {
    let project = Project::new();
    let (root, child, libraries) = project.setup();
    let root_uri = uri(&root);
    let child_uri = uri(&child);
    let mut lsp = Lsp::new();
    let opened = lsp.change(
        "textDocument/didOpen",
        json!({"textDocument":{"uri":root_uri,"text":fs::read_to_string(&root).unwrap()}}),
    );
    assert!(diagnostics(&opened, &root_uri).is_empty());
    let broken = CHILD.replace("{label}", "{未定义}");
    let changed = lsp.change(
        "textDocument/didOpen",
        json!({"textDocument":{"uri":child_uri,"text":broken}}),
    );
    let direct = CompilerSystem::new()
        .check_component_file_with_overlays(
            &root,
            &BTreeMap::from([(child.clone(), broken.clone())]),
        )
        .unwrap_err();
    let errors = diagnostics(&changed, &child_uri);
    assert!(!errors.is_empty());
    for error in errors {
        assert_eq!(error["code"], direct.code);
        assert_eq!(error["data"]["byteRange"]["start"], direct.start);
        let prefix = &broken[..direct.start];
        assert_eq!(
            error["range"]["start"]["line"],
            prefix.bytes().filter(|b| *b == b'\n').count()
        );
        assert_eq!(
            error["range"]["start"]["character"],
            prefix.rsplit('\n').next().unwrap().encode_utf16().count()
        );
    }
    lsp.send(json!({"jsonrpc":"2.0","id":800,"method":"textDocument/formatting","params":{"textDocument":{"uri":child_uri}}}));
    let response = lsp.read();
    assert_eq!(response["id"], 800);
    let formatted = CompilerSystem::new()
        .format_inline(&broken, &child_uri)
        .unwrap()
        .formatted;
    assert_eq!(response["result"][0]["newText"], formatted);
    assert_eq!(
        response["result"][0]["range"]["end"],
        json!({"line":2,"character":0})
    );
    assert_eq!(fs::read_to_string(&child).unwrap(), CHILD); // LSP 返回编辑，不直接写磁盘。
    let closed = lsp.change(
        "textDocument/didClose",
        json!({"textDocument":{"uri":child_uri}}),
    );
    assert!(diagnostics(&closed, &child_uri).is_empty());
    let native = project.write(
        "native.json",
        &interface::encode(&self::libraries(DataType::Int)).unwrap(),
    );
    let changed = lsp.change(
        "workspace/didChangeWatchedFiles",
        json!({"changes":[{"uri":uri(&native),"type":2}]}),
    );
    assert!(!diagnostics(&changed, &child_uri).is_empty());
    fs::write(&native, interface::encode(&libraries).unwrap()).unwrap();
    let restored = lsp.change(
        "workspace/didChangeWatchedFiles",
        json!({"changes":[{"uri":uri(&native),"type":2}]}),
    );
    assert!(diagnostics(&restored, &child_uri).is_empty());
    let closed = lsp.change(
        "textDocument/didClose",
        json!({"textDocument":{"uri":root_uri}}),
    );
    assert!(diagnostics(&closed, &root_uri).is_empty());
    lsp.send(json!({"jsonrpc":"2.0","id":900,"method":"shutdown","params":null}));
    assert_eq!(lsp.read()["id"], 900);
    lsp.send(json!({"jsonrpc":"2.0","method":"exit"}));
    assert!(lsp.child.wait().unwrap().success());
}
