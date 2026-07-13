use std::fs;
use std::path::{Path, PathBuf};

fn read_source(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path)
        .expect("read source")
        .replace("\r\n", "\n")
}

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
    path == "native/factory.rs"
        || path.starts_with("native/factory/")
        || path.starts_with("native/backends/")
        || path.starts_with("native/graphics/")
}

fn is_platform_cfg_boundary(path: &str) -> bool {
    path == "native/factory.rs"
        || path.starts_with("native/factory/")
        || path.starts_with("native/backends/")
        || (path.starts_with("native/graphics/")
            && path.split('/').any(|component| component == "platform"))
}

fn is_architecture_guard(path: &str) -> bool {
    path == "tests/architecture/boundaries.rs"
}

#[test]
fn metal_identity_stays_named_as_cpu_pixel_upload() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let context = fs::read_to_string(src.join("native/graphics/metal/platform/context.rs"))
        .expect("read Metal PixelUpload context");
    let registry = fs::read_to_string(src.join("native/factory/registry_macos.rs"))
        .expect("read macOS graphics registry");

    assert!(
        context.contains("pub struct MetalPixelUploadContext"),
        "the CPU PixelUpload implementation must not be named like a Metal native raster context"
    );
    assert!(
        context.contains("GraphicsContextCaps::cpu_pixel_upload"),
        "Metal identity must retain CPU PixelUpload caps"
    );
    assert!(
        registry.contains("raster: RasterMode::Cpu")
            && registry.contains("present: PresentMode::PixelUpload"),
        "macOS Metal identity recipe must remain Cpu × PixelUpload"
    );
    assert!(
        !registry.contains("raster: RasterMode::GpuNative"),
        "Metal identity must not claim unimplemented native raster"
    );
}

#[test]
fn raw_gpu_factory_creation_stays_inside_the_native_factory_bridge() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let native_mod = fs::read_to_string(src.join("native/mod.rs")).expect("read native module");
    let factory =
        fs::read_to_string(src.join("native/factory/mod.rs")).expect("read native factory module");
    let registry =
        fs::read_to_string(src.join("native/factory/registry.rs")).expect("read graphics registry");

    assert!(
        !native_mod.contains("create_gpu_context_with_backend"),
        "the public native module must not re-export raw surface context creation"
    );
    assert!(
        factory.contains("pub(crate) fn create_gpu_context_with_backend"),
        "the explicit raw-surface factory helper is only for crate-local test and native wiring"
    );
    assert!(
        registry.contains("pub(crate) fn try_create_context")
            && registry.contains("pub(crate) fn try_create_gpu_context"),
        "registry raw-surface constructors must stay inside the factory bridge"
    );
    for relative in [
        "native/graphics/d3d11/platform/context.rs",
        "native/graphics/d3d11/platform/pipeline.rs",
        "native/graphics/d3d12/platform/context.rs",
        "native/graphics/metal/platform/context.rs",
        "native/graphics/vulkan/platform/context.rs",
        "native/graphics/opengl/platform/wgl.rs",
        "native/graphics/opengl/platform/egl.rs",
    ] {
        let source = fs::read_to_string(src.join(relative)).expect("read native API object source");
        assert!(
            !source.contains("pub fn new("),
            "{relative} must not expose a raw native API constructor outside the crate"
        );
    }
}

#[test]
fn windows_graphics_contexts_share_the_drawable_dpr_contract() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native/graphics");
    let platform = fs::read_to_string(src.join("platform/windows.rs"))
        .expect("read shared Windows graphics platform helper");
    let d3d11 = fs::read_to_string(src.join("d3d11/platform/context.rs"))
        .expect("read D3D11 graphics context");
    let d3d12 = fs::read_to_string(src.join("d3d12/platform/context.rs"))
        .expect("read D3D12 graphics context");
    let wgl =
        fs::read_to_string(src.join("opengl/platform/wgl.rs")).expect("read WGL graphics context");

    assert!(
        platform.contains("pub(crate) struct DrawableSize")
            && platform.contains("pub(crate) fn drawable_size")
            && platform.contains("pub(crate) fn drawable_size_from_hdc"),
        "Windows graphics APIs must share one logical-to-drawable DPR calculation"
    );
    for (name, context) in [("D3D11", d3d11), ("D3D12", d3d12)] {
        assert!(
            context.contains("win_surface::drawable_size")
                && context.contains("self.device_pixel_ratio()")
                && context.contains("fn device_pixel_ratio(&self) -> f32"),
            "{name} must create, resize, and report caps from the shared drawable DPR"
        );
    }
    assert!(
        wgl.contains("drawable_size_from_hdc") && !wgl.contains("fn drawable_size("),
        "WGL must not retain a divergent local DPI calculation"
    );
}

