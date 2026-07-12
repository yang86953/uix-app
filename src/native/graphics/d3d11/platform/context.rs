//! Direct3D 11 graphics context for Windows.
//!
//! Caps: [`RasterMode::GpuNative`] × [`PresentMode::Swapchain`] (#169).
//! Native solid/rounded fill + stroke + glyph atlas text + linear/radial
//! gradients + simple path meshes + box/ambient shadow; unsupported Canvas2D
//! ops soft-raster and alpha-blit (same hybrid pattern as the native GL path).

#![allow(nonstandard_style)]

use std::ffi::c_void;

use super::pipeline::D3d11Pipeline;
use crate::core::{Errc, Error, Result};
use crate::native::graphics::platform::windows as win_surface;
use crate::native::traits::present::{
    GpuBoxShadow, GpuGlyphBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSolidMesh,
    GpuSolidRect, GpuStrokeRect, GraphicsBackend, GraphicsContextCaps, IGraphicsContext,
    NativeRasterCaps, OffscreenTargetId, PresentDamage, PresentFrame, SoftFallbackTile,
};
use ::windows::Win32::Foundation::{E_OUTOFMEMORY, HMODULE, HWND, TRUE};
use ::windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL,
    D3D_FEATURE_LEVEL_10_0, D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
};
use ::windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CPU_ACCESS_READ,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING, D3D11_VIEWPORT,
    D3D11CreateDeviceAndSwapChain, ID3D11Device, ID3D11DeviceContext, ID3D11RenderTargetView,
    ID3D11ShaderResourceView, ID3D11Texture2D,
};
use ::windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_MODE_DESC, DXGI_MODE_SCALING_UNSPECIFIED,
    DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED, DXGI_RATIONAL, DXGI_SAMPLE_DESC,
};
use ::windows::Win32::Graphics::Dxgi::{
    DXGI_ERROR_DEVICE_HUNG, DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_DEVICE_RESET,
    DXGI_ERROR_DRIVER_INTERNAL_ERROR, DXGI_ERROR_REMOTE_OUTOFMEMORY, DXGI_PRESENT,
    DXGI_SWAP_CHAIN_DESC, DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_DISCARD,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIDevice, IDXGISwapChain,
};
use ::windows::core::Interface;

type HWND_PTR = *mut c_void;

struct OffscreenTarget {
    #[allow(dead_code)] // kept alive for RTV/SRV; not read directly after create
    texture: ID3D11Texture2D,
    rtv: ID3D11RenderTargetView,
    srv: ID3D11ShaderResourceView,
    width: i32,
    height: i32,
}

const D3D11_FEATURE_LEVELS: [D3D_FEATURE_LEVEL; 4] = [
    D3D_FEATURE_LEVEL_11_1,
    D3D_FEATURE_LEVEL_11_0,
    D3D_FEATURE_LEVEL_10_1,
    D3D_FEATURE_LEVEL_10_0,
];

fn swap_chain_desc(hwnd: HWND_PTR, width: i32, height: i32) -> DXGI_SWAP_CHAIN_DESC {
    DXGI_SWAP_CHAIN_DESC {
        BufferDesc: DXGI_MODE_DESC {
            Width: width.max(1) as u32,
            Height: height.max(1) as u32,
            RefreshRate: DXGI_RATIONAL {
                Numerator: 60,
                Denominator: 1,
            },
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            ScanlineOrdering: DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED,
            Scaling: DXGI_MODE_SCALING_UNSPECIFIED,
        },
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        OutputWindow: HWND(hwnd),
        Windowed: TRUE,
        SwapEffect: DXGI_SWAP_EFFECT_DISCARD,
        Flags: 0,
    }
}

fn d3d_hresult_code(result: ::windows::core::HRESULT) -> Errc {
    match result {
        DXGI_ERROR_DEVICE_HUNG
        | DXGI_ERROR_DEVICE_REMOVED
        | DXGI_ERROR_DEVICE_RESET
        | DXGI_ERROR_DRIVER_INTERNAL_ERROR => Errc::GraphicsDeviceLost,
        E_OUTOFMEMORY | DXGI_ERROR_REMOTE_OUTOFMEMORY => Errc::GraphicsOutOfMemory,
        _ => Errc::PlatformError,
    }
}

