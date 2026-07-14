//! Direct3D 11 graphics context for Windows.
//!
//! Caps: [`RasterMode::GpuNative`] × [`PresentMode::Swapchain`] (#169).
//! Native solid/rounded fill + stroke + glyph atlas text + linear/radial
//! gradients + simple path meshes + box/ambient shadow; unsupported Canvas2D
//! ops soft-raster and alpha-blit (same hybrid pattern as the native GL path).
//! bitblt swapchain 固定使用 `DXGI_SWAP_EFFECT_DISCARD`；状态边界把
//! `DXGI_STATUS_OCCLUDED` 映射为 `Errc::GraphicsOccluded`，并以
//! `Present(0, DXGI_PRESENT_TEST)` 做无帧数据的退出探测。

#![allow(nonstandard_style)]

use std::ffi::c_void;

use super::pipeline::D3d11Pipeline;
use super::swapchain::d3d_error;
pub(crate) use super::swapchain::{
    map_dxgi_present_result, map_dxgi_present_test_result, map_dxgi_resize_result, swap_chain_desc,
};
use crate::core::{Errc, Error, Result};
use crate::native::graphics::platform::windows as win_surface;
use crate::native::traits::present::{
    GpuBoxShadow, GpuGlyphBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSolidMesh,
    GpuSolidRect, GpuStrokeRect, GraphicsBackend, GraphicsContextCaps, IGraphicsContext,
    NativeRasterCaps, OffscreenTargetId, PresentCoherency, PresentDamage, PresentFrame,
    PresentTestResult, SoftFallbackTile,
};
use ::windows::core::Interface;
use ::windows::Win32::Foundation::HMODULE;
use ::windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL,
    D3D_FEATURE_LEVEL_10_0, D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
};
use ::windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDeviceAndSwapChain, ID3D11Device, ID3D11DeviceContext, ID3D11RenderTargetView,
    ID3D11ShaderResourceView, ID3D11Texture2D, D3D11_BIND_RENDER_TARGET,
    D3D11_BIND_SHADER_RESOURCE, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING, D3D11_VIEWPORT,
};
use ::windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use ::windows::Win32::Graphics::Dxgi::{
    IDXGIDevice, IDXGISwapChain, DXGI_PRESENT, DXGI_PRESENT_TEST, DXGI_SWAP_CHAIN_FLAG,
};

type HWND_PTR = *mut c_void;

struct OffscreenTarget {
    #[allow(dead_code)] // kept alive for RTV/SRV; not read directly after create
    texture: ID3D11Texture2D,
    pub(crate) rtv: ID3D11RenderTargetView,
    srv: ID3D11ShaderResourceView,
    width: i32,
    height: i32,
}