fn rust_code_without_comments(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'r' {
            let mut quote = index + 1;
            while quote < bytes.len() && bytes[quote] == b'#' {
                quote += 1;
            }
            if quote < bytes.len() && bytes[quote] == b'"' {
                let hashes = quote - index - 1;
                output.extend_from_slice(&bytes[index..=quote]);
                index = quote + 1;
                while index < bytes.len() {
                    output.push(bytes[index]);
                    if bytes[index] == b'"'
                        && index + hashes < bytes.len()
                        && (hashes == 0
                            || bytes[index + 1..=index + hashes]
                                .iter()
                                .all(|byte| *byte == b'#'))
                    {
                        if hashes > 0 {
                            output.extend_from_slice(&bytes[index + 1..=index + hashes]);
                        }
                        index += hashes + 1;
                        break;
                    }
                    index += 1;
                }
                continue;
            }
        }

        if bytes[index] == b'"' {
            output.push(bytes[index]);
            index += 1;
            while index < bytes.len() {
                output.push(bytes[index]);
                if bytes[index] == b'\\' && index + 1 < bytes.len() {
                    index += 1;
                    output.push(bytes[index]);
                } else if bytes[index] == b'"' {
                    index += 1;
                    break;
                }
                index += 1;
            }
            continue;
        }

        if bytes[index..].starts_with(b"//") {
            while index < bytes.len() && bytes[index] != b'\n' {
                output.push(b' ');
                index += 1;
            }
            continue;
        }

        if bytes[index..].starts_with(b"/*") {
            let mut depth = 1;
            output.extend_from_slice(b"  ");
            index += 2;
            while index < bytes.len() && depth > 0 {
                if bytes[index..].starts_with(b"/*") {
                    depth += 1;
                    output.extend_from_slice(b"  ");
                    index += 2;
                } else if bytes[index..].starts_with(b"*/") {
                    depth -= 1;
                    output.extend_from_slice(b"  ");
                    index += 2;
                } else {
                    output.push(if bytes[index] == b'\n' { b'\n' } else { b' ' });
                    index += 1;
                }
            }
            continue;
        }

        output.push(bytes[index]);
        index += 1;
    }

    String::from_utf8(output).expect("comment stripping must preserve UTF-8 source")
}

fn skip_string(bytes: &[u8], index: &mut usize) -> bool {
    if bytes.get(*index) == Some(&b'r') {
        let mut quote = *index + 1;
        while quote < bytes.len() && bytes[quote] == b'#' {
            quote += 1;
        }
        if quote < bytes.len() && bytes[quote] == b'"' {
            let hashes = quote - *index - 1;
            *index = quote + 1;
            while *index < bytes.len() {
                if bytes[*index] == b'"'
                    && *index + hashes < bytes.len()
                    && (hashes == 0
                        || bytes[*index + 1..=*index + hashes]
                            .iter()
                            .all(|byte| *byte == b'#'))
                {
                    *index += hashes + 1;
                    return true;
                }
                *index += 1;
            }
            return true;
        }
    }

    if bytes.get(*index) != Some(&b'"') {
        return false;
    }
    *index += 1;
    while *index < bytes.len() {
        if bytes[*index] == b'\\' && *index + 1 < bytes.len() {
            *index += 2;
        } else if bytes[*index] == b'"' {
            *index += 1;
            break;
        } else {
            *index += 1;
        }
    }
    true
}

