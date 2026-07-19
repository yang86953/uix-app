//! Shared `wgpu` graphics context. All native GPU APIs execute the same UIX
//! renderer; backend selection changes only wgpu's adapter implementation.

pub(crate) mod renderer;
#[path = "platform/surface.rs"]
mod surface;

use std::ffi::c_void;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::core::{Errc, Error, Result};
use crate::native::traits::present::{
    GpuBoxShadow, GpuGlyphBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSolidMesh,
    GpuSolidRect, GpuStrokeRect, GraphicsBackend, GraphicsContextCaps, IGraphicsContext,
    NativeRasterCaps, PresentCoherency, PresentDamage, PresentTestResult,
};

use renderer::WgpuRenderer;

pub(crate) fn create_vulkan(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    WgpuContext::new(surface, width, height, GraphicsBackend::Vulkan)
        .map(|context| Box::new(context) as _)
}

#[allow(dead_code)] // compiled on every target so platform cfg stays in platform/**
pub(crate) fn create_d3d12(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    WgpuContext::new(surface, width, height, GraphicsBackend::D3d12)
        .map(|context| Box::new(context) as _)
}

#[allow(dead_code)] // compiled on every target so platform cfg stays in platform/**
pub(crate) fn create_opengl(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    WgpuContext::new(surface, width, height, GraphicsBackend::OpenGlEs)
        .map(|context| Box::new(context) as _)
}

#[allow(dead_code)] // compiled on every target so platform cfg stays in platform/**
pub(crate) fn create_metal(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    WgpuContext::new(surface, width, height, GraphicsBackend::Metal)
        .map(|context| Box::new(context) as _)
}

pub struct WgpuContext {
    _instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    _adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: WgpuRenderer,
    backend: GraphicsBackend,
    native_surface: *mut c_void,
    logical_width: i32,
    logical_height: i32,
    width: i32,
    height: i32,
    device_lost: Arc<AtomicBool>,
    shutdown: bool,
}

impl WgpuContext {
    fn new(
        native_surface: *mut c_void,
        width: i32,
        height: i32,
        requested: GraphicsBackend,
    ) -> Result<Self> {
        let backends = wgpu_backends(requested)?;
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = backends;
        descriptor.flags = wgpu::InstanceFlags::DISCARD_HAL_LABELS;
        let instance = wgpu::Instance::new(descriptor);
        let surface = unsafe { surface::create_surface(&instance, native_surface)? };
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
            apply_limit_buckets: false,
        }))
        .map_err(|error| {
            Error::new(
                Errc::PlatformError,
                format!("wgpu {requested} adapter unavailable: {error}"),
            )
        })?;
        let info = adapter.get_info();
        if graphics_backend(info.backend) != requested {
            return Err(Error::new(
                Errc::PlatformError,
                format!(
                    "wgpu requested {requested}, but selected {:?} adapter {}",
                    info.backend, info.name
                ),
            ));
        }
        let descriptor = wgpu::DeviceDescriptor {
            label: Some("uix-wgpu-device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            ..Default::default()
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&descriptor)).map_err(|error| {
                Error::new(Errc::PlatformError, format!("wgpu request_device: {error}"))
            })?;
        device.on_uncaptured_error(Arc::new(|error| {
            crate::core::log::error_fn(format_args!("wgpu uncaptured error: {error}"));
        }));
        let device_lost = Arc::new(AtomicBool::new(false));
        let callback_device_lost = Arc::clone(&device_lost);
        device.set_device_lost_callback(move |reason, message| {
            callback_device_lost.store(true, Ordering::Release);
            crate::core::log::error_fn(format_args!(
                "wgpu device lost: reason={reason:?}; message={message}"
            ));
        });
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| !format.is_srgb())
            .or_else(|| capabilities.formats.first().copied())
            .ok_or_else(|| Error::new(Errc::PlatformError, "wgpu surface has no formats"))?;
        let present_mode = capabilities
            .present_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::Fifo)
            .or_else(|| capabilities.present_modes.first().copied())
            .ok_or_else(|| Error::new(Errc::PlatformError, "wgpu surface has no present modes"))?;
        let alpha_mode = capabilities
            .alpha_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::CompositeAlphaMode::PreMultiplied)
            .or_else(|| capabilities.alpha_modes.first().copied())
            .ok_or_else(|| Error::new(Errc::PlatformError, "wgpu surface has no alpha modes"))?;
        let extent = surface::drawable_extent(native_surface, width, height);
        let drawable_width = extent.width;
        let drawable_height = extent.height;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: drawable_width,
            height: drawable_height,
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);
        let renderer = WgpuRenderer::new(&device, &queue, format)?;
        crate::core::log::info_fn(format_args!(
            "WgpuContext: backend={requested}; adapter=\"{}\"; type={:?}; driver=\"{}\"; {}x{}",
            info.name, info.device_type, info.driver, drawable_width, drawable_height
        ));
        Ok(Self {
            _instance: instance,
            surface,
            _adapter: adapter,
            device,
            queue,
            config,
            renderer,
            backend: requested,
            native_surface,
            logical_width: extent.logical_width,
            logical_height: extent.logical_height,
            width: drawable_width as i32,
            height: drawable_height as i32,
            device_lost,
            shutdown: false,
        })
    }

    fn ensure_active(&self) -> Result<()> {
        if self.shutdown {
            Err(Error::new(
                Errc::InvalidState,
                "WgpuContext operation requested after shutdown",
            ))
        } else if self.device_lost.load(Ordering::Acquire) {
            Err(Error::new(Errc::GraphicsDeviceLost, "wgpu device was lost"))
        } else {
            Ok(())
        }
    }
}

