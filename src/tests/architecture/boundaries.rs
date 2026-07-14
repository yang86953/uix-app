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
    let path = path.strip_prefix("tests/").unwrap_or(path);
    path == "native/factory.rs"
        || path.starts_with("native/factory/")
        || path.starts_with("native/backends/")
        || path.starts_with("native/graphics/")
}

fn is_platform_cfg_boundary(path: &str) -> bool {
    let path = path.strip_prefix("tests/").unwrap_or(path);
    path == "native/factory.rs"
        || path.starts_with("native/factory/")
        || path.starts_with("native/backends/")
        || path.starts_with("native/agent_transport/")
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
fn vulkan_pixel_upload_is_active_on_all_desktop_registries() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let context = fs::read_to_string(src.join("native/graphics/vulkan/platform/context.rs"))
        .expect("read Vulkan PixelUpload context");
    let platform_mod = fs::read_to_string(src.join("native/graphics/vulkan/platform/mod.rs"))
        .expect("read Vulkan platform module");
    let macos_window = fs::read_to_string(src.join("native/backends/macos/platform.rs"))
        .expect("read macOS window platform");

    for relative in [
        "native/factory/registry_windows.rs",
        "native/factory/registry_linux.rs",
        "native/factory/registry_macos.rs",
    ] {
        let registry = fs::read_to_string(src.join(relative)).expect("read graphics registry");
        assert!(
            registry.contains("id: GraphicsBackend::Vulkan")
                && registry.contains("BackendStatus::Active")
                && registry.contains("RasterMode::Cpu")
                && registry.contains("PresentMode::PixelUpload"),
            "{relative} must declare Vulkan as Active Cpu × PixelUpload"
        );
        assert!(
            !registry.contains("BackendStatus::Planned"),
            "{relative} must not leave Vulkan as a Planned stub"
        );
    }

    assert!(
        context.contains("create_win32_surface")
            && context.contains("create_wayland_surface")
            && context.contains("create_metal_surface")
            && context.contains("portability_enumeration")
            && context.contains("portability_subset")
            && context.contains("CAMetalLayer")
            && context.contains("cpu_shadow")
            && context.contains("hydrate_cpu_shadow_from_staging")
            && context.contains("fn upload_surface_pixels(")
            && context.contains("fn shutdown_result("),
        "Vulkan PixelUpload must cover Win32/Wayland/MoltenVK WSI, CPU-shadow readback, replace upload, and checked shutdown"
    );
    assert!(
        !platform_mod.contains("portability adapter is not implemented on macOS"),
        "macOS Vulkan must not remain a Planned stub"
    );
    assert!(
        macos_window.contains("CAMetalLayer")
            && macos_window.contains("setFramebufferOnly:")
            && macos_window.contains("setDrawableSize:"),
        "macOS window surface must expose CAMetalLayer for MoltenVK WSI"
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

#[test]
fn partial_present_requires_typed_coherency_and_damage_conversion() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let present = read_source(src.join("native/traits/present.rs"));
    let damage = read_source(src.join("core/damage.rs"));
    let gdi = read_source(src.join("native/backends/windows/gdi_presenter.rs"));

    assert!(
        present.contains("pub present_coherency: PresentCoherency")
            && !present.contains("pub partial_present: bool"),
        "native caps must use typed coherency instead of a partial-present boolean"
    );
    assert!(
        damage.contains("pub struct PresentDamageTracker")
            && damage.contains("min(tx1).floor()")
            && damage.contains("max(tx1).ceil()")
            && damage.contains("PresentCoherency::TrackedSwapchain"),
        "logical damage conversion must expand edges and retain swapchain image history"
    );
    assert!(
        gdi.contains("PresentCoherency::RetainedBuffer"),
        "only the retained GDI DIB path may currently advertise narrow external presents"
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
fn native_contexts_use_checked_shutdown() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cases = [
        (
            "D3D12",
            "src/native/graphics/d3d12/platform/context.rs",
            "fn try_shutdown(&mut self) -> Result<()> {\n        self.shutdown_result()\n    }",
        ),
        (
            "WGL",
            "src/native/graphics/opengl/platform/wgl.rs",
            "fn try_shutdown(&mut self) -> Result<(), Error> {\n        self.shutdown_result()\n    }",
        ),
        (
            "EGL",
            "src/native/graphics/opengl/platform/egl.rs",
            "fn try_shutdown(&mut self) -> Result<(), Error> {\n        self.shutdown_result()\n    }",
        ),
        (
            "Vulkan",
            "src/native/graphics/vulkan/platform/context.rs",
            "fn try_shutdown(&mut self) -> Result<()> {\n        self.shutdown_result()\n    }",
        ),
        (
            "D3D11",
            "src/native/graphics/d3d11/platform/context.rs",
            "fn try_shutdown(&mut self) -> Result<()> {\n        self.shutdown_result()\n    }",
        ),
        (
            "Metal",
            "src/native/graphics/metal/platform/context.rs",
            "fn try_shutdown(&mut self) -> Result<()> {\n        self.shutdown_result()\n    }",
        ),
    ];

    for (backend, path, checked_shutdown) in cases {
        let source = read_source(root.join(path));
        assert!(
            source.contains(checked_shutdown),
            "{backend} checked shutdown must return shutdown_result instead of a legacy void hook"
        );
    }
}

#[test]
fn graphics_contracts_expose_only_checked_shutdown() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (contract, path) in [
        ("IGraphicsContext", "src/native/traits/present.rs"),
        ("GraphicsEngine", "src/draw/traits/engine.rs"),
        ("RenderBackend", "src/draw/backend/traits.rs"),
    ] {
        let source = read_source(root.join(path));
        assert!(
            source.contains("fn try_shutdown(&mut self) -> Result<(), Error>;"),
            "{contract} must require checked try_shutdown"
        );
        assert!(
            !source.contains("fn shutdown(&mut self);"),
            "{contract} must not keep a legacy void shutdown hook"
        );
        assert!(
            !source.contains(
                "fn try_shutdown(&mut self) -> Result<(), Error> {\n        self.shutdown();"
            ),
            "{contract} must not adapt checked shutdown to a void hook"
        );
    }
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
fn application_defers_show_until_first_present() {
    let application = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app/shell/application.rs"),
    )
    .expect("read application");
    let event_loop = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app/event_loop/event_loop.rs"),
    )
    .expect("read event loop");
    let window_driver =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app/window_driver.rs"))
            .expect("read window driver");

    assert!(
        application.contains("show deferred")
            && application.contains("WindowDriver::new(width, height, true)")
            && !application.contains("initial window show failed"),
        "Application must not ShowWindow before fonts/session/first present"
    );
    assert!(
        event_loop.contains("!platform_window.is_visible()")
            && window_driver.contains("if frame_committed && self.deferred_show")
            && window_driver.contains("first_present_ms=")
            && window_driver.contains("deferred show after first present failed"),
        "the shared window driver must reveal each window only after a successful first present"
    );
}

