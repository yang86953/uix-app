use super::support::*;
use std::fs;
use std::path::Path;

#[test]
fn platform_cfgs_stay_inside_native_boundary() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut violations = Vec::new();

    for file in rust_files_under(&src) {
        let rel = relative_src_path(&file);
        let text = fs::read_to_string(&file).unwrap();
        let lines = platform_cfg_attribute_lines(&text);
        if !lines.is_empty() && !is_platform_cfg_boundary(&rel) {
            violations.push(format!(
                "{rel}:{}",
                lines
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "platform cfgs must stay in native/factory/**, native/backends/**, native/agent_transport/**, or native/graphics/**/platform/**: {violations:?}"
    );
}

#[test]
fn architecture_scanner_ignores_comments_and_cfg_feature_values() {
    let source = r###"
// crate::app and #[cfg(windows)] are documentation only.
/* nested /* crate::draw */ comments are ignored too. */
const EXAMPLE: &str = "#[cfg(unix)]";
#[cfg(feature = "windows")]
fn feature_named_windows() {}
#[cfg(
    target_os = "windows"
)]
fn windows_only() {}
"###;

    let code = rust_code_without_comments(source);
    assert!(!code.contains("crate::app"));
    assert!(!code.contains("crate::draw"));
    assert_eq!(platform_cfg_attribute_lines(source), vec![7]);
}

#[test]
fn domain_dependency_scanner_ignores_string_literals() {
    let source = r###"
const NORMAL: &str = "crate::app";
const RAW: &str = r#"crate::draw"#;
const BYTE: &[u8] = b"crate::ui";
const RAW_BYTE: &[u8] = br#"crate::native"#;
"###;

    let code = rust_code_without_comments_or_strings(source);
    assert_eq!(code.len(), source.len());
    assert_eq!(code.matches('\n').count(), source.matches('\n').count());
    for forbidden in ["crate::app", "crate::draw", "crate::ui", "crate::native"] {
        assert!(!code.contains(forbidden));
    }
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
        "crate::crate::native",
        "crate::crate::draw",
        "crate::crate::ui",
        "crate::crate::app",
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
            "crate::crate::native",
            "crate::crate::draw",
            "crate::crate::ui",
            "crate::crate::app",
            "crate::crate::data",
        ],
    );
    assert_domain_has_no_forbidden_dependencies(
        "native",
        &[
            "crate::draw",
            "crate::ui",
            "crate::app",
            "crate::data",
            "crate::crate::draw",
            "crate::crate::ui",
            "crate::crate::app",
            "crate::crate::data",
        ],
    );
    assert_domain_has_no_forbidden_dependencies(
        "draw",
        &[
            "crate::ui",
            "crate::app",
            "crate::data",
            "crate::crate::ui",
            "crate::crate::app",
            "crate::crate::data",
        ],
    );
    assert_domain_has_no_forbidden_dependencies(
        "ui",
        &[
            "crate::app",
            "crate::data",
            "crate::crate::app",
            "crate::crate::data",
        ],
    );
}

#[test]
fn draw_keeps_api_graphics_objects_inside_native() {
    assert_domain_has_no_forbidden_dependencies(
        "draw",
        &[
            "glow::",
            "NativeOpenGlRuntime",
            "NativeGraphicsRuntime",
            "acquire_native_runtime",
        ],
    );
}

#[test]
fn egl_native_pipeline_does_not_claim_an_unimplemented_gles2_fallback() {
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native/graphics/opengl/platform/egl.rs"),
    )
    .expect("EGL context source");

    assert!(source.contains("OPENGL_ES3_BIT"));
    assert!(source.contains("CONTEXT_MAJOR_VERSION"));
    assert!(
        !source.contains("OPENGL_ES2_BIT") && !source.contains("CONTEXT_CLIENT_VERSION, 2"),
        "the native raster pipeline uses #version 300 es shaders, so GLES2 must fail initialization rather than be advertised as a fallback"
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
