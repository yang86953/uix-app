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

fn assert_domain_has_no_forbidden_dependencies(domain: &str, forbidden: &[&str]) {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let root = src.join(domain);
    let mut violations = Vec::new();

    for file in rust_files_under(&root) {
        let text = fs::read_to_string(&file).unwrap();
        let rel = relative_src_path(&file);
        for needle in forbidden {
            if text.contains(needle) {
                violations.push(format!("{rel} contains {needle}"));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "{domain} domain must not depend on forbidden upper domains: {violations:?}"
    );
}

#[test]
fn platform_cfgs_stay_inside_native_boundary() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let platform_cfg_needles = [
        "cfg(windows",
        "cfg(unix",
        "cfg(all(windows",
        "cfg(all(unix",
        "cfg(any(windows",
        "cfg(any(unix",
        "cfg(not(windows",
        "cfg(not(unix",
        "cfg_attr(windows",
        "cfg_attr(unix",
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
            && !is_native_backend_boundary(&rel)
        {
            violations.push(rel);
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
        if text.contains("native::backends") && !is_native_backend_boundary(&rel) {
            violations.push(rel);
        }
    }

    assert!(
        violations.is_empty(),
        "native backend symbols must not leak above native boundary: {violations:?}"
    );
}

#[test]
fn data_domain_does_not_depend_on_upper_domains() {
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
    assert_domain_has_no_forbidden_dependencies("data", &forbidden);
}

#[test]
fn domain_dependencies_stay_layered() {
    assert_domain_has_no_forbidden_dependencies(
        "core",
        &[
            "crate::native",
            "crate::draw",
            "crate::ui",
            "crate::app",
            "crate::data",
            "super::super::native",
            "super::super::draw",
            "super::super::ui",
            "super::super::app",
            "super::super::data",
        ],
    );
    assert_domain_has_no_forbidden_dependencies(
        "native",
        &[
            "crate::draw",
            "crate::ui",
            "crate::app",
            "crate::data",
            "super::super::draw",
            "super::super::ui",
            "super::super::app",
            "super::super::data",
        ],
    );
    assert_domain_has_no_forbidden_dependencies(
        "draw",
        &[
            "crate::ui",
            "crate::app",
            "crate::data",
            "super::super::ui",
            "super::super::app",
            "super::super::data",
        ],
    );
    assert_domain_has_no_forbidden_dependencies(
        "ui",
        &[
            "crate::app",
            "crate::data",
            "super::super::app",
            "super::super::data",
        ],
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
        "reset_dirty",
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

#[test]
fn active_work_registry_stays_internal() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let checked = ["prelude.rs", "lib.rs", "app/mod.rs"];
    let mut violations = Vec::new();

    for rel in checked {
        let text = fs::read_to_string(src.join(rel)).unwrap();
        if text.contains("ActiveWorkRegistry") {
            violations.push(format!("{rel} exposes ActiveWorkRegistry"));
        }
    }

    assert!(
        violations.is_empty(),
        "ActiveWorkRegistry is framework-managed and must not be exposed to app code: {violations:?}"
    );
}

#[test]
fn graphics_backend_selection_stays_off_prelude_until_public_api_lands() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let prelude = fs::read_to_string(src.join("prelude.rs")).unwrap();
    let forbidden = ["GraphicsBackend", "create_gpu_context_with_backend"];
    let violations: Vec<_> = forbidden
        .into_iter()
        .filter(|term| prelude.contains(term))
        .collect();

    assert!(
        violations.is_empty(),
        "P6.1 graphics backend selection is diagnostic/native-only until P6.5 public API lands: {violations:?}"
    );
}

#[test]
fn low_level_widget_loop_stays_off_prelude() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let prelude = fs::read_to_string(src.join("prelude.rs")).unwrap();

    assert!(
        !prelude.contains("run_widget_loop"),
        "prelude should route apps through App/View APIs, not the low-level WidgetTree loop"
    );
}

#[test]
fn internal_widget_tree_types_stay_off_user_entrypoints() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let checked = ["prelude.rs", "ui/mod.rs"];
    let internal_types = ["WidgetTree", "WidgetNode", "BoxedWidget"];
    let mut violations = Vec::new();

    for rel in checked {
        let text = fs::read_to_string(src.join(rel)).unwrap();
        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if !(trimmed.starts_with("pub use") || trimmed.contains("assert_exported::<")) {
                continue;
            }
            for symbol in internal_types {
                if trimmed
                    .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .any(|token| token == symbol)
                {
                    violations.push(format!("{rel}:{} exposes {symbol}", idx + 1));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "WidgetTree/WidgetNode/BoxedWidget are internal view adapter details, not user entrypoints: {violations:?}"
    );
}

#[test]
fn widget_id_stays_off_public_widget_boundary_signatures() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let checked = [
        "ui/core/widget/mod.rs",
        "ui/core/widget/tree_core.rs",
        "ui/core/widget/tree_dirty.rs",
        "ui/core/widget/tree_events.rs",
        "ui/core/widget/tree_layout.rs",
    ];
    let mut violations = Vec::new();

    for rel in checked {
        let text = fs::read_to_string(src.join(rel)).unwrap();
        if text.contains("pub type WidgetId") {
            violations.push(format!("{rel} exposes WidgetId as a public alias"));
        }
        let mut in_public_fn = false;
        let mut in_widget_core = false;
        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("pub trait WidgetCore") {
                in_widget_core = true;
            }
            if trimmed.starts_with("pub fn ") {
                in_public_fn = true;
            }

            if (in_public_fn || in_widget_core) && trimmed.contains("WidgetId") {
                violations.push(format!("{rel}:{} exposes WidgetId in {trimmed}", idx + 1));
            }

            if in_public_fn && trimmed.contains('{') {
                in_public_fn = false;
            }
            if in_widget_core && trimmed == "}" {
                in_widget_core = false;
            }
        }
    }

    assert!(
        violations.is_empty(),
        "public widget boundaries use ComponentId; WidgetId stays an internal WidgetTree alias: {violations:?}"
    );
}