#[test]
fn root_and_secondary_windows_share_one_frame_driver() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let event_loop = read_source(root.join("src/app/event_loop/event_loop.rs"));
    let application = read_source(root.join("src/app/shell/application.rs"));
    let window_driver = read_source(root.join("src/app/window_driver.rs"));

    assert!(
        window_driver.contains("pub(crate) struct WindowDriver")
            && window_driver.contains("pub(crate) fn drive_frame("),
        "the ordered per-window frame pipeline must have one owner"
    );
    assert!(
        event_loop.contains("driver.drive_frame(WindowFrameContext")
            && application.contains("driver.drive_frame(WindowFrameContext"),
        "root and secondary windows must call the same WindowDriver"
    );
    for duplicate in [
        "FrameRenderer::new()",
        "PresentDamageTracker::new()",
        "fn sync_secondary_animation_deadlines",
        "fn ensure_secondary_surface_matches_window",
    ] {
        assert!(
            !event_loop.contains(duplicate) && !application.contains(duplicate),
            "outer loops must not regain duplicated frame-driver logic: {duplicate}"
        );
    }
}

#[test]
fn window_driver_uses_one_shot_surface_scheduler() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scheduler = read_source(root.join("src/app/frame_scheduler.rs"));
    let driver = read_source(root.join("src/app/window_driver.rs"));
    let event_loop = read_source(root.join("src/app/event_loop/event_loop.rs"));

    assert!(
        scheduler.contains("pub(crate) enum SurfaceState")
            && scheduler.contains("generation: u64")
            && scheduler.contains("next_request_id: u64")
            && scheduler.contains("outstanding: Option<FrameRequest>")
            && scheduler.contains("pub(crate) fn notify_opportunity(")
            && scheduler.contains("request.token != token")
            && scheduler.contains("SurfaceState::Recovering"),
        "per-window frame scheduling must retain token-safe one-shot and recovery state"
    );
    assert!(
        driver.contains("frame_scheduler: FrameScheduler")
            && driver.contains("active_work.register_open(kind)")
            && driver.contains("self.frame_scheduler.frame_failed(failure, frame_time)")
            && driver.contains("request_native_frame(NativeFrameRequest::after_present(token))")
            && driver.contains("platform_window.native_frame_presented(token)")
            && driver.contains("if frame_committed")
            && driver.contains("cancel_outstanding_native_frame"),
        "WindowDriver must own frame opportunities, open animation registrations, and bounded recovery"
    );
    assert!(
        event_loop.contains("let window_deadline = driver.next_deadline("),
        "the root event loop must wait on the shared driver's one-shot deadline"
    );
    for forbidden in [
        "ANIMATION_FRAME_INTERVAL",
        "Duration::from_millis(16)",
        "register(kind, now +",
    ] {
        assert!(
            !driver.contains(forbidden),
            "WindowDriver must not regain fixed-interval animation polling: {forbidden}"
        );
    }
}

#[test]
fn wayland_frame_pacing_and_shm_backpressure_are_protocol_driven() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let window_ops = read_source(root.join("src/native/backends/linux/wayland/window_ops.rs"));
    let presenter = read_source(root.join("src/native/backends/linux/wayland/presenter.rs"));
    let buffer_lease = read_source(root.join("src/native/shared/buffer_lease.rs"));

    assert!(
        window_ops.contains("let callback = surface.frame()")
            && window_ops.contains("UiEvent::frame_opportunity(")
            && window_ops.contains("current.as_ref() == Some(&request)")
            && window_ops.contains("request.token == token"),
        "Wayland must deliver only the exact outstanding wl_surface.frame request"
    );
    assert!(
        !presenter.contains("surface.frame()")
            && presenter.contains("ShmBuffer::try_acquire")
            && presenter.contains("Errc::WouldBlock")
            && presenter.contains("wl_buffer::Event::Release")
            && buffer_lease.contains("compare_exchange(false, true"),
        "Wayland SHM presentation must reuse only compositor-released buffers and back off when both are busy"
    );
}

