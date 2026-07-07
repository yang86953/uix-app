use std::fs;
use std::path::{Path, PathBuf};

fn rust_files_under(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_files(dir, &mut files);
    files
}

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
}

fn relative_src_path(path: &Path) -> String {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    path.strip_prefix(src)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

fn is_native_backend_boundary(path: &str) -> bool {
    path == "native/factory.rs" || path.starts_with("native/backends/")
}

fn is_architecture_guard(path: &str) -> bool {
    path == "tests/architecture/boundaries.rs"
}

#[test]
fn platform_cfgs_stay_inside_native_boundary() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let platform_cfg_needles = [
        "cfg(windows",
        "cfg(unix",
        "cfg(target_os",
        "cfg(target_family",
    ];
    let mut violations = Vec::new();

    for file in rust_files_under(&src) {
        let rel = relative_src_path(&file);
        if is_architecture_guard(&rel) {
            continue;
        }
        let text = fs::read_to_string(&file).unwrap();
        if platform_cfg_needles
            .iter()
            .any(|needle| text.contains(needle))
        {
            if !is_native_backend_boundary(&rel) {
                violations.push(rel);
            }
        }
    }

    assert!(
        violations.is_empty(),
        "platform cfgs must stay in native/factory.rs or native/backends/**: {violations:?}"
    );
}

#[test]
fn native_backend_symbols_stay_inside_native_boundary() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut violations = Vec::new();

    for file in rust_files_under(&src) {
        let rel = relative_src_path(&file);
        if is_architecture_guard(&rel) {
            continue;
        }
        let text = fs::read_to_string(&file).unwrap();
        if text.contains("native::backends") {
            if !is_native_backend_boundary(&rel) {
                violations.push(rel);
            }
        }
    }

    assert!(
        violations.is_empty(),
        "native backend symbols must not leak above native boundary: {violations:?}"
    );
}

#[test]
fn data_domain_does_not_depend_on_upper_domains() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/data");
    let forbidden = [
        "crate::native",
        "crate::draw",
        "crate::ui",
        "crate::app",
        "super::super::native",
        "super::super::draw",
        "super::super::ui",
        "super::super::app",
    ];
    let mut violations = Vec::new();

    for file in rust_files_under(&data) {
        let text = fs::read_to_string(&file).unwrap();
        if forbidden.iter().any(|needle| text.contains(needle)) {
            violations.push(relative_src_path(&file));
        }
    }

    assert!(
        violations.is_empty(),
        "data domain must not depend on native/draw/ui/app: {violations:?}"
    );
}

#[test]
fn removed_compatibility_terms_do_not_return_to_source() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let removed_terms = [
        "preferred_size",
        "define_widget",
        "ScrollContainer",
        "LightIdle",
        "set_handler_generation",
        "crate::ui::WidgetNode",
        "uix::ui::WidgetNode",
        "grid_col_span",
    ];
    let mut violations = Vec::new();

    for file in rust_files_under(&src) {
        let rel = relative_src_path(&file);
        if is_architecture_guard(&rel) {
            continue;
        }
        let text = fs::read_to_string(&file).unwrap();
        for term in removed_terms {
            if text.contains(term) {
                violations.push(format!("{rel} contains {term}"));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "removed compatibility terms must not return to source: {violations:?}"
    );
}

#[test]
fn legacy_style_manager_stays_off_recommended_entrypoints() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let checked = ["prelude.rs", "ui/mod.rs"];
    let legacy_exports = ["StyleManager", "WidgetStylePreset"];
    let mut violations = Vec::new();

    for rel in checked {
        let text = fs::read_to_string(src.join(rel)).unwrap();
        for symbol in legacy_exports {
            if text.contains(symbol) {
                violations.push(format!("{rel} exposes {symbol}"));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "legacy style presets must stay behind ui::managers, not prelude/ui::*: {violations:?}"
    );
}