impl IGraphicsContext for WgpuContext {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(
            self.backend,
            PresentCoherency::FullOnly,
            self.device_pixel_ratio(),
        )
    }

    fn native_raster_caps(&self) -> NativeRasterCaps {
        NativeRasterCaps::wgpu_full()
    }

    fn initialize(&mut self, _native_window: *mut c_void, _width: i32, _height: i32) -> Result<()> {
        self.ensure_active()
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        self.ensure_active()?;
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        let extent = surface::drawable_extent(self.native_surface, width, height);
        self.logical_width = extent.logical_width;
        self.logical_height = extent.logical_height;
        self.width = extent.width as i32;
        self.height = extent.height as i32;
        self.config.width = extent.width;
        self.config.height = extent.height;
        self.surface.configure(&self.device, &self.config);
        Ok(())
    }

    fn make_current(&mut self) -> Result<()> {
        self.ensure_active()
    }

    fn swap_buffers(&mut self, _damage: PresentDamage) -> Result<()> {
        self.ensure_active()?;
        match self.renderer.present(
            &self.device,
            &self.queue,
            &self.surface,
            self.config.width,
            self.config.height,
        ) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == Errc::GraphicsSurfaceLost => {
                self.surface.configure(&self.device, &self.config);
                Err(error)
            }
            Err(error) => Err(error),
        }
    }

    fn try_shutdown(&mut self) -> Result<()> {
        if self.shutdown {
            return Ok(());
        }
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        self.shutdown = true;
        Ok(())
    }

    fn read_pixels(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) -> Result<Vec<u32>> {
        Err(Error::new(
            Errc::NotImplemented,
            "GPU-only WgpuContext does not expose synchronous CPU readback",
        ))
    }

    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }

    fn device_pixel_ratio(&self) -> f32 {
        self.width as f32 / self.logical_width.max(1) as f32
    }

    fn test_present(&mut self) -> Result<PresentTestResult> {
        Err(Error::new(
            Errc::NotImplemented,
            "wgpu surface does not expose a no-draw present test",
        ))
    }

    fn clear_render_target(&mut self, r: f32, g: f32, b: f32, a: f32) -> Result<()> {
        self.renderer.begin_frame([r, g, b, a]);
        Ok(())
    }

    fn draw_solid_rects(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuSolidRect],
    ) -> Result<()> {
        self.renderer
            .solid_rects((viewport_w, viewport_h), scissor, rects);
        Ok(())
    }

    fn draw_stroke_rects(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuStrokeRect],
    ) -> Result<()> {
        self.renderer
            .stroke_rects((viewport_w, viewport_h), scissor, rects);
        Ok(())
    }

    fn draw_glyphs(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        glyphs: &[GpuGlyphBlit],
    ) -> Result<()> {
        self.renderer
            .glyphs((viewport_w, viewport_h), scissor, glyphs)
    }

    fn draw_linear_gradients(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuLinearGradientRect],
    ) -> Result<()> {
        self.renderer
            .linear_gradients((viewport_w, viewport_h), scissor, rects);
        Ok(())
    }

    fn draw_radial_gradients(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        gradients: &[GpuRadialGradient],
    ) -> Result<()> {
        self.renderer
            .radial_gradients((viewport_w, viewport_h), scissor, gradients);
        Ok(())
    }

    fn draw_solid_meshes(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        meshes: &[GpuSolidMesh],
    ) -> Result<()> {
        self.renderer
            .solid_meshes((viewport_w, viewport_h), scissor, meshes);
        Ok(())
    }

    fn draw_box_shadows(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        shadows: &[GpuBoxShadow],
    ) -> Result<()> {
        self.renderer
            .box_shadows((viewport_w, viewport_h), scissor, shadows);
        Ok(())
    }

    fn bind_swapchain_target(&mut self) -> Result<()> {
        self.ensure_active()
    }
}

fn wgpu_backends(backend: GraphicsBackend) -> Result<wgpu::Backends> {
    match backend {
        GraphicsBackend::Vulkan => Ok(wgpu::Backends::VULKAN),
        GraphicsBackend::D3d12 => Ok(wgpu::Backends::DX12),
        GraphicsBackend::Metal => Ok(wgpu::Backends::METAL),
        GraphicsBackend::OpenGlEs => Ok(wgpu::Backends::GL),
        other => Err(Error::new(
            Errc::InvalidArgument,
            format!("wgpu does not expose backend {other}"),
        )),
    }
}

fn graphics_backend(backend: wgpu::Backend) -> GraphicsBackend {
    match backend {
        wgpu::Backend::Vulkan => GraphicsBackend::Vulkan,
        wgpu::Backend::Dx12 => GraphicsBackend::D3d12,
        wgpu::Backend::Metal => GraphicsBackend::Metal,
        wgpu::Backend::Gl => GraphicsBackend::OpenGlEs,
        _ => GraphicsBackend::Auto,
    }
}

impl Drop for WgpuContext {
    fn drop(&mut self) {
        if let Err(error) = self.try_shutdown() {
            crate::core::log::error_fn(format_args!(
                "WgpuContext shutdown failed: {}",
                error.short_what()
            ));
        }
    }
}
