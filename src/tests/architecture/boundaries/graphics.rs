use super::support::*;
use std::fs;
use std::path::Path;

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
