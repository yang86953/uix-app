use super::support::*;
use std::fs;
use std::path::Path;

#[test]
fn ui_and_demo_production_paint_only_use_paint_context() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let demo_tests = manifest.join("demo/src/tests");
    let forbidden = [
        (".canvas_2d(", "raw Canvas2D access"),
        (".spatial(", "raw SpatialContext access"),
        ("SpatialContext::new", "raw SpatialContext construction"),
        ("Canvas2D", "the low-level Canvas2D type"),
        ("pixels_mut(", "mutable framebuffer access"),
        ("draw::backend", "a draw backend module"),
        ("RenderBackend", "the internal render backend trait"),
        ("DrawSurface", "a backend draw surface"),
        ("draw::rasterizer", "a rasterizer module"),
        ("native::graphics", "a native graphics module"),
        ("FrameEncoder", "the internal frame encoder"),
        ("CpuCanvas2D", "the CPU canvas implementation"),
        ("NoopCanvas2D", "the test canvas implementation"),
        ("RasterRenderer", "a raster renderer implementation"),
        ("RenderingBackend", "the internal rendering backend trait"),
        ("SharedRasterizer", "the software rasterizer implementation"),
        ("PixelSurface", "a mutable pixel surface"),
        ("wgpu::", "direct wgpu access"),
    ];
    let mut files = rust_files_under(&manifest.join("src/ui"));
    files.extend(rust_files_under(&manifest.join("demo/src")));
    files.sort();

    let mut violations = Vec::new();
    for file in files {
        if file.starts_with(&demo_tests) {
            continue;
        }
        let source = rust_code_without_comments_or_strings(&read_source(&file));
        let relative = file.strip_prefix(manifest).unwrap_or(&file);
        for (line_index, line) in source.lines().enumerate() {
            for (needle, boundary) in forbidden {
                if line.contains(needle) {
                    violations.push(format!(
                        "{}:{} reaches {boundary} via `{needle}`",
                        relative.display(),
                        line_index + 1
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "UI and Demo production drawing must stay behind PaintContext; raw canvas, pixels, and \
         renderer internals are restricted to draw/native and the app presenter boundary:\n{}",
        violations.join("\n")
    );
}

#[test]
fn draw_exposes_one_renderer_runtime_and_no_legacy_engine_types() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = manifest.join("src");
    let draw = src.join("draw");
    let demo_src = manifest.join("demo/src");
    let excluded_roots = [src.join("tests"), demo_src.join("tests")];

    let mut renderer_declarations = Vec::new();
    for file in rust_files_under(&draw) {
        let source = rust_code_without_comments_or_strings(&read_source(&file));
        let identifiers = rust_identifiers(&source).collect::<Vec<_>>();
        let count = identifiers
            .windows(3)
            .filter(|tokens| tokens[0] == "pub" && tokens[1] == "struct" && tokens[2] == "Renderer")
            .count();
        let relative = file
            .strip_prefix(&draw)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        for _ in 0..count {
            renderer_declarations.push(relative.clone());
        }
    }
    renderer_declarations.sort();

    assert_eq!(
        renderer_declarations,
        ["renderer/runtime.rs".to_string()],
        "src/draw must expose exactly one public Renderer, from renderer/runtime.rs"
    );

    let legacy_names = [
        "SoftwareEngine",
        "GpuEngine",
        "PresentUploadEngine",
        "NullEngine",
        "GraphicsEngine",
        "FrameRenderer",
        "FrameRecordingEngine",
        "RecoveringGraphicsEngine",
        "WgpuRenderer",
    ];
    let mut legacy_violations = Vec::new();
    for root in [&src, &demo_src] {
        for file in rust_files_under(root) {
            if excluded_roots
                .iter()
                .any(|excluded| file.starts_with(excluded))
            {
                continue;
            }
            let source = rust_code_without_comments_or_strings(&read_source(&file));
            let relative = file.strip_prefix(manifest).unwrap_or(&file);
            for (line_index, line) in source.lines().enumerate() {
                for identifier in rust_identifiers(line) {
                    if legacy_names.contains(&identifier) {
                        legacy_violations.push(format!(
                            "{}:{} still uses legacy type `{identifier}`",
                            relative.display(),
                            line_index + 1
                        ));
                    }
                }
            }
        }
    }

    assert!(
        legacy_violations.is_empty(),
        "legacy renderer and engine types must not return to production code:\n{}",
        legacy_violations.join("\n")
    );
}

#[test]
fn test_render_backend_is_cfg_test_only() {
    let draw = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/draw");
    let backend_mod =
        rust_code_without_comments_or_strings(&read_source(draw.join("backend/mod.rs")));

    assert_item_has_cfg_test(&backend_mod, "mod test_backend;");
    assert_item_has_cfg_test(&backend_mod, "use test_backend::TestBackend;");
}

#[test]
fn all_desktop_gpu_recipes_share_the_wgpu_executor() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let context = fs::read_to_string(src.join("native/graphics/wgpu_backend/mod.rs"))
        .expect("read shared wgpu context");
    let executor = fs::read_to_string(src.join("native/graphics/wgpu_backend/executor.rs"))
        .expect("read shared wgpu executor");
    let shader = fs::read_to_string(src.join("native/graphics/wgpu_backend/executor.wgsl"))
        .expect("read shared WGSL shader");

    for relative in [
        "native/factory/registry_windows.rs",
        "native/factory/registry_linux.rs",
        "native/factory/registry_macos.rs",
    ] {
        let registry = fs::read_to_string(src.join(relative)).expect("read graphics registry");
        assert!(
            registry.contains("native::graphics::wgpu_backend")
                && registry.contains("RasterMode::GpuNative")
                && registry.contains("PresentMode::Swapchain"),
            "{relative} must route active GPU recipes through the shared wgpu context"
        );
    }

    assert!(
        context.contains("wgpu::Backends::VULKAN")
            && context.contains("wgpu::Backends::DX12")
            && context.contains("wgpu::Backends::METAL")
            && context.contains("wgpu::Backends::GL")
            && context.contains("WgpuExecutor::new"),
        "API selection must only choose a wgpu adapter backend"
    );
    assert!(executor.contains("include_str!(\"executor.wgsl\")"));
    assert!(shader.contains("fn shape_fs") && shader.contains("fn glyph_fs"));
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
        "native/graphics/wgpu_backend/mod.rs",
        "native/graphics/wgpu_backend/executor.rs",
        "native/graphics/wgpu_backend/platform/surface.rs",
    ] {
        let source = fs::read_to_string(src.join(relative)).expect("read native API object source");
        assert!(
            !source.contains("pub fn new("),
            "{relative} must not expose a raw native API constructor outside the crate"
        );
    }
}

#[test]
fn wgpu_windows_surface_uses_the_shared_drawable_dpr_contract() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native/graphics");
    let platform = fs::read_to_string(src.join("platform/windows.rs"))
        .expect("read shared Windows graphics platform helper");
    let surface = fs::read_to_string(src.join("wgpu_backend/platform/surface.rs"))
        .expect("read wgpu surface adapter");
    let context =
        fs::read_to_string(src.join("wgpu_backend/mod.rs")).expect("read wgpu graphics context");

    assert!(
        platform.contains("pub(crate) struct DrawableSize")
            && platform.contains("pub(crate) fn drawable_size")
            && platform.contains("pub(crate) fn drawable_size_from_hdc"),
        "Windows surface code must retain one logical-to-drawable DPR calculation"
    );
    assert!(
        surface.contains("platform::windows::drawable_size")
            && context.matches("surface::drawable_extent").count() >= 2
            && context.contains("fn device_pixel_ratio(&self) -> f32"),
        "wgpu create, resize, and caps must use the shared drawable extent"
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

fn rust_identifiers(source: &str) -> impl Iterator<Item = &str> {
    source
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .filter(|identifier| !identifier.is_empty())
}

fn assert_item_has_cfg_test(source: &str, item: &str) {
    let lines = source.lines().collect::<Vec<_>>();
    let matches = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| line.contains(item).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one `{item}` declaration, found {}",
        matches.len()
    );

    let item_line = matches[0];
    let cfg_test = lines[..item_line]
        .iter()
        .rev()
        .map(|line| line.trim())
        .skip_while(|line| line.is_empty())
        .take_while(|line| line.starts_with("#["))
        .any(|line| line == "#[cfg(test)]");
    assert!(cfg_test, "`{item}` must be gated by #[cfg(test)]");
}