#[test]
fn windows_frame_pacing_waits_off_thread_and_delivers_exactly_once() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pacer = read_source(root.join("src/native/backends/windows/frame_pacer.rs"));
    let window_ops = read_source(root.join("src/native/backends/windows/window_ops.rs"));
    let wnd_proc = read_source(root.join("src/native/backends/windows/wnd_proc.rs"));

    assert!(
        pacer.contains("thread::Builder::new()")
            && pacer.contains("receiver.recv()")
            && pacer.contains("DwmFlush()")
            && pacer.contains(".matches_submitted(ticket)")
            && pacer.contains("WM_UIX_FRAME_OPPORTUNITY")
            && pacer.contains("PostMessageW("),
        "Windows must wait for DWM on a blocking worker and post one exact ticket back to Win32"
    );
    assert!(
        window_ops.contains("fn os_request_native_frame(")
            && window_ops.contains("self.frame_pacer.request(request)")
            && window_ops.contains("fn os_native_frame_presented(")
            && window_ops.contains("self.frame_pacer.presented(token)")
            && window_ops.contains("fn os_cancel_native_frame(")
            && window_ops.contains("self.frame_pacer.cancel(token)"),
        "each Windows window must own native frame request and cancellation"
    );
    assert!(
        pacer.contains("mark_submitted(token)")
            && pacer.contains("matches_submitted(ticket)")
            && pacer.contains("mpsc::sync_channel(1)"),
        "DWM waiting must start only after exact present confirmation and retain bounded backpressure"
    );
    assert!(
        wnd_proc.contains("complete_posted_frame(&binding.frame_pacer")
            && wnd_proc.contains("UiEvent::frame_opportunity(")
            && wnd_proc.contains("platform.push_event("),
        "the private Win32 message must become one WindowId-routed frame opportunity"
    );
    assert!(
        !window_ops.contains("DwmFlush") && !wnd_proc.contains("DwmFlush"),
        "DwmFlush must never block the UI thread"
    );
}

#[test]
fn macos_frame_pacing_is_lazy_exact_and_post_present() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pacer = read_source(root.join("src/native/backends/macos/display_link.rs"));
    let platform = read_source(root.join("src/native/backends/macos/platform.rs"));
    let mailbox = read_source(root.join("src/native/shared/native_frame_mailbox.rs"));

    assert!(
        pacer.contains("displayLinkWithTarget:selector:")
            && pacer.contains("respondsToSelector:")
            && pacer.contains("NSRunLoopCommonModes")
            && !pacer.contains("CVDisplayLink"),
        "macOS must use the modern window-bound display link with an availability fallback"
    );
    assert!(
        pacer.contains("NativeFrameRequestPhase::AfterPresent")
            && pacer.contains(".mark_submitted(token)")
            && pacer.contains("set_paused(self.display_link, false)")
            && pacer.contains("set_paused(display_link, true)")
            && pacer.contains("msg_void(display_link, \"invalidate\")"),
        "macOS display pacing must arm lazily, start only after exact present, and invalidate submitted cancellation"
    );
    assert!(
        pacer.contains(".take_submitted()")
            && pacer.contains("UiEvent::frame_opportunity(")
            && pacer.contains("CACurrentMediaTime()")
            && pacer.contains("CFRunLoopWakeUp(run_loop)"),
        "one native callback must deliver one exact WindowId-routed opportunity and wake the main run loop"
    );
    assert!(
        mailbox.contains("self.pending.is_some_and(|request| request.token == token)")
            && mailbox.contains("let replaced_submitted = self.submitted")
            && mailbox.contains("pub(crate) fn take_submitted("),
        "the shared native mailbox must preserve exact token and stale-source isolation"
    );
    assert!(
        platform.contains("self.frame_pacer.request(request)")
            && platform.contains("self.frame_pacer.presented(token)")
            && platform.contains("self.frame_pacer.cancel(token)"),
        "each macOS window must own native frame request, present confirmation, and cancellation"
    );
    let shutdown = platform
        .find("self.frame_pacer.shutdown()")
        .expect("MacosWindowOps must shut down its display link");
    let release = platform
        .find("cocoa::release_object(window)")
        .expect("MacosWindowOps must release its NSWindow");
    assert!(
        shutdown < release,
        "the display link must stop before its NSWindow owner is released"
    );
}

#[test]
fn hidden_windows_suspend_visual_work_until_their_exact_show_signal() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let events = read_source(root.join("src/native/traits/event/types.rs"));
    let scheduler = read_source(root.join("src/app/frame_scheduler.rs"));
    let driver = read_source(root.join("src/app/window_driver.rs"));
    let wnd_proc = read_source(root.join("src/native/backends/windows/wnd_proc.rs"));

    assert!(
        events.contains("WindowShow")
            && events.contains("WindowHide")
            && events.contains("pub fn window_show()")
            && events.contains("pub fn window_hide()"),
        "visibility lifecycle events must be platform-independent"
    );
    assert!(
        scheduler.contains("pub(crate) enum SurfaceSuspendReason")
            && scheduler.contains("    Hidden,")
            && scheduler.contains("pub(crate) fn suspended_reason("),
        "the scheduler must retain the exact suspension reason"
    );
    assert!(
        driver.contains("UiEventType::WindowHide")
            && driver.contains("UiEventType::WindowShow")
            && driver.contains("self.suspend_if_surface_unavailable(tree, platform_window)")
            && driver.contains("== Some(SurfaceSuspendReason::Hidden)"),
        "WindowDriver must block hidden visual work and resume only Hidden"
    );
    assert!(
        wnd_proc.contains("WM_SHOWWINDOW")
            && wnd_proc.contains("UiEvent::window_show()")
            && wnd_proc.contains("UiEvent::window_hide()")
            && wnd_proc.contains("self.push_event("),
        "Win32 visibility changes must update state and route to one WindowId"
    );
}