pub(crate) const D3D11_FEATURE_LEVELS: [D3D_FEATURE_LEVEL; 4] = [
    D3D_FEATURE_LEVEL_11_1,
    D3D_FEATURE_LEVEL_11_0,
    D3D_FEATURE_LEVEL_10_1,
    D3D_FEATURE_LEVEL_10_0,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum D3d11DriverKind {
    Hardware,
    Warp,
}

impl D3d11DriverKind {
    fn native(self) -> D3D_DRIVER_TYPE {
        match self {
            Self::Hardware => D3D_DRIVER_TYPE_HARDWARE,
            Self::Warp => D3D_DRIVER_TYPE_WARP,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hardware => "hardware",
            Self::Warp => "warp",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D3d11AdapterInfo {
    pub driver: D3d11DriverKind,
    pub description: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub dedicated_video_memory: u64,
}

impl D3d11AdapterInfo {
    fn unavailable(driver: D3d11DriverKind) -> Self {
        Self {
            driver,
            description: "unavailable".to_string(),
            vendor_id: 0,
            device_id: 0,
            dedicated_video_memory: 0,
        }
    }

    pub fn diagnostic_summary(&self) -> String {
        format!(
            "driver={}; adapter=\"{}\"; vendor={:#06X}; device={:#06X}; dedicated_vram_mb={}",
            self.driver.as_str(),
            self.description,
            self.vendor_id,
            self.device_id,
            self.dedicated_video_memory / (1024 * 1024)
        )
    }
}

fn query_adapter_info(device: &ID3D11Device, driver: D3d11DriverKind) -> Result<D3d11AdapterInfo> {
    let dxgi_device: IDXGIDevice = device
        .cast()
        .map_err(|err| d3d_error("ID3D11Device::cast<IDXGIDevice>", err))?;
    let adapter = unsafe { dxgi_device.GetAdapter() }
        .map_err(|err| d3d_error("IDXGIDevice::GetAdapter", err))?;
    let desc =
        unsafe { adapter.GetDesc() }.map_err(|err| d3d_error("IDXGIAdapter::GetDesc", err))?;
    let description_len = desc
        .Description
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(desc.Description.len());
    let description = String::from_utf16_lossy(&desc.Description[..description_len]);
    Ok(D3d11AdapterInfo {
        driver,
        description,
        vendor_id: desc.VendorId,
        device_id: desc.DeviceId,
        dedicated_video_memory: desc.DedicatedVideoMemory as u64,
    })
}

pub struct D3d11Context {
    hwnd: HWND_PTR,
    device: ID3D11Device,
    pub(crate) context: ID3D11DeviceContext,
    swap_chain: IDXGISwapChain,
    pub(crate) rtv: Option<ID3D11RenderTargetView>,
    pipeline: D3d11Pipeline,
    pub(crate) adapter_info: D3d11AdapterInfo,
    logical_width: i32,
    logical_height: i32,
    width: i32,
    height: i32,
    offscreens: Vec<Option<OffscreenTarget>>,
    free_offscreen_ids: Vec<u32>,
    next_offscreen_id: u32,
    /// When set, draw/clear target the offscreen instead of the swapchain.
    bound_offscreen: Option<u32>,
}

impl D3d11Context {
    pub(crate) fn new(native_window: *mut c_void, width: i32, height: i32) -> Result<Self> {
        if native_window.is_null() {
            return Err(Error::new(
                Errc::PlatformError,
                "D3d11Context: native window handle is null",
            ));
        }
        let drawable = win_surface::drawable_size(native_window, width, height);
        match create_with_drawable(
            native_window,
            drawable,
            &D3D11_FEATURE_LEVELS,
            D3d11DriverKind::Hardware,
        ) {
            Ok(context) => Ok(context),
            Err(hardware_error) => {
                crate::core::log::warn_fn(format!(
                    "D3d11Context: hardware device unavailable; retrying with WARP: {}",
                    hardware_error.what()
                ));
                create_with_drawable(
                    native_window,
                    drawable,
                    &D3D11_FEATURE_LEVELS,
                    D3d11DriverKind::Warp,
                )
                .map_err(|warp_error| {
                    Error::new(
                        Errc::PlatformError,
                        format!(
                            "D3d11Context: hardware and WARP creation failed; hardware=[{}]; warp=[{}]",
                            hardware_error.what(),
                            warp_error.what()
                        ),
                    )
                })
            }
        }
    }

    /// Deterministic WARP-only constructor for crate tests.
    ///
    /// Production construction deliberately keeps the hardware-then-WARP
    /// policy in [`Self::new`]. Tests that compare backend pixels must not
    /// inherit a machine-specific hardware adapter instead.
    pub(crate) fn new_warp_test_context(
        native_window: *mut c_void,
        width: i32,
        height: i32,
    ) -> Result<Self> {
        if native_window.is_null() {
            return Err(Error::new(
                Errc::PlatformError,
                "D3d11Context: native window handle is null",
            ));
        }
        let drawable = win_surface::drawable_size(native_window, width, height);
        create_with_drawable(
            native_window,
            drawable,
            &D3D11_FEATURE_LEVELS,
            D3d11DriverKind::Warp,
        )
    }

    fn create_rtv(&mut self) -> Result<()> {
        let back_buffer: ID3D11Texture2D = unsafe {
            self.swap_chain
                .GetBuffer(0)
                .map_err(|err| d3d_error("IDXGISwapChain::GetBuffer", err))?
        };
        let mut rtv = None;
        unsafe {
            self.device
                .CreateRenderTargetView(&back_buffer, None, Some(&mut rtv))
                .map_err(|err| d3d_error("ID3D11Device::CreateRenderTargetView", err))?;
        }
        let rtv = rtv.ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "D3d11Context: CreateRenderTargetView returned no RTV",
            )
        })?;
        unsafe {
            self.context
                .OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
        }
        self.bind_viewport();
        self.rtv = Some(rtv);
        Ok(())
    }

    fn release_rtv(&mut self) {
        unsafe {
            self.context.OMSetRenderTargets(None, None);
        }
        self.rtv = None;
    }

    fn shutdown_result(&mut self) -> Result<()> {
        self.bound_offscreen = None;
        self.offscreens.clear();
        self.free_offscreen_ids.clear();
        self.next_offscreen_id = 0;
        self.release_rtv();
        Ok(())
    }

    fn bind_viewport(&self) {
        self.bind_viewport_size(self.width, self.height);
    }

    fn bind_viewport_size(&self, width: i32, height: i32) {
        let vp = D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: width.max(1) as f32,
            Height: height.max(1) as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };
        unsafe {
            self.context.RSSetViewports(Some(&[vp]));
        }
    }

    fn ensure_rtv(&mut self) -> Result<()> {
        if self.rtv.is_none() {
            self.create_rtv()?;
        }
        Ok(())
    }

    fn current_target_size(&self) -> (i32, i32) {
        if let Some(id) = self.bound_offscreen {
            if let Some(Some(t)) = self.offscreens.get(id as usize) {
                return (t.width, t.height);
            }
        }
        (self.width, self.height)
    }

    fn bind_current_draw_target(&mut self) -> Result<()> {
        if let Some(id) = self.bound_offscreen {
            let Some(Some(target)) = self.offscreens.get(id as usize) else {
                return Err(Error::new(
                    Errc::InvalidArgument,
                    format!("D3d11Context: bind_current_draw_target unknown offscreen {id}"),
                ));
            };
            unsafe {
                self.context
                    .OMSetRenderTargets(Some(&[Some(target.rtv.clone())]), None);
            }
            self.bind_viewport_size(target.width, target.height);
            return Ok(());
        }
        self.ensure_rtv()?;
        if let Some(rtv) = self.rtv.as_ref() {
            unsafe {
                self.context
                    .OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
            }
            self.bind_viewport();
        }
        Ok(())
    }

    fn present_result(&mut self) -> Result<()> {
        // SAFETY: the swap chain belongs to this context and is used only on
        // its owning UI thread while the context remains alive.
        //
        // Release all back-buffer refs *before* Present. Holding an RTV (or any
        // GetBuffer view) across Present with DXGI_SWAP_EFFECT_DISCARD lets DXGI
        // allocate extra swapchain buffers — unbounded GPU/system memory growth.
        // Recreate RTV *after* Present so the next frame targets the current
        // back buffer (stale RTV → alternating good/black frames, BUG-001).
        self.release_rtv();
        map_dxgi_present_result(unsafe { self.swap_chain.Present(1, DXGI_PRESENT(0)) })?;
        self.create_rtv()?;
        Ok(())
    }
}