fn rust_code_without_comments_or_strings(text: &str) -> String {
    let code = rust_code_without_comments(text);
    let bytes = code.as_bytes();
    let mut output = bytes.to_vec();
    let mut index = 0;

    while index < bytes.len() {
        let string_start = index;
        if skip_string(bytes, &mut index) {
            for byte in &mut output[string_start..index] {
                if *byte != b'\n' {
                    *byte = b' ';
                }
            }
        } else {
            index += 1;
        }
    }

    String::from_utf8(output).expect("source masking must preserve UTF-8 source")
}

fn cfg_attribute_mentions_platform(attribute: &str) -> bool {
    let bytes = attribute.as_bytes();
    let mut identifiers = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        if skip_string(bytes, &mut index) {
            continue;
        }
        if bytes[index].is_ascii_alphabetic() || bytes[index] == b'_' {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
            {
                index += 1;
            }
            identifiers.push(&attribute[start..index]);
        } else {
            index += 1;
        }
    }

    matches!(identifiers.first(), Some(&"cfg") | Some(&"cfg_attr"))
        && identifiers.iter().skip(1).any(|identifier| {
            matches!(
                *identifier,
                "windows" | "unix" | "target_os" | "target_family"
            )
        })
}

fn platform_cfg_attribute_lines(text: &str) -> Vec<usize> {
    let code = rust_code_without_comments(text);
    let bytes = code.as_bytes();
    let mut lines = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        if skip_string(bytes, &mut index) {
            continue;
        }
        if bytes[index] != b'#' {
            index += 1;
            continue;
        }

        let attribute_offset = index;
        index += 1;
        if bytes.get(index) == Some(&b'!') {
            index += 1;
        }
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
        if bytes.get(index) != Some(&b'[') {
            continue;
        }

        let attribute_start = index + 1;
        let mut depth = 1;
        index += 1;
        while index < bytes.len() && depth > 0 {
            if skip_string(bytes, &mut index) {
                continue;
            }
            match bytes[index] {
                b'[' => depth += 1,
                b']' => depth -= 1,
                _ => {}
            }
            index += 1;
        }

        if depth == 0
            && cfg_attribute_mentions_platform(&code[attribute_start..index.saturating_sub(1)])
        {
            lines.push(
                code[..attribute_offset]
                    .bytes()
                    .filter(|byte| *byte == b'\n')
                    .count()
                    + 1,
            );
        }
    }

    lines
}

fn assert_domain_has_no_forbidden_dependencies(domain: &str, forbidden: &[&str]) {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let root = src.join(domain);
    let mut violations = Vec::new();

    for file in rust_files_under(&root) {
        let text = rust_code_without_comments_or_strings(&fs::read_to_string(&file).unwrap());
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
        "platform cfgs must stay in native/factory/**, native/backends/**, or native/graphics/**/platform/**: {violations:?}"
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
fn graphics_backend_public_api_exposes_enum_not_context_factory() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let prelude = fs::read_to_string(src.join("prelude.rs")).unwrap();

    assert!(
        prelude.contains("GraphicsBackend"),
        "P6.5 exposes GraphicsBackend so App::graphics_backend can be used from prelude"
    );
    assert!(
        !prelude.contains("create_gpu_context_with_backend"),
        "low-level native GPU context creation stays out of prelude"
    );
}

#[test]
fn canvas_scroll_copy_requires_an_explicit_backend_semantic() {
    let canvas =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/draw/traits/canvas.rs"))
            .expect("read Canvas2D contract");

    assert!(
        canvas.contains("fn scroll_region(&mut self, viewport: Rect, dx: f32, dy: f32);"),
        "Canvas2D scroll copy must not regain a silent no-op default"
    );
}

#[test]
fn d3d12_checked_shutdown_cannot_fall_back_to_the_void_hook() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native/graphics/d3d12/platform/context.rs"),
    );

    assert!(
        source.contains(
            "fn try_shutdown(&mut self) -> Result<()> {\n        self.shutdown_result()\n    }"
        ),
        "D3D12 checked shutdown must return shutdown_result instead of the legacy void hook"
    );
}

#[test]
fn wgl_checked_shutdown_cannot_fall_back_to_the_void_hook() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native/graphics/opengl/platform/wgl.rs"),
    );

    assert!(
        source.contains(
            "fn try_shutdown(&mut self) -> Result<(), Error> {\n        self.shutdown_result()\n    }"
        ),
        "WGL checked shutdown must return shutdown_result instead of the legacy void hook"
    );
}