#[test]
fn d3d11_occlusion_is_a_per_window_idle_probe_not_graphics_recovery() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let present_traits = read_source(root.join("src/native/traits/present.rs"));
    let d3d11 = read_source(root.join("src/native/graphics/d3d11/platform/context.rs"));
    let d3d12 = read_source(root.join("src/native/graphics/d3d12/platform/context.rs"));
    let scheduler = read_source(root.join("src/app/frame_scheduler.rs"));
    let driver = read_source(root.join("src/app/window_driver.rs"));
    let recovering = read_source(root.join("src/draw/engine/recovering.rs"));

    assert!(
        present_traits.contains("pub enum PresentTestResult")
            && present_traits.contains("fn test_present(&mut self)")
            && present_traits.contains("Errc::NotImplemented"),
        "idle present probing must be an optional typed capability with zero caller maintenance"
    );
    assert!(
        d3d11.contains("DXGI_STATUS_OCCLUDED")
            && d3d11.contains("Errc::GraphicsOccluded")
            && d3d11.contains("Present(0, DXGI_PRESENT_TEST)")
            && d3d11.contains("DXGI_SWAP_EFFECT_DISCARD"),
        "D3D11 bitblt presentation must classify occlusion and use only DXGI's no-data exit probe"
    );
    assert!(
        d3d12.contains("DXGI_SWAP_EFFECT_FLIP_DISCARD") && !d3d12.contains("fn test_present("),
        "the flip-model D3D12 path must not falsely advertise DXGI occlusion status support"
    );
    assert!(
        scheduler.contains("    Occluded,")
            && scheduler.contains("OCCLUSION_PROBE_BASE_DELAY")
            && scheduler.contains("OCCLUSION_PROBE_MAX_DELAY")
            && scheduler.contains("pub(crate) fn take_due_occlusion_probe(")
            && scheduler.contains("SurfaceState::Suspended(_) => false"),
        "occlusion probes must be bounded registered work, never visual frame opportunities"
    );
    assert!(
        driver.contains("if self.frame_scheduler.take_due_occlusion_probe(now)")
            && driver.contains("engine.test_present()")
            && driver.contains("Ok(PresentTestResult::Occluded)")
            && driver.contains("Ok(PresentTestResult::Presentable)"),
        "WindowDriver must test occlusion before entering animation/layout/paint/present"
    );
    assert!(
        recovering.contains("matches!(failure, GraphicsFailure::Occluded(_))")
            && recovering.contains("self.engine.test_present()"),
        "a healthy occluded swapchain must be probed in place rather than rebuilt"
    );
}

#[test]
fn macos_occlusion_is_an_exact_per_window_signal_without_polling() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let window_traits = read_source(root.join("src/native/traits/window.rs"));
    let events = read_source(root.join("src/native/traits/event/types.rs"));
    let shared_window = read_source(root.join("src/native/shared/window.rs"));
    let delegate = read_source(root.join("src/native/backends/macos/window_delegate.rs"));
    let platform = read_source(root.join("src/native/backends/macos/platform.rs"));
    let driver = read_source(root.join("src/app/window_driver.rs"));

    assert!(
        window_traits.contains("pub enum WindowOcclusionState")
            && window_traits.contains("fn occlusion_state(&self) -> WindowOcclusionState")
            && window_traits.contains("WindowOcclusionState::Unknown")
            && shared_window.contains("fn os_occlusion_state(&self) -> WindowOcclusionState"),
        "exact compositor visibility must be optional so unsupported backends cannot claim exposure"
    );
    assert!(
        events.contains("WindowOcclusionChanged")
            && events.contains("pub fn window_occlusion_changed()"),
        "native occlusion changes must wake the shared per-window driver"
    );
    assert!(
        delegate.contains("windowDidChangeOcclusionState:")
            && delegate.contains("window_did_change_occlusion_state")
            && delegate.contains("UiEvent::window_occlusion_changed()")
            && delegate.contains("event.for_window((*context).window_id)"),
        "the AppKit delegate must route one occlusion notification to its exact WindowId"
    );
    assert!(
        platform.contains("NS_WINDOW_OCCLUSION_STATE_VISIBLE")
            && platform.contains("msg_usize(window, \"occlusionState\")")
            && platform.contains("WindowOcclusionState::Visible")
            && platform.contains("WindowOcclusionState::Occluded")
            && platform.contains("fn os_occlusion_state(&self) -> WindowOcclusionState"),
        "macOS must classify the current NSWindow occlusionState rather than trust notification order"
    );
    let availability_check = driver
        .find("self.suspend_if_surface_unavailable(tree, platform_window)")
        .expect("driver must recheck native surface availability");
    let drive_frame_tail = &driver[availability_check..];
    let present_probe = drive_frame_tail
        .find("self.frame_scheduler.take_due_occlusion_probe(now)")
        .expect("driver must retain DXGI's optional probe path");
    let visual_request = drive_frame_tail
        .find("self.arm_visual_request(now, tree, pending_root, *reconcile_pending)")
        .expect("driver must arm visual work only after availability checks");
    assert!(
        present_probe < visual_request,
        "native occlusion must suspend before animation, layout, paint, or present"
    );
    assert!(
        driver.contains("WindowOcclusionState::Occluded")
            && driver.contains("WindowOcclusionState::Visible")
            && driver.contains("WindowOcclusionState::Unknown")
            && driver.contains("Some(SurfaceSuspendReason::Hidden | SurfaceSuspendReason::Occluded)")
            && driver.contains("this path deliberately registers no")
            && driver.contains("availability probe deadline"),
        "event-driven macOS occlusion must retain dirty state, resume only exact suspension, and register no polling deadline"
    );
}