pub(crate) fn create_with_driver(
    hwnd: HWND_PTR,
    width: i32,
    height: i32,
    feature_levels: &[D3D_FEATURE_LEVEL],
    driver: D3d11DriverKind,
) -> Result<D3d11Context> {
    let mut swap_chain = None;
    let mut device = None;
    let mut context = None;
    let mut selected_level = D3D_FEATURE_LEVEL_10_0;
    let desc = swap_chain_desc(hwnd, width, height);

    unsafe {
        D3D11CreateDeviceAndSwapChain(
            None,
            driver.native(),
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(feature_levels),
            D3D11_SDK_VERSION,
            Some(&desc),
            Some(&mut swap_chain),
            Some(&mut device),
            Some(&mut selected_level),
            Some(&mut context),
        )
    }
    .map_err(|err| d3d_error("D3D11CreateDeviceAndSwapChain", err))?;

    let device = device.ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            "D3d11Context: D3D11CreateDeviceAndSwapChain returned no device",
        )
    })?;
    let context = context.ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            "D3d11Context: D3D11CreateDeviceAndSwapChain returned no device context",
        )
    })?;
    let swap_chain = swap_chain.ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            "D3d11Context: D3D11CreateDeviceAndSwapChain returned no swapchain",
        )
    })?;

    let adapter_info = query_adapter_info(&device, driver).unwrap_or_else(|error| {
        crate::core::log::warn_fn(format!(
            "D3d11Context: adapter diagnostics unavailable: {}",
            error.what()
        ));
        D3d11AdapterInfo::unavailable(driver)
    });
    let pipeline = D3d11Pipeline::new(&device)?;
    let mut ctx = D3d11Context {
        hwnd,
        device,
        context,
        swap_chain,
        rtv: None,
        pipeline,
        adapter_info,
        logical_width: width,
        logical_height: height,
        width,
        height,
        offscreens: Vec::new(),
        free_offscreen_ids: Vec::new(),
        next_offscreen_id: 0,
        bound_offscreen: None,
    };
    crate::core::log::info_fn(format!(
        "D3d11Context: created {width}x{height} swapchain at feature level {:?}; {}",
        selected_level,
        ctx.adapter_info.diagnostic_summary()
    ));
    ctx.create_rtv()?;
    Ok(ctx)
}