fn d3d_error(operation: &str, err: ::windows::core::Error) -> Error {
    Error::new(
        d3d_hresult_code(err.code()),
        format!("D3d11Context: {operation} failed: {err}"),
    )
}

fn map_dxgi_present_result(result: ::windows::core::HRESULT) -> Result<()> {
    if result.is_err() {
        return Err(Error::new(
            d3d_hresult_code(result),
            format!("D3d11Context: IDXGISwapChain::Present failed: {result:?}"),
        ));
    }
    Ok(())
}

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
    context: ID3D11DeviceContext,
    swap_chain: IDXGISwapChain,
    rtv: Option<ID3D11RenderTargetView>,
    pipeline: D3d11Pipeline,
    adapter_info: D3d11AdapterInfo,
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
    pub fn new(native_window: *mut c_void, width: i32, height: i32) -> Result<Self> {
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
    #[cfg(test)]
    pub(super) fn new_warp_test_context(
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

fn create_with_driver(
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
            false,
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
        unsafe {
            self.swap_chain.ResizeBuffers(
                0,
                drawable.width as u32,
                drawable.height as u32,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_SWAP_CHAIN_FLAG(0),
            )
        }
        .map_err(|err| {
            Error::new(
                Errc::PlatformError,
                format!("D3d11Context: ResizeBuffers failed: {err}"),
            )
        })?;
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

    fn shutdown(&mut self) {
        self.bound_offscreen = None;
        self.offscreens.clear();
        self.free_offscreen_ids.clear();
        self.next_offscreen_id = 0;
        self.release_rtv();
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

    fn destroy_offscreen_target(&mut self, id: OffscreenTargetId) {
        if self.bound_offscreen == Some(id.0) {
            self.bound_offscreen = None;
            let _ = self.bind_swapchain_target();
        }
        let idx = id.0 as usize;
        if idx < self.offscreens.len() && self.offscreens[idx].take().is_some() {
            self.free_offscreen_ids.push(id.0);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::traits::present::{
        GpuBoxShadow, GpuSolidMesh, PresentFrame, PresentMode, RasterMode,
    };

    #[test]
    fn swap_chain_desc_uses_bgra_windowed_backbuffer() {
        let desc = swap_chain_desc(std::ptr::dangling_mut::<c_void>(), 800, 600);

        assert_eq!(desc.BufferDesc.Width, 800);
        assert_eq!(desc.BufferDesc.Height, 600);
        assert_eq!(desc.BufferDesc.Format, DXGI_FORMAT_B8G8R8A8_UNORM);
        assert_eq!(desc.SampleDesc.Count, 1);
        assert_eq!(desc.BufferCount, 2);
        assert_eq!(desc.BufferUsage, DXGI_USAGE_RENDER_TARGET_OUTPUT);
        assert_eq!(desc.Windowed, TRUE);
    }

    #[test]
    fn d3d11_rejects_null_hwnd() {
        let err = match D3d11Context::new(std::ptr::null_mut(), 640, 480) {
            Ok(_) => panic!("expected null hwnd to fail"),
            Err(err) => err,
        };
        assert_eq!(err.code(), Errc::PlatformError);
    }

    #[test]
    fn dxgi_present_device_removed_is_a_typed_device_failure() {
        let error = map_dxgi_present_result(::windows::core::HRESULT(0x887A_0005u32 as i32))
            .expect_err("DXGI present failure must propagate");

        assert_eq!(error.code(), Errc::GraphicsDeviceLost);
        assert!(error.what().contains("IDXGISwapChain::Present"));
    }

    #[test]
    fn dxgi_present_out_of_memory_is_typed() {
        let error = map_dxgi_present_result(::windows::core::HRESULT(0x8007_000Eu32 as i32))
            .expect_err("DXGI out-of-memory must propagate");

        assert_eq!(error.code(), Errc::GraphicsOutOfMemory);
    }

    #[test]
    fn factory_create_d3d11_gpu_native_swapchain_on_real_window() {
        let mut platform = crate::native::create_platform().expect("platform");
        let window = platform
            .window_manager()
            .create_window("D3D11 GPU native test", 320, 240)
            .expect("window");
        let surface = window.native_surface_ptr();
        assert!(
            !surface.is_null(),
            "Windows HWND must be exposed as native_surface_ptr"
        );

        let mut ctx = D3d11Context::new(surface, 320, 240).expect("D3d11Context");
        assert_eq!(ctx.graphics_backend(), GraphicsBackend::D3d11);
        let adapter_info = &ctx.adapter_info;
        assert_ne!(adapter_info.description, "unavailable");
        assert!(!adapter_info.description.trim().is_empty());
        assert_ne!(adapter_info.vendor_id, 0);
        assert_ne!(adapter_info.device_id, 0);
        println!(
            "D3D11 real-window adapter: {}",
            adapter_info.diagnostic_summary()
        );
        let caps = ctx.caps();
        assert_eq!(caps.raster, RasterMode::GpuNative);
        assert_eq!(caps.present, PresentMode::Swapchain);
        assert!(!ctx.supports_pixel_present());
        assert!(!ctx.supports_gl_proc_address());

        assert_eq!(ctx.native_raster_caps(), NativeRasterCaps::d3d11_full());
        ctx.clear_render_target(0.1, 0.2, 0.3, 1.0)
            .expect("clear_render_target");
        ctx.draw_solid_rects(
            ctx.width() as f32,
            ctx.height() as f32,
            None,
            &[GpuSolidRect {
                x: 16.0,
                y: 24.0,
                w: 80.0,
                h: 40.0,
                rgba: [1.0, 0.2, 0.1, 1.0],
                radius: [8.0, 8.0, 8.0, 8.0],
            }],
        )
        .expect("draw_solid_rects");
        ctx.draw_stroke_rects(
            ctx.width() as f32,
            ctx.height() as f32,
            None,
            &[
                GpuStrokeRect {
                    x: 20.0,
                    y: 80.0,
                    w: 100.0,
                    h: 48.0,
                    rgba: [0.1, 0.8, 1.0, 1.0],
                    radius: [6.0, 6.0, 6.0, 6.0],
                    line_width: 2.0,
                },
                GpuStrokeRect {
                    x: 200.0,
                    y: 40.0,
                    w: 64.0,
                    h: 64.0,
                    rgba: [1.0, 1.0, 0.2, 1.0],
                    radius: [32.0, 32.0, 32.0, 32.0],
                    line_width: 3.0,
                },
            ],
        )
        .expect("draw_stroke_rects");
        // Glyph atlas: solid-color coverage blit (identity text path).
        let cov = std::sync::Arc::<[u8]>::from(vec![255u8; 8 * 8]);
        ctx.draw_glyphs(
            ctx.width() as f32,
            ctx.height() as f32,
            None,
            &[GpuGlyphBlit {
                x: 40.0,
                y: 160.0,
                w: 8.0,
                h: 8.0,
                rgba: [1.0, 1.0, 1.0, 1.0],
                coverage: cov,
                cov_w: 8,
                cov_h: 8,
            }],
        )
        .expect("draw_glyphs");
        ctx.draw_linear_gradients(
            ctx.width() as f32,
            ctx.height() as f32,
            None,
            &[GpuLinearGradientRect {
                x: 120.0,
                y: 16.0,
                w: 80.0,
                h: 24.0,
                color_a: [1.0, 0.0, 0.0, 1.0],
                color_b: [0.0, 0.0, 1.0, 1.0],
                dir: 0,
            }],
        )
        .expect("draw_linear_gradients");
        ctx.draw_radial_gradients(
            ctx.width() as f32,
            ctx.height() as f32,
            None,
            &[GpuRadialGradient {
                cx: 260.0,
                cy: 180.0,
                inner_r: 4.0,
                outer_r: 28.0,
                color_inner: [1.0, 1.0, 0.0, 1.0],
                color_outer: [0.0, 0.5, 0.0, 0.0],
            }],
        )
        .expect("draw_radial_gradients");
        // Simple triangle mesh (CPU-tessellated path fill).
        ctx.draw_solid_meshes(
            ctx.width() as f32,
            ctx.height() as f32,
            None,
            &[GpuSolidMesh {
                vertices: std::sync::Arc::<[f32]>::from(vec![
                    180.0, 80.0, 220.0, 80.0, 200.0, 120.0,
                ]),
                rgba: [0.2, 1.0, 0.4, 1.0],
            }],
        )
        .expect("draw_solid_meshes");
        ctx.draw_box_shadows(
            ctx.width() as f32,
            ctx.height() as f32,
            None,
            &[
                GpuBoxShadow {
                    x: 40.0,
                    y: 100.0,
                    w: 64.0,
                    h: 32.0,
                    offset_x: 4.0,
                    offset_y: 6.0,
                    blur: 8.0,
                    rgba: [0.0, 0.0, 0.0, 0.45],
                    radius: [6.0, 6.0, 6.0, 6.0],
                    ambient: false,
                },
                GpuBoxShadow {
                    x: 200.0,
                    y: 100.0,
                    w: 48.0,
                    h: 48.0,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    blur: 12.0,
                    rgba: [0.0, 0.0, 0.0, 0.3],
                    radius: [24.0, 24.0, 24.0, 24.0],
                    ambient: true,
                },
            ],
        )
        .expect("draw_box_shadows");
        // Soft overlay (transparent except one opaque pixel region via alpha).
        let mut soft = vec![0u32; (ctx.width() * ctx.height()) as usize];
        soft[0] = 0xFF00_FF00; // opaque green BGRA
        ctx.blit_soft_fallback(&soft, ctx.width(), ctx.height())
            .expect("blit_soft_fallback");
        let pixels = ctx
            .read_pixels(0, 0, ctx.width(), ctx.height())
            .expect("hardware D3D11 readback");
        assert_eq!(pixels.len(), (ctx.width() * ctx.height()) as usize);
        assert_eq!(pixels[0], 0xFF00_FF00);
        assert!(pixels.iter().any(|pixel| *pixel != pixels[0]));
        ctx.present(&PresentFrame::Swapchain {
            damage: PresentDamage::Full,
        })
        .expect("swapchain present");
        ctx.shutdown();
        drop(ctx);

        let mut warp_ctx = create_with_driver(
            surface,
            320,
            240,
            &D3D11_FEATURE_LEVELS,
            D3d11DriverKind::Warp,
        )
        .expect("D3D11 WARP context");
        assert_eq!(warp_ctx.adapter_info.driver, D3d11DriverKind::Warp);
        assert_ne!(warp_ctx.adapter_info.description, "unavailable");
        println!(
            "D3D11 WARP real-window adapter: {}",
            warp_ctx.adapter_info.diagnostic_summary()
        );

        warp_ctx
            .clear_render_target(0.25, 0.5, 0.75, 1.0)
            .expect("clear WARP render target");
        let pixels = warp_ctx
            .read_pixels(0, 0, warp_ctx.width(), warp_ctx.height())
            .expect("WARP D3D11 readback");
        assert_eq!(
            pixels.len(),
            (warp_ctx.width() * warp_ctx.height()) as usize
        );
        assert_eq!(pixels[0] >> 24, 0xFF);
        assert_ne!(pixels[0] & 0x00FF_FFFF, 0);
        warp_ctx
            .present(&PresentFrame::Swapchain {
                damage: PresentDamage::Full,
            })
            .expect("WARP swapchain present");
        warp_ctx.shutdown();
    }

    #[test]
    fn d3d11_soft_blit_ignores_a_previous_native_scissor() {
        let mut platform = crate::native::create_platform().expect("platform");
        let window = platform
            .window_manager()
            .create_window("D3D11 soft scissor", 96, 64)
            .expect("window");
        let mut ctx = D3d11Context::new(window.native_surface_ptr(), 96, 64).expect("context");
        ctx.clear_render_target(0.0, 0.0, 0.0, 1.0)
            .expect("clear target");
        ctx.draw_solid_rects(
            96.0,
            64.0,
            Some((24, 16, 32, 24)),
            &[GpuSolidRect {
                x: 24.0,
                y: 16.0,
                w: 32.0,
                h: 24.0,
                rgba: [1.0, 1.0, 1.0, 1.0],
                radius: [0.0; 4],
            }],
        )
        .expect("draw clipped native rect");

        let mut soft = vec![0u32; 96 * 64];
        soft[2 * 96 + 2] = 0xFF00_FF00; // BGRA opaque green, outside the native scissor.
        ctx.blit_soft_fallback(&soft, 96, 64)
            .expect("blit full soft surface");

        assert_eq!(
            ctx.read_pixels(2, 2, 1, 1).expect("soft-scissor readback"),
            vec![0xFF00_FF00],
            "soft fallback must not inherit the previous native draw scissor"
        );
        ctx.shutdown();
    }

    #[test]
    fn d3d11_soft_blit_does_not_resample_a_prior_partial_segment() {
        let mut platform = crate::native::create_platform().expect("platform");
        let window = platform
            .window_manager()
            .create_window("D3D11 WARP soft damage", 64, 48)
            .expect("window");
        let mut ctx = create_with_driver(
            window.native_surface_ptr(),
            64,
            48,
            &D3D11_FEATURE_LEVELS,
            D3d11DriverKind::Warp,
        )
        .expect("D3D11 WARP context");
        ctx.clear_render_target(0.0, 0.0, 0.0, 0.0)
            .expect("clear transparent target");

        ctx.blit_soft_fallback_tile(&[0x80FF_0000], SoftFallbackTile::at_destination(4, 5, 1, 1))
            .expect("first compact soft segment");
        let first_before = ctx
            .read_pixels(4, 5, 1, 1)
            .expect("first soft segment readback")[0];
        assert_ne!(first_before, 0, "first segment must reach the target");

        ctx.blit_soft_fallback_tile(
            &[0x8000_00FF],
            SoftFallbackTile::at_destination(36, 19, 1, 1),
        )
        .expect("second compact soft segment");

        assert_eq!(
            ctx.read_pixels(4, 5, 1, 1)
                .expect("first soft segment readback")[0],
            first_before,
            "a later partial upload must not re-blend stale texture data"
        );
        assert_ne!(
            ctx.read_pixels(36, 19, 1, 1)
                .expect("second soft segment readback")[0],
            0,
            "second segment must reach its own target pixel"
        );
        ctx.shutdown();
    }

    #[test]
    fn d3d11_resize_grows_backbuffer_and_fills_far_corner() {
        let mut platform = crate::native::create_platform().expect("platform");
        let mut window = platform
            .window_manager()
            .create_window("D3D11 resize grow", 320, 240)
            .expect("window");
        let surface = window.native_surface_ptr();
        let mut ctx = D3d11Context::new(surface, 320, 240).expect("D3d11Context");
        let initial_drawable = win_surface::drawable_size(surface, 320, 240);
        assert_eq!(
            (ctx.width(), ctx.height()),
            (initial_drawable.width, initial_drawable.height)
        );

        // Grow the HWND; D3D11 resize reads GetClientRect.
        window
            .properties_mut()
            .set_size(900, 700)
            .expect("set_size");
        // Pump messages so WM_SIZE updates the client rect before GetClientRect.
        let _ = platform.event_loop().poll_event(&|_| true);

        let drawable = win_surface::drawable_size(surface, 1, 1);
        assert!(
            drawable.logical_width > 320 && drawable.logical_height > 240,
            "client should grow after set_size, got {}x{}",
            drawable.logical_width,
            drawable.logical_height
        );

        let (cw, ch) = (drawable.width, drawable.height);
        ctx.resize(drawable.logical_width, drawable.logical_height)
            .expect("resize after client-size change");
        assert_eq!(
            (ctx.width(), ctx.height()),
            (cw, ch),
            "D3D11 context must adopt the shared physical drawable extent after resize"
        );
        assert!(
            (ctx.caps().device_pixel_ratio - drawable.width as f32 / drawable.logical_width as f32)
                .abs()
                < f32::EPSILON,
            "D3D11 caps must report the drawable-to-logical DPR"
        );

        ctx.clear_render_target(1.0, 0.0, 0.0, 1.0)
            .expect("clear full RT after resize");
        let px = ctx
            .read_pixels(cw - 2, ch - 2, 1, 1)
            .expect("far-corner readback");
        assert_eq!(px.len(), 1, "far-corner readback");
        assert_eq!(
            px[0] >> 24,
            0xFF,
            "far corner must be opaque after clear; got {:#010X}",
            px[0]
        );
        assert_ne!(
            px[0] & 0x00FF_FFFF,
            0,
            "far corner must not stay black after resize clear"
        );

        ctx.present(&PresentFrame::Swapchain {
            damage: PresentDamage::Full,
        })
        .expect("present after resize");
        ctx.shutdown();
    }

    #[test]
    fn d3d11_offscreen_target_create_bind_clear_blit_destroy() {
        let mut platform = crate::native::create_platform().expect("platform");
        let window = platform
            .window_manager()
            .create_window("D3D11 offscreen RT", 160, 120)
            .expect("window");
        let mut ctx = D3d11Context::new(window.native_surface_ptr(), 160, 120).expect("ctx");
        assert!(
            ctx.native_raster_caps().offscreen_targets,
            "D3D11 advertises Picture offscreen only after crop/scissor semantics are fixed"
        );

        let id = ctx
            .create_offscreen_target(32, 24)
            .expect("create_offscreen_target");
        ctx.bind_offscreen_target(id).expect("bind offscreen");
        ctx.clear_render_target(1.0, 0.0, 0.0, 1.0)
            .expect("clear offscreen");
        ctx.bind_swapchain_target().expect("bind swapchain");
        ctx.clear_render_target(0.0, 0.0, 0.0, 1.0)
            .expect("clear swapchain");
        ctx.blit_offscreen_target(
            id,
            crate::core::Rect::new(0.0, 0.0, 32.0, 24.0),
            crate::core::Rect::new(8.0, 8.0, 32.0, 24.0),
        )
        .expect("blit offscreen");
        ctx.destroy_offscreen_target(id);
        ctx.present(&PresentFrame::Swapchain {
            damage: PresentDamage::Full,
        })
        .expect("present");
        ctx.shutdown();
    }

    #[test]
    fn d3d11_warp_offscreen_crop_ignores_and_restores_previous_raster_state() {
        let mut platform = crate::native::create_platform().expect("platform");
        let window = platform
            .window_manager()
            .create_window("D3D11 WARP offscreen crop", 96, 64)
            .expect("window");
        let mut ctx = create_with_driver(
            window.native_surface_ptr(),
            96,
            64,
            &D3D11_FEATURE_LEVELS,
            D3d11DriverKind::Warp,
        )
        .expect("D3D11 WARP context");
        assert_eq!(ctx.adapter_info.driver, D3d11DriverKind::Warp);
        assert!(ctx.native_raster_caps().offscreen_targets);

        let offscreen = ctx
            .create_offscreen_target(64, 48)
            .expect("create offscreen target");
        ctx.bind_offscreen_target(offscreen)
            .expect("bind offscreen target");
        ctx.clear_render_target(0.0, 0.0, 1.0, 1.0)
            .expect("clear source blue");
        ctx.draw_solid_rects(
            64.0,
            48.0,
            None,
            &[GpuSolidRect {
                x: 16.0,
                y: 8.0,
                w: 16.0,
                h: 16.0,
                rgba: [1.0, 0.0, 0.0, 1.0],
                radius: [0.0; 4],
            }],
        )
        .expect("paint source crop red");

        ctx.bind_swapchain_target().expect("bind swapchain target");
        ctx.clear_render_target(0.0, 0.0, 0.0, 1.0)
            .expect("clear swapchain black");
        ctx.draw_solid_rects(
            96.0,
            64.0,
            Some((0, 0, 4, 4)),
            &[GpuSolidRect {
                x: 0.0,
                y: 0.0,
                w: 96.0,
                h: 64.0,
                rgba: [0.0, 0.0, 0.0, 1.0],
                radius: [0.0; 4],
            }],
        )
        .expect("preset narrow scissor");

        ctx.blit_offscreen_target(
            offscreen,
            crate::core::Rect::new(16.0, 8.0, 16.0, 16.0),
            crate::core::Rect::new(48.0, 24.0, 16.0, 16.0),
        )
        .expect("blit nonzero source crop");

        assert_eq!(
            ctx.read_pixels(55, 31, 1, 1)
                .expect("offscreen crop readback"),
            vec![0xFFFF_0000],
            "the destination outside the old narrow scissor must sample the requested red source crop"
        );

        let mut viewport_count = 1;
        let mut viewport = D3D11_VIEWPORT::default();
        let mut scissor_count = 1;
        let mut scissor = ::windows::Win32::Foundation::RECT::default();
        unsafe {
            ctx.context
                .RSGetViewports(&mut viewport_count, Some(&mut viewport));
            ctx.context
                .RSGetScissorRects(&mut scissor_count, Some(&mut scissor));
        }
        assert_eq!(viewport_count, 1);
        assert_eq!(
            (
                viewport.TopLeftX,
                viewport.TopLeftY,
                viewport.Width,
                viewport.Height
            ),
            (0.0, 0.0, 96.0, 64.0),
            "offscreen blit must restore the caller viewport"
        );
        assert_eq!(scissor_count, 1);
        assert_eq!(
            (scissor.left, scissor.top, scissor.right, scissor.bottom),
            (0, 0, 4, 4),
            "offscreen blit must restore the caller scissor"
        );

        ctx.destroy_offscreen_target(offscreen);
        ctx.shutdown();
    }

    #[test]
    fn repeated_present_keeps_single_rtv_and_stays_drawable() {
        // Regression: Present while holding RTV caused DXGI to allocate extra
        // buffers (memory growth). Release-before-Present + recreate-after must
        // keep drawing stable across many frames.
        let mut platform = crate::native::create_platform().expect("platform");
        let window = platform
            .window_manager()
            .create_window("D3D11 present memory", 160, 120)
            .expect("window");
        let surface = window.native_surface_ptr();
        let mut ctx = D3d11Context::new(surface, 160, 120).expect("D3d11Context");
        for i in 0..64 {
            let t = (i as f32) / 64.0;
            ctx.clear_render_target(t, 0.2, 1.0 - t, 1.0)
                .expect("clear");
            ctx.present(&PresentFrame::Swapchain {
                damage: PresentDamage::Full,
            })
            .expect("present");
            assert!(
                ctx.rtv.is_some(),
                "RTV must be recreated after Present for the next frame"
            );
        }
        let pixels = ctx
            .read_pixels(0, 0, 1, 1)
            .expect("repeated present readback");
        assert_eq!(pixels.len(), 1);
        assert_eq!(pixels[0] >> 24, 0xFF, "final frame must remain opaque");
        ctx.shutdown();
    }

    #[test]
    fn dual_hwnd_theme_palette_clear_present_and_readback() {
        // P6 joint slice: two real HWNDs × D3D11 × light/dark layout palette
        // clear + Present + readback. Theme broadcast itself is covered by
        // FakePlatform; this closes the D3D11 multi-window present gap.
        let mut platform = crate::native::create_platform().expect("platform");
        let primary = platform
            .window_manager()
            .create_window("D3D11 theme primary", 160, 120)
            .expect("primary window");
        let secondary = platform
            .window_manager()
            .create_window("D3D11 theme secondary", 160, 120)
            .expect("secondary window");

        let mut ctx_a =
            D3d11Context::new(primary.native_surface_ptr(), 160, 120).expect("primary D3D11");
        let mut ctx_b =
            D3d11Context::new(secondary.native_surface_ptr(), 160, 120).expect("secondary D3D11");

        // Mirrors ThemePrimitives antd light/dark `color_bg_layout` (native
        // must not import `ui::Theme`).
        let light = (247.0 / 255.0, 247.0 / 255.0, 248.0 / 255.0, 1.0);
        let dark = (20.0 / 255.0, 20.0 / 255.0, 20.0 / 255.0, 1.0);

        let sample = |ctx: &mut D3d11Context, rgba: (f32, f32, f32, f32)| -> u32 {
            ctx.clear_render_target(rgba.0, rgba.1, rgba.2, rgba.3)
                .expect("clear");
            // Read before Present: DXGI_SWAP_EFFECT_DISCARD may drop contents.
            let pixels = ctx.read_pixels(0, 0, 1, 1).expect("theme palette readback");
            assert_eq!(pixels.len(), 1);
            ctx.present(&PresentFrame::Swapchain {
                damage: PresentDamage::Full,
            })
            .expect("present");
            assert!(ctx.rtv.is_some(), "RTV must exist after Present");
            pixels[0]
        };

        let a_light = sample(&mut ctx_a, light);
        let b_light = sample(&mut ctx_b, light);
        assert_eq!(a_light, b_light, "both windows must share light palette");
        assert_eq!(a_light >> 24, 0xFF);

        let a_dark = sample(&mut ctx_a, dark);
        let b_dark = sample(&mut ctx_b, dark);
        assert_eq!(a_dark, b_dark, "both windows must share dark palette");
        assert_ne!(
            a_light & 0x00FF_FFFF,
            a_dark & 0x00FF_FFFF,
            "light and dark layout palettes must differ on GPU"
        );

        ctx_a.shutdown();
        ctx_b.shutdown();
    }
}