#[test]
fn wayland_window_modes_are_owned_per_window() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let backend = read_source(root.join("src/native/backends/linux/wayland/mod.rs"));
    let window = read_source(root.join("src/native/backends/linux/wayland/window.rs"));
    let window_ops = read_source(root.join("src/native/backends/linux/wayland/window_ops.rs"));

    assert!(
        !backend.contains("pub(crate) maximized: Arc<Mutex<bool>>")
            && !backend.contains("pub(crate) fullscreen: Arc<Mutex<bool>>")
            && window_ops.contains("configured_modes: Arc<Mutex<NativeWindowModeState>>")
            && window_ops.contains("NativeWindowModeState::default()")
            && window.contains("Rc::clone(&state)"),
        "Wayland configure state must be created per window and share only that window's WindowState"
    );
}

#[test]
fn wayland_input_events_follow_surface_focus_and_keep_their_window_target() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let window = read_source(root.join("src/native/backends/linux/wayland/window.rs"));
    let window_ops = read_source(root.join("src/native/backends/linux/wayland/window_ops.rs"));
    let seat = read_source(root.join("src/native/backends/linux/wayland/seat.rs"));
    let event_loop = read_source(root.join("src/native/backends/linux/wayland/event_loop.rs"));
    let text_input = read_source(root.join("src/native/backends/linux/wayland/text_input.rs"));
    let ime_events = read_source(root.join("src/native/shared/ime_events.rs"));

    assert!(
        window.contains("self.surface_windows.clone()")
            && window_ops.contains(".register_surface(surface_id, window_id)")
            && window_ops.contains(".unregister_surface(surface_id)"),
        "each live Wayland surface must be registered to exactly one application window"
    );
    assert!(
        seat.contains(".pointer_enter(surface.as_ref().id())")
            && seat.contains(".pointer_target()")
            && seat.contains(".keyboard_enter(surface.as_ref().id())")
            && seat.contains(".keyboard_target()")
            && seat.contains("event.for_window(window_id)")
            && seat.contains("UiEvent::key_down(code, current_mods).for_window(window_id)")
            && seat.contains("UiEventType::WindowFocus")
            && seat.contains("UiEventType::WindowBlur"),
        "Wayland pointer and keyboard callbacks must inherit the matching surface focus"
    );
    assert!(
        event_loop.contains("current_target != Some(window_id)")
            && event_loop.contains("UiEvent::key_down(code, mods).for_window(window_id)")
            && event_loop.contains("UiEvent::text_input(text).for_window(window_id)"),
        "Wayland key repeats must retain and revalidate their original window target"
    );
    assert!(
        text_input.contains("window_for_surface(surface.as_ref().id())")
            && text_input.contains("zwp_text_input_v3::Event::PreeditString")
            && text_input.contains("zwp_text_input_v3::Event::Done")
            && text_input.contains("active_generation.load(Ordering::SeqCst) != generation")
            && text_input.contains("pending.apply_for_window")
            && ime_events.contains("on_marked_text_for_window")
            && ime_events.contains("on_committed_text_for_window")
            && ime_events.contains("on_unmark_text_for_window"),
        "Wayland IME batches must be accepted only for the selected surface and target one window"
    );
}

#[test]
fn macos_native_contexts_have_real_owners_and_input_events_keep_their_window_target() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = read_source(root.join("src/native/backends/macos/objc_runtime.rs"));
    let delegate = read_source(root.join("src/native/backends/macos/window_delegate.rs"));
    let text_input = read_source(root.join("src/native/backends/macos/text_input_view.rs"));
    let platform = read_source(root.join("src/native/backends/macos/platform.rs"));
    let ime_owner = read_source(root.join("src/native/shared/ime_owner.rs"));

    assert!(
        runtime.contains("class_addIvar")
            && runtime.contains("ivar_getOffset")
            && runtime.contains("install_box")
            && runtime.contains("take_box")
            && runtime.contains("call_super_dealloc")
            && delegate.contains("_uixDelegateContext")
            && delegate.contains("delegate_dealloc")
            && text_input.contains("_uixTextInputContext")
            && text_input.contains("view_dealloc")
            && !delegate.contains("context as Id")
            && !text_input.contains("objc_setAssociatedObject"),
        "macOS Rust contexts must live in raw ivars and be reclaimed exactly once from Objective-C dealloc"
    );
    assert!(
        delegate.contains("retain_delegate_for_window(window, delegate)")
            && delegate.contains("objc_setAssociatedObject(")
            && delegate.contains("delegate,")
            && platform.contains("msg_void_bool(window, \"setReleasedWhenClosed:\", NO)")
            && platform.contains("impl Drop for MacosWindowOps")
            && platform.contains("cocoa::release_object(window)"),
        "a real Objective-C delegate and Rust-owned NSWindow must bound native callback lifetimes"
    );
    assert!(
        platform.contains("fn objc_msgSend_stret()")
            && platform.contains("unsafe extern \"C\" fn(*mut CGRect, Id, Sel)")
            && delegate.contains("fn objc_msgSend_stret()")
            && delegate.contains("unsafe extern \"C\" fn(*mut CGRect, Id, Sel)"),
        "x86_64 macOS CGRect message returns must use the Objective-C stret ABI"
    );
    assert!(
        platform.contains("window_id: Option<WindowId>")
            && platform.contains("window_delegate::window_id(event_window)")
            && platform.contains("event.for_window(window_id)")
            && platform.contains("suppress_keydown_text(event.window_id)")
            && delegate.contains("UiEventType::WindowFocus")
            && delegate.contains("UiEventType::WindowBlur"),
        "macOS pointer, keyboard, focus and blur events must resolve their originating NSWindow"
    );
    assert!(
        text_input.contains("view_window_id(view) != Some(window_id)")
            && text_input.contains("on_marked_text_for_window")
            && text_input.contains("on_committed_text_for_window")
            && text_input.contains("on_unmark_text_for_window")
            && text_input.contains("session_generation")
            && ime_owner.contains("deactivate_selected")
            && ime_owner.contains("self.active.map(|session| session.target) != Some(selected)"),
        "macOS IME callbacks must carry the selected window and stale blur must not stop a new owner"
    );
}