fn create_with_drawable(
    hwnd: HWND_PTR,
    drawable: win_surface::DrawableSize,
    feature_levels: &[D3D_FEATURE_LEVEL],
    driver: D3d11DriverKind,
) -> Result<D3d11Context> {
    let mut context = create_with_driver(
        hwnd,
        drawable.width,
        drawable.height,
        feature_levels,
        driver,
    )?;
    context.logical_width = drawable.logical_width;
    context.logical_height = drawable.logical_height;
    Ok(context)
}

impl IGraphicsContext for D3d11Context {
    fn caps(&self) -> crate::native::traits::present::GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(
            GraphicsBackend::D3d11,
            PresentCoherency::FullOnly,
            self.device_pixel_ratio(),
        )
    }

    fn graphics_backend(&self) -> GraphicsBackend {
        GraphicsBackend::D3d11
    }

    fn native_raster_caps(&self) -> NativeRasterCaps {
        NativeRasterCaps::d3d11_full()
    }

    fn initialize(&mut self, _native_window: *mut c_void, _width: i32, _height: i32) -> Result<()> {
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        let drawable = win_surface::drawable_size(self.hwnd, width, height);
        if drawable.width == self.width && drawable.height == self.height {
            return Ok(());
        }
        self.release_rtv();
        if let Err(error) = unsafe {
            self.swap_chain.ResizeBuffers(
                0,
                drawable.width as u32,
                drawable.height as u32,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_SWAP_CHAIN_FLAG(0),
            )
        } {
            map_dxgi_resize_result(error.code())?;
        }
        self.logical_width = drawable.logical_width;
        self.logical_height = drawable.logical_height;
        self.width = drawable.width;
        self.height = drawable.height;
        self.create_rtv()
    }

    fn make_current(&mut self) -> Result<()> {
        self.bind_current_draw_target()
    }

    fn swap_buffers(&mut self, _damage: PresentDamage) -> Result<()> {
        self.present_result()
    }

    fn test_present(&mut self) -> Result<PresentTestResult> {
        // DXGI_PRESENT_TEST is the documented exit probe for an already-idle
        // bitblt swapchain. It submits no frame data and must use sync 0.
        map_dxgi_present_test_result(unsafe { self.swap_chain.Present(0, DXGI_PRESENT_TEST) })
    }

    fn try_shutdown(&mut self) -> Result<()> {
        self.shutdown_result()
    }

    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Result<Vec<u32>> {
        let x0 = x.clamp(0, self.width);
        let y0 = y.clamp(0, self.height);
        let x1 = x.saturating_add(width).clamp(x0, self.width);
        let y1 = y.saturating_add(height).clamp(y0, self.height);
        let read_w = x1 - x0;
        let read_h = y1 - y0;
        if read_w <= 0 || read_h <= 0 {
            return Ok(Vec::new());
        }

        (|| -> Result<Vec<u32>> {
            let back_buffer: ID3D11Texture2D = unsafe {
                self.swap_chain
                    .GetBuffer(0)
                    .map_err(|err| d3d_error("IDXGISwapChain::GetBuffer(read_pixels)", err))?
            };
            let desc = D3D11_TEXTURE2D_DESC {
                Width: self.width as u32,
                Height: self.height as u32,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };
            let mut staging = None;
            unsafe {
                self.device
                    .CreateTexture2D(&desc, None, Some(&mut staging))
                    .map_err(|err| d3d_error("ID3D11Device::CreateTexture2D(read_pixels)", err))?;
            }
            let staging = staging.ok_or_else(|| {
                Error::new(
                    Errc::PlatformError,
                    "D3d11Context: read_pixels staging texture was not created",
                )
            })?;
            unsafe {
                self.context.CopyResource(&staging, &back_buffer);
            }
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            unsafe {
                self.context
                    .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                    .map_err(|err| d3d_error("ID3D11DeviceContext::Map(read_pixels)", err))?;
            }
            let mut pixels = vec![0u32; (read_w as usize).saturating_mul(read_h as usize)];
            for row in 0..read_h as usize {
                let src = unsafe {
                    mapped
                        .pData
                        .cast::<u8>()
                        .add((y0 as usize + row) * mapped.RowPitch as usize + x0 as usize * 4)
                        .cast::<u32>()
                };
                let dst = pixels[row * read_w as usize..].as_mut_ptr();
                unsafe {
                    std::ptr::copy_nonoverlapping(src, dst, read_w as usize);
                }
            }
            unsafe {
                self.context.Unmap(&staging, 0);
            }
            Ok(pixels)
        })()
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

    fn clear_render_target(&mut self, r: f32, g: f32, b: f32, a: f32) -> Result<()> {
        self.bind_current_draw_target()?;
        if let Some(id) = self.bound_offscreen {
            let Some(Some(target)) = self.offscreens.get(id as usize) else {
                return Err(Error::new(
                    Errc::InvalidArgument,
                    format!("D3d11Context: clear_render_target unknown offscreen {id}"),
                ));
            };
            unsafe {
                self.context
                    .ClearRenderTargetView(&target.rtv, &[r, g, b, a]);
            }
            return Ok(());
        }
        let Some(rtv) = self.rtv.as_ref() else {
            return Err(Error::new(
                Errc::PlatformError,
                "D3d11Context: clear_render_target without RTV",
            ));
        };
        unsafe {
            self.context.ClearRenderTargetView(rtv, &[r, g, b, a]);
        }
        Ok(())
    }

    fn upload_surface_pixels(&mut self, pixels: &[u32], width: i32, height: i32) -> Result<()> {
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        if width != self.width || height != self.height {
            self.resize(width, height)?;
        }
        let expected = (width as usize).saturating_mul(height as usize);
        if pixels.len() < expected {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "D3d11Context: pixel buffer too small, got {}, need {expected}",
                    pixels.len()
                ),
            ));
        }
        self.ensure_rtv()?;
        let back_buffer: ID3D11Texture2D = unsafe {
            self.swap_chain
                .GetBuffer(0)
                .map_err(|err| d3d_error("IDXGISwapChain::GetBuffer", err))?
        };
        unsafe {
            self.context.UpdateSubresource(
                &back_buffer,
                0,
                None,
                pixels.as_ptr().cast(),
                (width as u32) * 4,
                0,
            );
        }
        Ok(())
    }

    fn draw_solid_rects(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuSolidRect],
    ) -> Result<()> {
        self.ensure_rtv()?;
        self.make_current()?;
        self.pipeline
            .draw_solid_rects(&self.context, viewport_w, viewport_h, scissor, rects)
    }

    fn draw_stroke_rects(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuStrokeRect],
    ) -> Result<()> {
        self.ensure_rtv()?;
        self.make_current()?;
        self.pipeline
            .draw_stroke_rects(&self.context, viewport_w, viewport_h, scissor, rects)
    }

    fn draw_glyphs(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        glyphs: &[GpuGlyphBlit],
    ) -> Result<()> {
        self.ensure_rtv()?;
        self.make_current()?;
        self.pipeline.draw_glyphs(
            &self.device,
            &self.context,
            viewport_w,
            viewport_h,
            scissor,
            glyphs,
        )
    }

    fn draw_linear_gradients(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuLinearGradientRect],
    ) -> Result<()> {
        self.ensure_rtv()?;
        self.make_current()?;
        self.pipeline
            .draw_linear_gradients(&self.context, viewport_w, viewport_h, scissor, rects)
    }

    fn draw_radial_gradients(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        grads: &[GpuRadialGradient],
    ) -> Result<()> {
        self.ensure_rtv()?;
        self.make_current()?;
        self.pipeline
            .draw_radial_gradients(&self.context, viewport_w, viewport_h, scissor, grads)
    }

    fn draw_solid_meshes(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        meshes: &[GpuSolidMesh],
    ) -> Result<()> {
        self.ensure_rtv()?;
        self.make_current()?;
        self.pipeline.draw_solid_meshes(
            &self.device,
            &self.context,
            viewport_w,
            viewport_h,
            scissor,
            meshes,
        )
    }

    fn draw_box_shadows(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        shadows: &[GpuBoxShadow],
    ) -> Result<()> {
        self.ensure_rtv()?;
        self.make_current()?;
        self.pipeline
            .draw_box_shadows(&self.context, viewport_w, viewport_h, scissor, shadows)
    }

    fn blit_soft_fallback(&mut self, pixels: &[u32], width: i32, height: i32) -> Result<()> {
        self.ensure_rtv()?;
        self.make_current()?;
        self.pipeline
            .blit_soft_fallback(&self.device, &self.context, pixels, width, height)
    }

    fn blit_soft_fallback_tile(&mut self, pixels: &[u32], tile: SoftFallbackTile) -> Result<()> {
        self.bind_current_draw_target()?;
        self.make_current()?;
        let (target_width, target_height) = self.current_target_size();
        self.pipeline.blit_soft_fallback_tile(
            &self.device,
            &self.context,
            pixels,
            target_width,
            target_height,
            tile,
        )
    }

    fn clear_rects(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        rects: &[GpuSolidRect],
    ) -> Result<()> {
        self.bind_current_draw_target()?;
        self.pipeline
            .clear_rects(&self.context, viewport_w, viewport_h, rects)
    }

    fn create_offscreen_target(
        &mut self,
        width: i32,
        height: i32,
    ) -> Result<OffscreenTargetId, Error> {
        let w = width.max(1);
        let h = height.max(1);
        let desc = D3D11_TEXTURE2D_DESC {
            Width: w as u32,
            Height: h as u32,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut texture = None;
        unsafe {
            self.device
                .CreateTexture2D(&desc, None, Some(&mut texture))
                .map_err(|err| d3d_error("CreateTexture2D(offscreen)", err))?;
        }
        let texture = texture.ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "D3d11Context: CreateTexture2D(offscreen) returned no texture",
            )
        })?;
        let mut rtv = None;
        unsafe {
            self.device
                .CreateRenderTargetView(&texture, None, Some(&mut rtv))
                .map_err(|err| d3d_error("CreateRenderTargetView(offscreen)", err))?;
        }
        let rtv = rtv.ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "D3d11Context: CreateRenderTargetView(offscreen) returned no RTV",
            )
        })?;
        let mut srv = None;
        unsafe {
            self.device
                .CreateShaderResourceView(&texture, None, Some(&mut srv))
                .map_err(|err| d3d_error("CreateShaderResourceView(offscreen)", err))?;
        }
        let srv = srv.ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "D3d11Context: CreateShaderResourceView(offscreen) returned no SRV",
            )
        })?;

        let id = if let Some(id) = self.free_offscreen_ids.pop() {
            id
        } else {
            let id = self.next_offscreen_id;
            self.next_offscreen_id = self.next_offscreen_id.saturating_add(1);
            id
        };
        let idx = id as usize;
        while self.offscreens.len() <= idx {
            self.offscreens.push(None);
        }
        self.offscreens[idx] = Some(OffscreenTarget {
            texture,
            rtv,
            srv,
            width: w,
            height: h,
        });
        Ok(OffscreenTargetId(id))
    }

    fn try_destroy_offscreen_target(&mut self, id: OffscreenTargetId) -> Result<(), Error> {
        // Do this for every checked destroy, not only when our cached marker
        // says the target is bound: a failed previous restore leaves the real
        // D3D target unknown. Do not release the offscreen slot until the
        // swapchain target is confirmed, so callers can retry safely.
        self.bind_swapchain_target()?;
        let idx = id.0 as usize;
        if idx < self.offscreens.len() && self.offscreens[idx].take().is_some() {
            self.free_offscreen_ids.push(id.0);
        }
        Ok(())
    }

    fn destroy_offscreen_target(&mut self, id: OffscreenTargetId) {
        if let Err(error) = self.try_destroy_offscreen_target(id) {
            crate::core::log::error_fn(format!(
                "D3d11Context: destroy offscreen target failed: {}",
                error.short_what()
            ));
        }
    }

    fn bind_offscreen_target(&mut self, id: OffscreenTargetId) -> Result<(), Error> {
        let idx = id.0 as usize;
        if self.offscreens.get(idx).and_then(|o| o.as_ref()).is_none() {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!("D3d11Context: bind_offscreen_target unknown id {}", id.0),
            ));
        }
        self.bound_offscreen = Some(id.0);
        self.bind_current_draw_target()
    }

    fn bind_swapchain_target(&mut self) -> Result<(), Error> {
        self.bound_offscreen = None;
        self.bind_current_draw_target()
    }

    fn blit_offscreen_target(
        &mut self,
        id: OffscreenTargetId,
        src: crate::core::Rect,
        dst: crate::core::Rect,
    ) -> Result<(), Error> {
        let idx = id.0 as usize;
        let Some(Some(target)) = self.offscreens.get(idx) else {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!("D3d11Context: blit_offscreen_target unknown id {}", id.0),
            ));
        };
        if self.bound_offscreen == Some(id.0) {
            return Err(Error::new(
                Errc::InvalidState,
                "D3d11Context: cannot blit offscreen while it is the bound RT",
            ));
        }
        let srv = target.srv.clone();
        let source_w = target.width as f32;
        let source_h = target.height as f32;
        let (tw, th) = self.current_target_size();
        self.bind_current_draw_target()?;
        self.pipeline.blit_srv_to_rect(
            &self.context,
            &srv,
            source_w,
            source_h,
            tw as f32,
            th as f32,
            src,
            dst,
        )
    }

    fn present(&mut self, frame: &PresentFrame<'_>) -> Result<()> {
        match frame {
            PresentFrame::Swapchain { .. } => {
                self.bind_swapchain_target()?;
                self.present_result()
            }
            PresentFrame::PixelBuffer {
                pixels,
                width,
                height,
                damage,
            } => self.present_pixels(pixels, *width, *height, damage.clone()),
        }
    }

    /// Kept for low-level tests; not advertised via caps (`PresentMode::Swapchain`).
    fn present_pixels(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        _damage: PresentDamage,
    ) -> Result<()> {
        self.upload_surface_pixels(pixels, width, height)?;
        self.present_result()
    }
}
