//! 构建入口的多导出隔离和失败边界，不修改已发布旧宏。
#![cfg(feature = "lang-build")]
use std::{collections::BTreeMap, fs, path::PathBuf};
use uix_app::lang::{
    build::Builder,
    runtime::{Type as DataType, components::*},
};

#[test]
fn component_builds_isolate_exports_and_keep_the_last_valid_file_on_failure() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/component-build-projects")
        .join(std::process::id().to_string());
    fs::create_dir_all(root.join("src")).unwrap();
    let source = root.join("src/main.uix");
    fs::write(
        &source,
        r#"import {Text} from "native";
        export component First(){return <Text>first</Text>;}
        export component Second(){return <Text>second</Text>;}
    "#,
    )
    .unwrap();
    let libraries = BTreeMap::from([(
        "native".into(),
        BTreeMap::from([(
            "Text".into(),
            NativeExport::Component(ComponentSignature {
                parameters: vec![("children".into(), Type::Data(DataType::String))],
                required: ["children".into()].into(),
            }),
        )]),
    )]);
    let builder = Builder::new(&root, root.join("out"));
    let first = builder
        .compile_component("src/main.uix", "First", &libraries)
        .unwrap();
    let second = builder
        .compile_component("src/main.uix", "Second", &libraries)
        .unwrap();
    assert_ne!(first, second);
    let before = fs::read_to_string(&first).unwrap();
    assert!(before.contains("\"first\""));
    assert!(!before.contains("\"second\""));
    assert!(fs::read_to_string(&second).unwrap().contains("\"second\""));
    let modified = fs::metadata(&first).unwrap().modified().unwrap();
    builder
        .compile_component("src/main.uix", "First", &libraries)
        .unwrap();
    assert_eq!(fs::metadata(&first).unwrap().modified().unwrap(), modified);
    assert!(
        builder
            .compile_component("../main.uix", "First", &libraries)
            .is_err()
    );
    assert!(
        builder
            .compile_component("src/main.uix", "../escape", &libraries)
            .is_err()
    );
    assert!(
        builder
            .compile_component("src/main.uix", "First", &NativeLibraries::new())
            .is_err()
    );
    fs::write(
        &source,
        r#"import {Text} from "native";
        export component First(){return <Text>changed</Text>;}
        component Unused(){return missing;}
    "#,
    )
    .unwrap();
    let error = builder
        .compile_component("src/main.uix", "First", &libraries)
        .unwrap_err();
    assert!(error.contains("main.uix"));
    assert_eq!(fs::read_to_string(&first).unwrap(), before);
}