#[test]
fn wayland_clipboard_io_is_polled_bounded_and_uses_an_input_serial() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let clipboard = read_source(root.join("src/native/backends/linux/wayland/clipboard.rs"));
    let event_loop = read_source(root.join("src/native/backends/linux/wayland/event_loop.rs"));
    let seat = read_source(root.join("src/native/backends/linux/wayland/seat.rs"));

    assert!(
        clipboard.contains("libc::O_NONBLOCK")
            && clipboard.contains("CLIPBOARD_READ_BUDGET")
            && clipboard.contains("CLIPBOARD_WRITE_BUDGET")
            && event_loop.contains("clipboard_fd")
            && event_loop.contains("NonBlockingReadStatus::Pending")
            && event_loop.contains("POLLOUT")
            && event_loop.contains("write_clipboard_pipe")
            && !event_loop.contains("read_to_end")
            && !clipboard.contains("write_all"),
        "Wayland clipboard reads and writes must survive backpressure and do bounded work from the main poll"
    );
    assert!(
        seat.contains(".record(serial)")
            && clipboard.contains(".latest()")
            && clipboard.contains("dd.set_selection(Some(&source), serial)")
            && !clipboard.contains("dd.set_selection(Some(&source), 0)"),
        "Wayland set_selection must use a serial captured from pointer or keyboard input"
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
        source.contains("cpu_shadow")
            && source.contains("no uploaded frame to read back (cpu_shadow empty)"),
        "Vulkan must fail typed when no staged frame exists, not return an empty success buffer"
    );
    assert!(
        source.contains("hydrate_cpu_shadow_from_staging"),
        "Vulkan must lazy-hydrate CPU shadow on readback instead of copying every present"
    );
    assert!(
        !source.contains("VulkanContext: native readback is not supported"),
        "Vulkan PixelUpload now provides CPU-shadow readback for destination-dependent IR"
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
    let vulkan = fs::read_to_string(src.join("vulkan/platform/context.rs")).expect("Vulkan");

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
fn draw_gpu_engine_has_no_legacy_shader_reexports() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let gpu_engine = fs::read_to_string(src.join("draw/gpu_engine/mod.rs")).unwrap();
    let shader_names = [
        "BLIT_FRAG",
        "BLIT_RGBA_FRAG",
        "BLUR_FRAG",
        "FULLSCREEN_VERT",
        "RECT_FRAG",
        "RECT_VERT",
    ];

    assert!(
        shader_names.iter().all(|name| !gpu_engine.contains(name)),
        "native shader sources must not be re-exported through draw::gpu_engine"
    );
}

#[test]
fn internal_widget_tree_types_stay_off_user_entrypoints() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let checked = ["prelude.rs"];
    let internal_types = [
        "WidgetTree",
        "WidgetNode",
        "BoxedWidget",
        "ViewAdapter",
        "WidgetManagers",
        "OverlayStack",
    ];
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
        "internal runtime types must stay off user entrypoints: {violations:?}"
    );

    let ui_mod = fs::read_to_string(src.join("ui/mod.rs")).unwrap();
    let view_mod = fs::read_to_string(src.join("ui/view/mod.rs")).unwrap();
    assert!(
        ui_mod.contains("pub(crate) mod core;") && !ui_mod.contains("pub mod core;"),
        "ui::core must remain an internal runtime module"
    );
    assert!(
        view_mod.contains("pub(crate) mod adapter;") && !view_mod.contains("pub mod adapter;"),
        "ViewAdapter must remain an internal runtime module"
    );
}

#[test]
fn semantic_actions_stay_on_the_shared_widget_event_path() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let executor = read_source(src.join("ui/semantic_action.rs"));
    let automation = read_source(src.join("ui/automation.rs"));
    let snapshots = read_source(src.join("ui/component_snapshot.rs"));

    assert!(
        executor.contains("self.dispatch_event(") && executor.contains("self.dispatch_semantic("),
        "semantic actions must reuse WidgetTree system/semantic event dispatch"
    );
    assert!(
        !executor.contains(".component_mut()") && !executor.contains("downcast_mut::<"),
        "semantic actions must not mutate component instances behind the normal event path"
    );
    assert!(
        automation.contains(".perform_semantic_action(node.id, &action)"),
        "TestApp must stay an adapter over the shared semantic action executor"
    );
    assert!(
        executor.contains(".and_then(|snapshot| snapshot.selection())")
            && executor.contains("!selection.multiple")
            && executor.contains("self.press_key(KeyCode::Down)")
            && snapshots.contains("pub struct SelectionSnapshot")
            && snapshots.contains("pub selected_indices: Vec<usize>")
            && snapshots.contains("pub disabled_indices: Vec<usize>"),
        "select must be declared from shared single-choice metadata and execute through keyboard events"
    );
}

