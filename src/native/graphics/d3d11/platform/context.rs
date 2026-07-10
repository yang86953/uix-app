//! Direct3D 11 graphics context for Windows.
//!
//! Caps: [`RasterMode::GpuNative`] × [`PresentMode::Swapchain`] (#169).
//! Native solid/rounded fill + stroke + glyph atlas text + linear/radial
//! gradients + simple path meshes + box/ambient shadow; unsupported Canvas2D
//! ops soft-raster and alpha-blit (same hybrid pattern as GL `GpuCanvas2D`).

#![allow(nonstandard_style)]

use std::ffi::c_void;

use super::pipeline::D3d11Pipeline;
use crate::core::{Errc, Error, Result};
use crate::native::graphics::platform::windows as win_surface;
use crate::native::traits::present::{
    GpuBoxShadow, GpuGlyphBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSolidMesh,
    GpuSolidRect, GpuStrokeRect, GraphicsBackend, GraphicsContextCaps, IGraphicsContext,
    PresentDamage,
};
use ::windows::core::Interface;
use ::windows::Win32::Foundation::{HMODULE, HWND, TRUE};
use ::windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL,
    D3D_FEATURE_LEVEL_10_0, D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
};
use ::windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDeviceAndSwapChain, ID3D11Device, ID3D11DeviceContext, ID3D11RenderTargetView,
    ID3D11Texture2D, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_STAGING, D3D11_VIEWPORT,
};
use ::windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_MODE_DESC, DXGI_MODE_SCALING_UNSPECIFIED,
    DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED, DXGI_RATIONAL, DXGI_SAMPLE_DESC,
};
use ::windows::Win32::Graphics::Dxgi::{
    IDXGIDevice, IDXGISwapChain, DXGI_PRESENT, DXGI_SWAP_CHAIN_DESC, DXGI_SWAP_CHAIN_FLAG,
    DXGI_SWAP_EFFECT_DISCARD, DXGI_USAGE_RENDER_TARGET_OUTPUT,
};

type HWND_PTR = *mut c_void;

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

fn d3d_error(operation: &str, err: ::windows::core::Error) -> Error {
    Error::new(
        Errc::PlatformError,
        format!("D3d11Context: {operation} failed: {err}"),
    )
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
    width: i32,
    height: i32,
}