#[test]
fn widget_core_public_boundary_uses_invalidation_not_dirty_bit() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let text = fs::read_to_string(src.join("ui/core/widget/mod.rs")).unwrap();
    let mut in_widget_core = false;
    let mut violations = Vec::new();

    for (idx, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("pub trait WidgetCore") {
            in_widget_core = true;
        }
        if in_widget_core
            && (trimmed.starts_with("fn dirty(") || trimmed.starts_with("fn set_dirty("))
        {
            violations.push(format!("ui/core/widget/mod.rs:{} has {trimmed}", idx + 1));
        }
        if in_widget_core && trimmed == "}" {
            in_widget_core = false;
        }
    }

    assert!(
        violations.is_empty(),
        "WidgetCore must expose invalidation/paint behavior through WidgetTree, not a dirty bit: {violations:?}"
    );
}

#[test]
fn state_public_boundary_uses_reconcile_invalidation_not_dirty_callback() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let text = fs::read_to_string(src.join("ui/foundation/state.rs")).unwrap();
    let forbidden = [
        "set_dirty_fn",
        "set_current_view_dirty_fn",
        "clear_current_view_dirty_fn",
        "CURRENT_VIEW_DIRTY_FN",
    ];
    let violations: Vec<_> = forbidden
        .into_iter()
        .filter(|term| text.contains(term))
        .collect();

    assert!(
        violations.is_empty(),
        "State must expose reconcile invalidation naming instead of dirty callbacks: {violations:?}"
    );
}

#[test]
fn demo_default_entrypoint_stays_on_prelude_app_path() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main = fs::read_to_string(root.join("demo/src/main.rs")).unwrap();

    assert!(
        main.contains("gui::run()") && main.contains("--cli"),
        "demo main should dispatch GUI (default) vs --cli only"
    );
    assert!(
        !main.contains("--dashboard"),
        "demo should not expose legacy --dashboard alias"
    );
}

#[test]
fn component_patch_downcast_path_has_no_hard_assertions() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let text = fs::read_to_string(src.join("ui/component_patch.rs")).unwrap();
    let forbidden = [".expect(", ".unwrap("];
    let violations: Vec<_> = forbidden
        .into_iter()
        .filter(|term| text.contains(term))
        .collect();

    assert!(
        violations.is_empty(),
        "component patching is a hot reconcile path and must not hard-assert downcasts: {violations:?}"
    );
}

#[test]
fn business_event_callbacks_stay_out_of_widget_fields() {
    let widgets = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/widgets");
    let mut violations = Vec::new();

    for file in rust_files_under(&widgets) {
        let rel = relative_src_path(&file);
        let text = fs::read_to_string(&file).unwrap();
        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("on_") && trimmed.contains(':') && !trimmed.contains("=>") {
                violations.push(format!("{rel}:{} contains {trimmed}", idx + 1));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "business event handlers belong in HandlerTable/View bindings, not widget struct fields: {violations:?}"
    );
}

#[test]
fn main_md_remains_the_architecture_document() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let forbidden = ["ARCHITECTURE.md", "architecture.md"];
    let present: Vec<_> = forbidden
        .iter()
        .copied()
        .filter(|name| root.join(name).exists())
        .collect();

    assert!(
        present.is_empty(),
        "docs/Main.md is the project architecture document; do not add standalone architecture files: {present:?}"
    );
}
