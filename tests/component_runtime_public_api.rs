//! 同一场景先动态执行，再在只启用 uix-components 的独立消费者中运行生成的原生函数。
#![cfg(feature = "uix-dynamic")]
use uix_app::lang::compiler::component_source as source;
include!("support/component_scenarios.rs");
static GENERATED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
fn compile(path: &std::path::Path, entry: &str, natives: &NativeBindings) -> Program {
    let mut interfaces = NativeLibraries::new();
    for (key, native) in natives {
        interfaces
            .entry(key.package.clone())
            .or_default()
            .insert(key.name.clone(), native.interface());
    }
    let checked = source::check(source::link_file(path).unwrap(), &interfaces).unwrap();
    let code = source::emit_native(&checked, entry).unwrap();
    if path.ends_with("pruning.uix") && entry == "Main" {
        assert!(!code.to_string().contains("UNREACHABLE_COMPONENT_SENTINEL"));
    }
    syn::parse2::<syn::Expr>(code.clone()).unwrap();
    GENERATED.lock().unwrap().push(code.to_string());
    source::lower(&checked, entry).unwrap()
}
#[test]
fn dynamic_and_independent_native_consumer() {
    let dynamic_outcome = run();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let consumer = root.join("target/component-native-consumer");
    fs::create_dir_all(consumer.join("src")).unwrap();
    let manifest = format!(
        r#"[package]
name="component-native-consumer"
version="0.0.0"
edition="2024"
[dependencies]
uix-app={{path={:?},default-features=false,features=["uix-components"]}}
[workspace]
"#,
        root.to_string_lossy()
    );
    fs::write(consumer.join("Cargo.toml"), manifest).unwrap();
    let generated = GENERATED.lock().unwrap();
    let mut code = String::from(
        "use uix_app::lang::runtime::components::Program;pub fn program(index:usize)->Program{match index{",
    );
    for (index, body) in generated.iter().enumerate() {
        code.push_str(&format!("{index}=>{body},"));
    }
    code.push_str("_=>panic!(\"missing generated case\"),}}");
    fs::write(consumer.join("src/lib.rs"), code).unwrap();
    drop(generated);
    let mut harness =
        fs::read_to_string(root.join("tests/support/component_scenarios.rs")).unwrap();
    harness.push_str(r#"
fn compile(_: &std::path::Path,_:&str,_:&NativeBindings)->Program{
static NEXT:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
component_native_consumer::program(NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed))
}
#[test]fn identical_component_scenarios(){let outcome=run();fs::write(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("native-outcome.txt"),outcome).unwrap();}
"#);
    fs::create_dir_all(consumer.join("tests")).unwrap();
    fs::write(consumer.join("tests/scenarios.rs"), harness).unwrap();
    let output = std::process::Command::new("cargo")
        .args(["test", "--offline", "-j2", "--manifest-path"])
        .arg(consumer.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", consumer.join("build"))
        .output()
        .unwrap();
    fs::write(consumer.join("cargo-test.stdout"), &output.stdout).unwrap();
    fs::write(consumer.join("cargo-test.stderr"), &output.stderr).unwrap();
    assert!(
        output.status.success(),
        "independent consumer failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::write(consumer.join("dynamic-outcome.txt"), &dynamic_outcome).unwrap();
    assert_eq!(
        dynamic_outcome,
        fs::read_to_string(consumer.join("native-outcome.txt")).unwrap(),
        "AOT/dynamic budget, failure location, and committed outcome differ"
    );
    let features = std::process::Command::new("cargo")
        .args(["tree", "--offline", "--manifest-path"])
        .arg(consumer.join("Cargo.toml"))
        .args(["-e", "features"])
        .env("CARGO_TARGET_DIR", consumer.join("build"))
        .output()
        .unwrap();
    assert!(features.status.success());
    let features = String::from_utf8(features.stdout).unwrap();
    fs::write(consumer.join("features.txt"), &features).unwrap();
    assert!(features.contains("uix-components"));
    for absent in [
        "uix-dynamic",
        "uix-modules",
        "lang-build",
        "quote v",
        "syn v",
        "regex v",
    ] {
        assert!(
            !features.contains(absent),
            "unexpected runtime dependency {absent}: {features}"
        );
    }
}