#[test]
fn egl_checked_shutdown_cannot_fall_back_to_the_void_hook() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native/graphics/opengl/platform/egl.rs"),
    );

    assert!(
        source.contains("fn try_shutdown(&mut self) -> Result<(), Error> {\n        self.shutdown_result()\n    }"),
        "EGL checked shutdown must return shutdown_result instead of the legacy void hook"
    );
}

#[test]
fn vulkan_checked_shutdown_cannot_fall_back_to_the_void_hook() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/native/graphics/vulkan/platform/context.rs"),
    );

    assert!(
        source.contains(
            "fn try_shutdown(&mut self) -> Result<()> {\n        self.shutdown_result()\n    }"
        ),
        "Vulkan checked shutdown must return shutdown_result instead of the legacy void hook"
    );
}

#[test]
fn d3d11_checked_shutdown_cannot_fall_back_to_the_void_hook() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native/graphics/d3d11/platform/context.rs"),
    );

    assert!(
        source.contains(
            "fn try_shutdown(&mut self) -> Result<()> {\n        self.shutdown_result()\n    }"
        ),
        "D3D11 checked shutdown must return shutdown_result instead of the legacy void hook"
    );
}

#[test]
fn metal_checked_shutdown_cannot_fall_back_to_the_void_hook() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native/graphics/metal/platform/context.rs"),
    );

    assert!(
        source.contains(
            "fn try_shutdown(&mut self) -> Result<()> {\n        self.shutdown_result()\n    }"
        ),
        "Metal checked shutdown must return shutdown_result instead of the legacy void hook"
    );
}

#[test]
fn igraphics_context_has_no_legacy_void_shutdown() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native/traits/present.rs"),
    );

    assert!(
        source.contains("fn try_shutdown(&mut self) -> Result<(), Error>;"),
        "IGraphicsContext must require checked try_shutdown"
    );
    assert!(
        !source.contains("fn shutdown(&mut self);"),
        "IGraphicsContext must not keep a legacy void shutdown hook"
    );
    assert!(
        !source.contains("fn try_shutdown(&mut self) -> Result<(), Error> {\n        self.shutdown();"),
        "try_shutdown must not default to a void shutdown adapter"
    );
}

#[test]
fn graphics_engine_has_no_legacy_void_shutdown() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/draw/traits/engine.rs"),
    );

    assert!(
        source.contains("fn try_shutdown(&mut self) -> Result<(), Error>;"),
        "GraphicsEngine must require checked try_shutdown"
    );
    assert!(
        !source.contains("fn shutdown(&mut self);"),
        "GraphicsEngine must not keep a legacy void shutdown hook"
    );
}

#[test]
fn render_backend_has_no_legacy_void_shutdown() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/draw/backend/traits.rs"),
    );

    assert!(
        source.contains("fn try_shutdown(&mut self) -> Result<(), Error>;"),
        "RenderBackend must require checked try_shutdown"
    );
    assert!(
        !source.contains("fn shutdown(&mut self);"),
        "RenderBackend must not keep a legacy void shutdown hook"
    );
}

#[test]
fn thread_bound_drop_guards_foreign_thread_teardown() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native/factory/thread_bound.rs"),
    );

    assert!(
        source.contains("impl Drop for ThreadBoundGraphicsContext"),
        "thread-bound contexts must own Drop so foreign-thread teardown stays typed"
    );
    assert!(
        source.contains("std::mem::forget(inner)"),
        "wrong-thread Drop must leak the native context instead of calling its Drop"
    );
}

#[test]
fn vulkan_readback_cannot_report_an_empty_success() {
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/native/graphics/vulkan/platform/context.rs"),
    )
    .expect("read Vulkan context");

    assert!(
        source.contains("VulkanContext: native readback is not supported"),
        "Vulkan must return a typed readback failure instead of an empty pixel buffer"
    );
}