#[test]
fn agent_semantics_share_one_snapshot_and_successful_present_boundary() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src");
    let cargo = read_source(root.join("Cargo.toml"));
    let semantic_snapshot = read_source(src.join("ui/semantic_snapshot.rs"));
    let automation = read_source(src.join("ui/automation.rs"));
    let window_semantics = read_source(src.join("app/window_semantics.rs"));
    let window_driver = read_source(src.join("app/window_driver.rs"));

    assert!(
        cargo.contains("agent-control = [") && cargo.contains("\"dep:serde_json\","),
        "the opt-in Agent Bridge must keep an explicit Cargo feature gate"
    );
    assert!(
        automation.contains("nodes: self.semantic_snapshot_body().nodes")
            && !automation.contains("ComponentConfigSnapshot::from_component"),
        "TestApp and recorders must reuse the transport-neutral semantic snapshot"
    );
    assert!(
        !semantic_snapshot.contains("crate::app"),
        "the UI semantic model must not depend on app/session transport state"
    );
    assert!(
        window_semantics.contains("if !self.enabled || self.snapshot.closed")
            && window_semantics.contains("tree.semantic_snapshot_body()"),
        "disabled semantic tracking must return before traversing the widget tree"
    );
    assert!(
        window_driver
            .contains("if frame_committed {\n            semantic_state.mark_presented();"),
        "presented_revision may advance only inside the successful frame commit boundary"
    );
}

#[test]
fn agent_commands_stay_bounded_targeted_and_ui_thread_owned() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let commands = read_source(src.join("app/agent_control.rs"));
    let runtime = read_source(src.join("app/session_runtime.rs"));
    let session = read_source(src.join("app/window_session.rs"));
    let driver = read_source(src.join("app/window_driver.rs"));
    let protocol = read_source(src.join("app/agent_protocol.rs"));

    assert!(
        commands.contains("DEFAULT_AGENT_COMMAND_QUEUE_CAPACITY: usize = 64")
            && commands.contains("MAX_AGENT_SETTLE_PASSES: usize = 32")
            && commands.contains("if inner.pending.len() >= inner.capacity"),
        "Agent ingress and synchronous settle must retain explicit hard bounds"
    );
    assert!(
        runtime.contains("session.agent_commands.submit(request)?")
            && runtime.contains("if should_wake {")
            && runtime.contains("self.wake_event_loop();"),
        "AppRuntime must wake only after the target window queue accepts its first command"
    );
    assert!(
        session.contains("agent_commands: WindowAgentState")
            && session.contains("self.agent_commands.close();"),
        "WindowSession must own and terminate its UI-side command state"
    );
    assert!(
        !runtime.contains("WidgetTree")
            && commands.contains("tree.perform_semantic_action(node_id, action)"),
        "transport/runtime ingress must not touch WidgetTree; only UI-side command drain may act"
    );
    assert!(
        commands.contains("AgentWindowAction::PressKey")
            && commands.contains("tree.dispatch_event(&SystemEvent::KeyDown")
            && commands.contains("AgentWindowAction::ClickAt")
            && commands.contains("tree.dispatch_event(&SystemEvent::PointerDown")
            && protocol
                .contains("const AGENT_WINDOW_ACTIONS: &[&str] = &[\"press_key\", \"click_at\"]")
            && protocol.contains("window action must not include a target"),
        "the two window-level fallbacks must share normal UI events and remain target-free"
    );

    let main_queue = driver
        .find("main_thread_queue.drain(&mut main_thread_context)")
        .expect("main-thread queue drain");
    let agent_queue = driver
        .find("agent_commands.drain_ready(")
        .expect("Agent command drain");
    let app_state = driver
        .find("tree.drain_app_state_semantic_events()")
        .expect("AppState semantic drain");
    let settle = driver
        .find("observe_agent_settle(")
        .expect("bounded settle observation");
    let present = driver.find("match outcome {").expect("present boundary");
    assert!(
        main_queue < agent_queue && agent_queue < app_state,
        "commands must drain in the target UI turn between main-thread work and AppState effects"
    );
    assert!(
        settle < present,
        "settled responses must not wait for paint or present"
    );
}

