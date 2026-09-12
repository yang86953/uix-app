//! `uix-lang-compiler/hot_reload.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use std::path::PathBuf;

use super::{HotReloadAdapter, HotReloadResult};

fn import_fixture() -> (PathBuf, PathBuf) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/uix_lang/imports/root.uix");
    let helper = root
        .parent()
        .expect("根文件必须有父目录")
        .join("shared/helper.uix");
    (root, helper)
}

fn ready(result: HotReloadResult) -> crate::CompileOutput {
    match result {
        HotReloadResult::Ready(output) => output,
        HotReloadResult::Unchanged { .. } => panic!("预期新的 AOT UI 结果"),
    }
}

#[test]
fn recursive_overlay_invalidates_once_without_changing_root_identity() {
    let (root, helper) = import_fixture();
    let mut adapter = HotReloadAdapter::new(root);
    let initial = ready(adapter.reload().expect("初始 UI 必须可编译"));

    let unchanged = adapter.reload().expect("相同源码图必须可重复检查");
    assert!(matches!(unchanged, HotReloadResult::Unchanged { .. }));

    adapter.set_overlay(
        helper,
        "@export('Helper')\n<Widget name=\"Helper\"><Text>热更新依赖</Text></Widget>\n<Helper />",
    );
    let changed = ready(adapter.reload().expect("递归依赖 overlay 必须可编译"));
    assert_eq!(
        initial.compilation_key.root_content_hash,
        changed.compilation_key.root_content_hash
    );
    assert_ne!(
        initial.compilation_key.dependency_hash,
        changed.compilation_key.dependency_hash
    );
}

#[test]
fn failed_overlay_preserves_last_successful_identity() {
    let (root, helper) = import_fixture();
    let mut adapter = HotReloadAdapter::new(root);
    let initial = ready(adapter.reload().expect("初始 UI 必须可编译"));
    let initial_key = initial.compilation_key.clone();

    adapter.set_overlay(
        &helper,
        "@export('Helper')\n<Widget name=\"Helper\"><Mystery /></Widget>\n<Helper />",
    );
    assert!(adapter.reload().is_err());
    assert_eq!(adapter.last_ready_key(), Some(&initial_key));

    assert!(adapter.remove_overlay(&helper).is_some());
    assert_eq!(adapter.overlay_count(), 0);
    let recovered = adapter.reload().expect("移除无效 overlay 后必须恢复");
    assert!(matches!(recovered, HotReloadResult::Unchanged { .. }));
}