impl D3d11Context {
    pub fn new(native_window: *mut c_void, width: i32, height: i32) -> Result<Self> {
        if native_window.is_null() {
            return Err(Error::new(
                Errc::PlatformError,
                "D3d11Context: native window handle is null",
            ));
        }
        let (client_w, client_h) = win_surface::client_size(native_window, width, height);
        match create_with_driver(
            native_window,
            client_w,
            client_h,
            &D3D11_FEATURE_LEVELS,
            D3d11DriverKind::Hardware,
        ) {
            Ok(context) => Ok(context),
            Err(hardware_error) => {
                crate::core::log::warn_fn(format!(
                    "D3d11Context: hardware device unavailable; retrying with WARP: {}",
                    hardware_error.what()
                ));
                create_with_driver(
                    native_window,
                    client_w,
                    client_h,
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
        let vp = D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: self.width.max(1) as f32,
            Height: self.height.max(1) as f32,
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
        width,
        height,
    };
    crate::core::log::info_fn(format!(
        "D3d11Context: created {width}x{height} swapchain at feature level {:?}; {}",
        selected_level,
        ctx.adapter_info.diagnostic_summary()
    ));
    ctx.create_rtv()?;
    Ok(ctx)
}

impl IGraphicsContext for D3d11Context {
    fn caps(&self) -> crate::native::traits::present::GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(GraphicsBackend::D3d11, false, 1.0)
    }

    fn graphics_backend(&self) -> GraphicsBackend {
        GraphicsBackend::D3d11
    }

    fn initialize(&mut self, _native_window: *mut c_void, _width: i32, _height: i32) -> Result<()> {
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) {
        let (client_w, client_h) = win_surface::client_size(self.hwnd, width, height);
        if client_w == self.width && client_h == self.height {
            return;
        }
        self.release_rtv();
        if let Err(err) = unsafe {
            self.swap_chain.ResizeBuffers(
                0,
                client_w as u32,
                client_h as u32,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_SWAP_CHAIN_FLAG(0),
            )
        } {
            crate::core::log::warn_fn(format!("D3d11Context: ResizeBuffers failed: {err}"));
            return;
        }
        self.width = client_w;
        self.height = client_h;
        if let Err(err) = self.create_rtv() {
            crate::core::log::warn_fn(format!(
                "D3d11Context: recreate RTV after resize failed: {}",
                err.short_what()
            ));
        }
    }

    fn make_current(&mut self) {
        if let Err(err) = self.ensure_rtv() {
            crate::core::log::warn_fn(format!(
                "D3d11Context: make_current ensure_rtv failed: {}",
                err.short_what()
            ));
            return;
        }
        if let Some(rtv) = self.rtv.as_ref() {
            unsafe {
                self.context
                    .OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
            }
            self.bind_viewport();
        }
    }

    fn swap_buffers(&mut self, _damage: PresentDamage) {
        let result = unsafe { self.swap_chain.Present(1, DXGI_PRESENT(0)) };
        if result.is_err() {
            crate::core::log::error_fn(format!("D3d11Context: Present failed: {result:?}"));
        }
    }

    fn shutdown(&mut self) {
        self.release_rtv();
    }

    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Vec<u32> {
        let x0 = x.clamp(0, self.width);
        let y0 = y.clamp(0, self.height);
        let x1 = x.saturating_add(width).clamp(x0, self.width);
        let y1 = y.saturating_add(height).clamp(y0, self.height);
        let read_w = x1 - x0;
        let read_h = y1 - y0;
        if read_w <= 0 || read_h <= 0 {
            return Vec::new();
        }

        let result = (|| -> Result<Vec<u32>> {
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
        })();

        match result {
            Ok(pixels) => pixels,
            Err(err) => {
                crate::core::log::warn_fn(format!(
                    "D3d11Context: read_pixels failed: {}",
                    err.short_what()
                ));
                Vec::new()
            }
        }
    }

    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }

    fn clear_render_target(&mut self, r: f32, g: f32, b: f32, a: f32) -> Result<()> {
        self.ensure_rtv()?;
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
            self.resize(width, height);
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

    fn supports_native_geometry(&self) -> bool {
        true
    }

    fn draw_solid_rects(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuSolidRect],
    ) -> Result<()> {
        self.ensure_rtv()?;
        self.make_current();
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
        self.make_current();
        self.pipeline
            .draw_stroke_rects(&self.context, viewport_w, viewport_h, scissor, rects)
    }

    fn supports_native_glyphs(&self) -> bool {
        true
    }

    fn draw_glyphs(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        glyphs: &[GpuGlyphBlit],
    ) -> Result<()> {
        self.ensure_rtv()?;
        self.make_current();
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
        self.make_current();
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
        self.make_current();
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
        self.make_current();
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
        self.make_current();
        self.pipeline
            .draw_box_shadows(&self.context, viewport_w, viewport_h, scissor, shadows)
    }

    fn blit_soft_fallback(&mut self, pixels: &[u32], width: i32, height: i32) -> Result<()> {
        self.ensure_rtv()?;
        self.make_current();
        self.pipeline
            .blit_soft_fallback(&self.device, &self.context, pixels, width, height)
    }

    fn clear_rects(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        rects: &[GpuSolidRect],
    ) -> Result<()> {
        self.ensure_rtv()?;
        self.make_current();
        self.pipeline
            .clear_rects(&self.context, viewport_w, viewport_h, rects)
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
        let hr = unsafe { self.swap_chain.Present(1, DXGI_PRESENT(0)) };
        if hr.is_err() {
            return Err(Error::new(
                Errc::PlatformError,
                format!("D3d11Context: Present failed: {hr:?}"),
            ));
        }
        Ok(())
    }
}

unsafe impl Send for D3d11Context {}
unsafe impl Sync for D3d11Context {}

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

        assert!(ctx.supports_native_geometry());
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
        let pixels = ctx.read_pixels(0, 0, ctx.width(), ctx.height());
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
        let pixels = warp_ctx.read_pixels(0, 0, warp_ctx.width(), warp_ctx.height());
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
}
