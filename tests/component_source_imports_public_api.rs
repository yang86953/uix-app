//! 通过公开入口读取真实文件/编辑器快照；框架不知道官方控件名称。
#![cfg(feature = "lang-build")]

use std::{collections::BTreeMap, fs, path::PathBuf};
use uix_app::lang::compiler::{DiagnosticPhase, component_source::*};

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/component-source-projects")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, path: &str, content: &str) -> PathBuf {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
        path
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn pure_component_exports_and_aliases_share_one_closed_source_graph() {
    let project = Project::new();
    let entry = project.write(
        "editable_list.uix",
        include_str!("fixtures/components/editable_list.uix"),
    );
    project.write(
        "counter.uix",
        include_str!("fixtures/components/counter.uix"),
    );
    project.write("card.uix", include_str!("fixtures/components/card.uix"));
    let linked = link_file(&entry).unwrap();
    assert_eq!(linked.source_graph.files().len(), 3);
    assert_eq!(linked.source_graph.imports().len(), 2);
    let root = &linked.units[&linked.source_graph.root()];
    let Binding::Declaration(counter_id) = root.bindings["Count"] else {
        panic!();
    };
    assert_eq!(
        linked.units[&counter_id.source_id].declarations[counter_id.index]
            .name
            .text,
        "Counter"
    );
    assert!(
        matches!(&root.bindings["Input"], Binding::NativeImport { package, name } if package == "sample-native" && name == "Input")
    );
    assert!(!root.bindings.contains_key("caption")); // 未选择的导出不泄漏。
    let Binding::Declaration(card_id) = root.bindings["Card"] else {
        panic!();
    };
    assert!(
        !linked.units[&card_id.source_id]
            .bindings
            .contains_key("Row")
    );
    // 单次读取后删除依赖也不改变已有快照、导出身份或摘要。
    let hash = linked.source_graph.dependency_hash();
    fs::remove_file(project.0.join("counter.uix")).unwrap();
    assert_eq!(linked.source_graph.dependency_hash(), hash);
    assert!(
        linked
            .source_graph
            .file(counter_id.source_id)
            .unwrap()
            .source
            .contains("state count")
    );
}

#[test]
fn editor_overlays_replace_dependencies_and_can_supply_unsaved_files() {
    let project = Project::new();
    let entry = project.write(
        "main.uix",
        "import { Card } from './card.uix'; export component Main() { return <Card/>; }",
    );
    let card = project.write(
        "card.uix",
        "export component Card() { return <Text>disk</Text>; }",
    );
    let original = link_file(&entry).unwrap();
    let overlays = BTreeMap::from([
        (
            entry.clone(),
            "import { NewCard } from './new.uix'; export component Main() { return <NewCard/>; }"
                .into(),
        ),
        (
            project.0.join("new.uix"),
            "export component NewCard() { return <Text>未保存</Text>; }".into(),
        ),
        (card, "invalid and unused overlay".into()),
    ]);
    let edited = link_file_with_overlays(&entry, &overlays).unwrap();
    assert_ne!(
        original.source_graph.dependency_hash(),
        edited.source_graph.dependency_hash()
    );
    assert_eq!(edited.source_graph.files().len(), 2);
    assert!(
        edited.units[&edited.source_graph.root()]
            .bindings
            .contains_key("NewCard")
    );
    assert!(fs::read_to_string(entry).unwrap().contains("./card.uix"));
    assert!(!project.0.join("new.uix").exists());
}

#[test]
fn repeated_imports_do_not_duplicate_definitions_and_equal_names_in_different_files_do_not_collide()
{
    let project = Project::new();
    let entry = project.write("main.uix", "import { Counter as Left } from './left/counter.uix'; import { Counter as Same } from './left/counter.uix'; import { Counter as Right } from './right/counter.uix'; export component Main() { return <><Left/><Right/></>; }");
    for directory in ["left", "right"] {
        project.write(
            &format!("{directory}/counter.uix"),
            include_str!("fixtures/components/counter.uix"),
        );
    }
    let linked = link_file(&entry).unwrap();
    let root = &linked.units[&linked.source_graph.root()];
    assert_eq!(root.bindings["Left"], root.bindings["Same"]);
    assert_ne!(root.bindings["Left"], root.bindings["Right"]);
    assert_eq!(linked.source_graph.files().len(), 3);
    assert_eq!(linked.source_graph.imports().len(), 3);
}

#[test]
fn private_missing_duplicate_and_cyclic_imports_fail_at_the_original_reference() {
    let project = Project::new();
    project.write("hidden.uix", "component Hidden() { return <Text/>; }");
    project.write(
        "broken.uix",
        "export component Broken() { return <Text></Wrong>; }",
    );
    project.write(
        "cycle.uix",
        "import { Main } from './main.uix'; export component Cycle() { return <Main/>; }",
    );
    for (source, code, slice, file) in [
        (
            "import { Hidden } from './hidden.uix';",
            "component-import-export",
            "Hidden",
            "main.uix",
        ),
        (
            "import { Missing } from './missing.uix';",
            "component-import-source",
            "'./missing.uix'",
            "main.uix",
        ),
        (
            "import { Broken } from './broken.uix';",
            "component-closing-tag",
            "W",
            "broken.uix",
        ),
        (
            "import { Cycle } from './cycle.uix'; export component Main() { return <Cycle/>; }",
            "component-import-cycle",
            "'./main.uix'",
            "cycle.uix",
        ),
        (
            "import { Text as Main } from 'native'; export component Main() { return <Text/>; }",
            "component-duplicate-name",
            "Main",
            "main.uix",
        ),
        (
            "type Item = Int; type Item = String;",
            "component-duplicate-name",
            "Item",
            "main.uix",
        ),
        (
            "import { X } from './host.rs';",
            "component-import-path",
            "'./host.rs'",
            "main.uix",
        ),
    ] {
        let entry = project.write("main.uix", source);
        let error = link_file(&entry).unwrap_err();
        assert_eq!(error.code, code, "{source}: {error:?}");
        assert!(error.source_name.ends_with(file));
        let original = fs::read_to_string(&error.source_name).unwrap();
        assert_eq!(&original[error.start..error.end], slice);
        assert_eq!(
            error.phase,
            if code == "component-closing-tag" {
                DiagnosticPhase::Syntax
            } else {
                DiagnosticPhase::Import
            }
        );
    }
}