#[test]
fn metal_pixel_upload_readback_cannot_report_an_empty_success() {
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native/graphics/metal/platform/context.rs"),
    )
    .expect("read Metal PixelUpload context");

    assert!(
        source.contains("MetalPixelUploadContext: native readback is not supported"),
        "Metal PixelUpload must return a typed readback failure instead of an empty pixel buffer"
    );
}

#[test]
fn d3d11_checked_offscreen_destroy_cannot_ignore_swapchain_restore_failure() {
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native/graphics/d3d11/platform/context.rs"),
    )
    .expect("read D3D11 context");

    assert!(
        source.contains("fn try_destroy_offscreen_target(&mut self, id: OffscreenTargetId) -> Result<(), Error>"),
        "D3D11 checked offscreen destroy must not fall back to the void hook"
    );
    assert!(
        source.contains("self.bind_swapchain_target()?;"),
        "D3D11 checked offscreen destroy must propagate the swapchain restore failure"
    );
}

#[test]
fn replace_upload_is_declared_on_destination_dependent_backends() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native/graphics");
    let d3d12 = fs::read_to_string(src.join("d3d12/platform/context.rs")).expect("D3D12");
    let wgl = fs::read_to_string(src.join("opengl/platform/wgl.rs")).expect("WGL");
    let egl = fs::read_to_string(src.join("opengl/platform/egl.rs")).expect("EGL");
    let vulkan =
        fs::read_to_string(src.join("vulkan/platform/context.rs")).expect("Vulkan");

    for (name, source) in [
        ("D3D12", d3d12.as_str()),
        ("WGL", wgl.as_str()),
        ("EGL", egl.as_str()),
        ("Vulkan", vulkan.as_str()),
    ] {
        assert!(
            source.contains("fn upload_surface_pixels("),
            "{name} must declare upload_surface_pixels for Additive/Scroll replace upload"
        );
    }
}

#[test]
fn pixel_upload_contexts_cannot_report_swapchain_present_success() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native/graphics");
    let vulkan =
        fs::read_to_string(src.join("vulkan/platform/context.rs")).expect("read Vulkan context");
    let metal = fs::read_to_string(src.join("metal/platform/context.rs"))
        .expect("read Metal PixelUpload context");

    assert!(
        vulkan.contains(
            "VulkanContext: swapchain present is not supported for the PixelUpload recipe"
        ),
        "Vulkan PixelUpload must reject the legacy swapchain present hook"
    );
    assert!(
        metal.contains("MetalPixelUploadContext: swapchain present requires native Metal raster"),
        "Metal PixelUpload must reject the legacy swapchain present hook"
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
fn architecture_document_stays_in_the_qualified_docs_tree() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let architecture_doc = root.join("docs/架构.md");
    let domain_dir = root.join("docs/领域");
    let forbidden = ["ARCHITECTURE.md", "architecture.md"];
    let present: Vec<_> = forbidden
        .iter()
        .copied()
        .filter(|name| root.join(name).exists())
        .collect();

    let required_generic = ["产品.md", "架构.md", "进度.md", "使用.md"];
    for name in required_generic {
        let path = root.join("docs").join(name);
        assert!(
            path.is_file(),
            "generic docs skeleton missing: {}",
            path.display()
        );
    }

    assert!(
        architecture_doc.is_file(),
        "qualified architecture navigation is missing: {}",
        architecture_doc.display()
    );
    assert!(
        !domain_dir.exists(),
        "docs/领域/ must not exist; project depth lives in 产品/架构/进度/使用: {}",
        domain_dir.display()
    );

    let forbidden_root_domain = [
        "按需驱动.md",
        "公开API.md",
        "ui.md",
        "界面.md",
        "运行时.md",
        "渲染.md",
        "缺陷.md",
    ];
    let leaked: Vec<_> = forbidden_root_domain
        .iter()
        .copied()
        .filter(|name| root.join("docs").join(name).exists())
        .collect();
    assert!(
        leaked.is_empty(),
        "removed domain encyclopedias must not reappear under docs/: {leaked:?}"
    );

    assert!(
        root.join("AGENTS.md").is_file(),
        "AGENTS.md AI guidance contract is missing"
    );

    assert!(
        present.is_empty(),
        "docs/架构.md is the project architecture navigation; do not add standalone root architecture files: {present:?}"
    );
}