#[test]
fn agent_bridge_is_explicit_process_scoped_and_transport_neutral() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let application = read_source(src.join("app/shell/application.rs"));
    let bridge = read_source(src.join("app/agent_bridge.rs"));
    let runtime = read_source(src.join("app/session_runtime.rs"));
    let semantics = read_source(src.join("app/window_semantics.rs"));

    assert!(
        application
            .contains("#[cfg(feature = \"agent-control\")]\n    pub fn enable_agent_control")
            && application.contains("if self.agent_control_enabled {")
            && application.contains("self.runtime.enable_agent_control()"),
        "Agent control must require both its Cargo feature and an explicit App runtime switch"
    );
    assert!(
        bridge.contains("pub(crate) struct AgentProcessBridge")
            && bridge.contains("pub(crate) fn list_windows")
            && bridge.contains("self.submit_for_live_window(")
            && bridge.contains("contains_live_agent_window(window_id)"),
        "one process Bridge must list live windows and route commands only to registered targets"
    );
    assert!(
        !bridge.contains("use crate::ui::WidgetTree")
            && !bridge.contains("&mut WidgetTree")
            && !bridge.contains("native::backends")
            && !bridge.contains("TcpListener")
            && !bridge.contains("UdpSocket"),
        "the process adapter must remain transport-neutral and unable to touch widget trees"
    );
    assert!(
        bridge.contains("MAX_AGENT_WAIT_TIMEOUT: Duration = Duration::from_secs(30)")
            && bridge.contains("waiters: BTreeMap<WindowId, Arc<Condvar>>")
            && bridge.contains(".wait_timeout(state, remaining)")
            && bridge.contains("state.waiters.get(&window_id).cloned()")
            && bridge.contains("waiter.notify_all()")
            && bridge.contains("AgentWaitCondition::PresentedAtLeast"),
        "wait must be bounded and block only target-window Bridge workers on directory notifications"
    );
    assert!(
        !bridge.contains("wake_event_loop")
            && !bridge.contains("post_to_ui")
            && !bridge.contains("run_interval")
            && !bridge.contains("sleep("),
        "wait must not poll, inject UI work, or wake the native event loop"
    );
    assert!(
        runtime.contains("agent_bridge: AgentBridgeDirectory")
            && runtime.contains("self.agent_bridge.close_window(window_id)")
            && runtime.contains("self.agent_bridge.close_all()"),
        "Bridge window and app lifecycle must be owned by AppRuntime"
    );
    assert!(
        semantics.contains("agent_window: Option<AgentWindowRegistration>")
            && semantics.contains("agent_window.publish_semantics(&self.snapshot)"),
        "only the UI-owned semantic state may publish revision metadata to discovery"
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
fn authored_widget_callbacks_live_in_keyed_side_tables() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let form = fs::read_to_string(root.join("src/ui/widgets/input/form.rs")).unwrap();
    let table = fs::read_to_string(root.join("src/ui/widgets/display/table.rs")).unwrap();
    let virtual_scroll =
        fs::read_to_string(root.join("src/ui/foundation/virtual_scroll.rs")).unwrap();
    let render_handlers = fs::read_to_string(root.join("src/ui/render_handler.rs")).unwrap();

    let form_component = form
        .split("pub struct Form {")
        .nth(1)
        .and_then(|tail| tail.split("measure =>").next())
        .expect("Form component declaration");
    let table_component = table
        .split("pub struct Table {")
        .nth(1)
        .and_then(|tail| tail.split("measure =>").next())
        .expect("Table component declaration");
    let virtual_scroll_component = virtual_scroll
        .split("pub struct VirtualScroll {")
        .nth(1)
        .and_then(|tail| tail.split("measure =>").next())
        .expect("VirtualScroll component declaration");

    assert!(
        !form_component.contains("Box<dyn Fn") && !form_component.contains("validator:"),
        "Form component data must not own custom validator closures"
    );
    assert!(
        form.contains("validator_key: Option<FormValidatorKey>")
            && form.contains("pub struct FormValidatorTable"),
        "Form rules must resolve named validators through FormValidatorTable"
    );
    assert!(
        !table_component.contains("expand_renderer"),
        "Table component data must not own expand renderer closures"
    );
    assert!(
        render_handlers.contains("HashMap<ComponentId, ExpandRenderer>")
            && render_handlers.contains("clear_component"),
        "table expand renderers must be keyed by ComponentId and cleared with node lifecycle"
    );
    assert!(
        !virtual_scroll_component.contains("Box<dyn Fn")
            && !virtual_scroll_component.contains("renderer:"),
        "VirtualScroll component data must not own item renderer closures"
    );
    assert!(
        render_handlers.contains("HashMap<ComponentId, VirtualScrollRenderer>"),
        "virtual scroll item renderers must be keyed by ComponentId"
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

#[test]
fn production_modules_do_not_embed_or_mount_tests() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut inline_body = Vec::new();
    let mut path_mount = Vec::new();
    let mut mod_tests = Vec::new();
    let mut bare_test_attr = Vec::new();

    for path in rust_files_under(&src) {
        let rel = relative_src_path(&path);
        if rel.starts_with("tests/") || rel == "lib.rs" {
            continue;
        }
        let text = read_source(&path);

        if text.contains("#[path") && text.contains("tests/") {
            path_mount.push(rel.clone());
        }
        if text.contains("mod tests;") {
            mod_tests.push(rel.clone());
        }
        if text.contains("#[test]") {
            bare_test_attr.push(rel.clone());
        }

        let mut search = text.as_str();
        let mut offset = 0usize;
        while let Some(idx) = search.find("mod ") {
            let abs = offset + idx;
            let after = &text[abs..];
            let Some(rest) = after.strip_prefix("mod ") else {
                break;
            };
            let name_end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            let name = &rest[..name_end];
            let after_name = rest[name_end..].trim_start();
            if after_name.starts_with('{') {
                let body_start = abs + after.find('{').expect("brace");
                let mut depth = 0i32;
                let mut i = body_start;
                let bytes = text.as_bytes();
                while i < bytes.len() {
                    match bytes[i] as char {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                let body = &text[body_start..=i.min(text.len().saturating_sub(1))];
                if body.contains("#[test]") {
                    inline_body.push(format!("{rel}::{name}"));
                }
            }
            offset = abs + 4;
            search = &text[offset..];
        }
    }

    assert!(
        inline_body.is_empty(),
        "inline test bodies must live under src/tests: {inline_body:?}"
    );
    assert!(
        path_mount.is_empty(),
        "production modules must not #[path]-mount tests: {path_mount:?}"
    );
    assert!(
        mod_tests.is_empty(),
        "production modules must not declare mod tests: {mod_tests:?}"
    );
    assert!(
        bare_test_attr.is_empty(),
        "production modules must not contain #[test]: {bare_test_attr:?}"
    );
}
