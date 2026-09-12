//! Library declarations must work without knowledge of an official component package.
#![cfg(feature = "lang-tools")]
use serde_json::json;
use std::{fs, path::PathBuf, sync::Arc};
use uix_app::lang::{
    compiler::{
        CompileTarget, CompilerSystem, QueryEntry, QueryKind, components::ComponentCatalog,
    },
    demand,
};

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "uix-catalog-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("library/src")).unwrap();
        fs::write(root.join("Cargo.toml"), "[package]\nname='consumer'\nversion='0.1.0'\nedition='2024'\n[dependencies]\nsample-ui={path='library'}\n").unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
        fs::write(root.join("library/Cargo.toml"), "[package]\nname='sample-ui'\nversion='0.1.0'\nedition='2024'\n[dependencies]\nheavy={version='1',optional=true}\n[features]\noptional-heavy=['dep:heavy']\n").unwrap();
        fs::write(root.join("library/src/lib.rs"), "pub fn pill() {}\n").unwrap();
        fs::write(root.join("library/shade.bin"), "palette").unwrap();
        let descriptor = json!({
            "version":1,"package":"sample-ui",
            "units": {"pill":{},"flavor":{"resources":["shade.bin"]},"expensive":{"dependencies":["heavy"]}},
            "components": {
                "Pill":{"unit":"sample-ui/pill","constructor":"::sample_ui::pill($children)","children":{"kind":"text"},"properties":{"value":{"kind":"expression","method":"value"},"count":{"kind":"integer","minimum":0,"maximum":5,"method":"count"}}},
                "Expensive":{"unit":"sample-ui/expensive","constructor":"::sample_ui::expensive()"}
            },
            "data_constructors":{"Flavor":{"unit":"sample-ui/flavor","rust_path":"::sample_ui::Flavor"}}
        });
        fs::write(
            root.join("library/uix-library.json"),
            serde_json::to_vec_pretty(&descriptor).unwrap(),
        )
        .unwrap();
        fs::write(root.join("uix.json"), serde_json::to_vec(&json!({"packages":[{"path":"library"}],"sources":["main.uix"],"target":"x86_64-unknown-linux-gnu"})).unwrap()).unwrap();
        Self(root)
    }
    fn source(&self, source: &str) {
        fs::write(self.0.join("main.uix"), source).unwrap();
    }
    fn catalog(&self) -> Arc<ComponentCatalog> {
        Arc::new(ComponentCatalog::for_project(&self.0).unwrap())
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn third_party_components_and_data_report_only_emitted_units() {
    let project = Project::new();
    project.source(
        "<Widget name=\"Unused\"><Expensive /></Widget><Pill value={Flavor('mint')}>Hello</Pill>",
    );
    let output = CompilerSystem::new()
        .compile_file_auto(&project.0.join("main.uix"))
        .unwrap();
    assert_eq!(
        output.required_units,
        ["sample-ui/flavor".into(), "sample-ui/pill".into()].into()
    );
    assert!(output.tokens.to_string().contains("sample_ui"));
}

#[test]
fn catalog_scope_does_not_leak_to_other_compilations() {
    let project = Project::new();
    project.catalog().with(|| {
        CompilerSystem::new()
            .check_inline("<Pill>ok</Pill>", "inline", CompileTarget::View)
            .unwrap();
    });
    assert!(
        CompilerSystem::new()
            .check_inline("<Pill>ok</Pill>", "inline", CompileTarget::View)
            .is_err()
    );
}

#[test]
fn declared_integer_bounds_reject_negative_and_fractional_values() {
    let project = Project::new();
    project.catalog().with(|| {
        for source in [
            "<Pill count=\"-1\" />",
            "<Pill count=\"6\" />",
            "<Pill count={2.5} />",
        ] {
            assert!(
                CompilerSystem::new()
                    .check_inline(source, "invalid", CompileTarget::View)
                    .is_err(),
                "{source}"
            );
        }
        CompilerSystem::new()
            .check_inline("<Pill count=\"5\" />", "valid", CompileTarget::View)
            .unwrap();
    });
}

#[test]
fn queries_include_project_library_components_and_data() {
    let project = Project::new();
    for (kind, expected) in [(QueryKind::Components, "Pill"), (QueryKind::Data, "Flavor")] {
        let output = CompilerSystem::new()
            .query_for_project(&project.0, kind)
            .unwrap();
        assert!(
            output
                .entries
                .iter()
                .any(|entry| matches!(entry, QueryEntry::Library { name, .. } if name == expected))
        );
    }
    assert!(
        !CompilerSystem::new()
            .query(QueryKind::Components)
            .entries
            .iter()
            .any(|entry| matches!(entry, QueryEntry::Library { .. }))
    );
}

#[test]
fn build_view_prunes_unused_dependency_and_copies_selected_resources() {
    let project = Project::new();
    project.source("<Pill value={Flavor('mint')}>Hello</Pill>");
    let plan = demand::plan(&project.0.join("uix.json"), None).unwrap();
    assert_eq!(
        plan.packages["sample-ui"].selected,
        ["flavor".into(), "pill".into()].into()
    );
    let manifest_path = plan.materialize().unwrap();
    let view = manifest_path.parent().unwrap().parent().unwrap();
    let manifest = fs::read_to_string(view.join("packages/sample-ui/Cargo.toml")).unwrap();
    let manifest: toml::Value = toml::from_str(&manifest).unwrap();
    assert_eq!(
        manifest["dependencies"]["heavy"]["optional"].as_bool(),
        Some(true)
    );
    assert!(
        !manifest["features"]["__uix_selected"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str() == Some("dep:heavy"))
    );
    assert_eq!(
        fs::read_to_string(view.join("resources/sample-ui/shade.bin")).unwrap(),
        "palette"
    );
    let descriptor: serde_json::Value = serde_json::from_slice(
        &fs::read(view.join("packages/sample-ui/uix-library.json")).unwrap(),
    )
    .unwrap();
    assert!(descriptor["components"].get("Expensive").is_none());
    assert!(descriptor["data_constructors"].get("Flavor").is_some());
    assert!(
        fs::read_to_string(project.0.join("library/Cargo.toml"))
            .unwrap()
            .contains("dep:heavy")
    );
}

#[test]
fn portable_modules_accept_library_defined_view_contracts() {
    use uix_app::lang::{
        compiler::modules,
        runtime::{HostPorts, Instance, Limits, Value},
    };
    let project = Project::new();
    let path = project.0.join("library/uix-library.json");
    let mut descriptor: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    descriptor["components"]["Pill"]["module_view"] = json!({"properties":{"caption":"default"}});
    fs::write(&path, serde_json::to_vec(&descriptor).unwrap()).unwrap();
    project.source(r#"<Module name="Custom" version="1" schema="1"><View><Pill key="root" caption="mint" /></View></Module>"#);
    let output = modules::check_file(&project.0.join("main.uix")).unwrap();
    let mut instance = Instance::new(output.module, HostPorts::new(), Limits::default()).unwrap();
    let view = instance.view().unwrap().unwrap();
    assert_eq!(view.kind, "Pill");
    assert_eq!(view.properties["caption"], Value::String("mint".into()));
}

#[test]
fn app_source_selects_application_while_a_view_does_not() {
    let project = Project::new();
    project.source("<App><Pill>Hello</Pill></App>");
    let app = CompilerSystem::new()
        .compile_file_auto(&project.0.join("main.uix"))
        .unwrap();
    assert!(app.required_units.contains("uix-app/application"));
    project.source("<Pill>Hello</Pill>");
    let view = CompilerSystem::new()
        .compile_file_auto(&project.0.join("main.uix"))
        .unwrap();
    assert!(!view.required_units.contains("uix-app/application"));
}

fn theme_catalog(project: &Project, spacing_key: &str) -> Arc<ComponentCatalog> {
    let path = project.0.join("library/uix-library.json");
    let mut descriptor: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    descriptor["units"]["theme"] = json!({});
    descriptor["theme_tokens"] = json!({
        "accent": {"key":"sample.accent","unit":"sample-ui/theme","kind":"color"},
        "spacing": {"key":spacing_key,"unit":"sample-ui/theme","kind":"number","minimum":0,"allow_px":true,"fallback":7},
        "surfaceShadow": {"key":"sample.shadow","unit":"sample-ui/theme","kind":"shadow","shadow_layers":2}
    });
    fs::write(path, serde_json::to_vec(&descriptor).unwrap()).unwrap();
    project.catalog()
}

#[test]
fn third_party_theme_schema_drives_types_queries_and_incremental_identity() {
    let project = Project::new();
    let source = "<Pill style=\"color: #accent; padding: #spacing;\">custom</Pill>";
    let first = theme_catalog(&project, "sample.spacing").with(|| {
        let output = CompilerSystem::new().compile_inline(source, "custom", CompileTarget::View).unwrap();
        assert!(output.required_units.contains("sample-ui/theme"));
        let tokens = output.tokens.to_string();
        assert!(tokens.contains("sample.accent") && tokens.contains("sample.spacing"));
        let query = CompilerSystem::new().query(QueryKind::Themes);
        assert!(query.entries.iter().any(|entry| matches!(entry, QueryEntry::Theme(spec) if spec.name == "accent" && spec.key == "sample.accent")));
        assert!(CompilerSystem::new().check_inline("<Pill style=\"color: #spacing;\" />", "wrong-type", CompileTarget::View).is_err());
        output.compilation_key.component_catalog_hash
    });
    let second = theme_catalog(&project, "other.spacing").with(|| {
        CompilerSystem::new()
            .compile_inline(source, "custom", CompileTarget::View)
            .unwrap()
            .compilation_key
            .component_catalog_hash
    });
    assert_ne!(
        first, second,
        "a changed theme schema must invalidate compiled output"
    );
    assert!(
        CompilerSystem::new()
            .query(QueryKind::Themes)
            .entries
            .is_empty()
    );
}

#[test]
fn third_party_theme_supports_its_own_shadow_shape_and_value_bounds() {
    let project = Project::new();
    theme_catalog(&project, "sample.spacing").with(|| {
        let source = "@theme light { spacing: 12px; surfaceShadow: 0 2px 3px #000, 1px 3px 4px #fff; }<App><Pill>ok</Pill></App>";
        let output = CompilerSystem::new().compile_inline(source, "custom-app", CompileTarget::App).unwrap();
        assert!(output.tokens.to_string().contains("sample.shadow"));
        assert!(output.required_units.contains("sample-ui/theme"));
        for source in [
            "@theme light { spacing: -1px; }<App><Pill /></App>",
            "@theme light { surfaceShadow: 0 2px 3px #000; }<App><Pill /></App>",
        ] {
            assert!(CompilerSystem::new().check_inline(source, "invalid", CompileTarget::App).is_err());
        }
    });
}
