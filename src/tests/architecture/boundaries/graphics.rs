use super::support::*;
use std::fs;
use std::path::Path;

#[test]
fn all_desktop_gpu_recipes_share_the_wgpu_renderer() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let context = fs::read_to_string(src.join("native/graphics/wgpu_backend/mod.rs"))
        .expect("read shared wgpu context");
    let renderer = fs::read_to_string(src.join("native/graphics/wgpu_backend/renderer.rs"))
        .expect("read shared wgpu renderer");
    let shader = fs::read_to_string(src.join("native/graphics/wgpu_backend/renderer.wgsl"))
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
            && context.contains("WgpuRenderer::new"),
        "API selection must only choose a wgpu adapter backend"
    );
    assert!(renderer.contains("include_str!(\"renderer.wgsl\")"));
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
        "native/graphics/wgpu_backend/renderer.rs",
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
