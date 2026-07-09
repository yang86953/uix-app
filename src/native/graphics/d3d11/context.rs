//! Direct3D 11 graphics context for Windows.

#![cfg(windows)]
#![allow(nonstandard_style)]

use std::ffi::c_void;

use crate::core::{Errc, Error, Result};
use crate::native::graphics::platform::windows as win_surface;
use crate::native::traits::present::{
    GraphicsBackend, GraphicsContextCaps, IGraphicsContext, PresentDamage,
};
use ::windows::Win32::Foundation::{HMODULE, HWND, TRUE};
use ::windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_10_0,
    D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
};
use ::windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDeviceAndSwapChain, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
};
use ::windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_MODE_DESC, DXGI_MODE_SCALING_UNSPECIFIED,
    DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED, DXGI_RATIONAL, DXGI_SAMPLE_DESC,
};
use ::windows::Win32::Graphics::Dxgi::{
    IDXGISwapChain, DXGI_PRESENT, DXGI_SWAP_CHAIN_DESC, DXGI_SWAP_CHAIN_FLAG,
    DXGI_SWAP_EFFECT_DISCARD, DXGI_USAGE_RENDER_TARGET_OUTPUT,
};

type HWND_PTR = *mut c_void;

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

pub struct D3d11Context {
    hwnd: HWND_PTR,
    _device: ID3D11Device,
    context: ID3D11DeviceContext,
    swap_chain: IDXGISwapChain,
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
        let feature_levels = [
            D3D_FEATURE_LEVEL_11_1,
            D3D_FEATURE_LEVEL_11_0,
            D3D_FEATURE_LEVEL_10_1,
            D3D_FEATURE_LEVEL_10_0,
        ];

        create_with_driver(
            native_window,
            client_w,
            client_h,
            &feature_levels,
            D3D_DRIVER_TYPE_HARDWARE,
        )
        .or_else(|_| {
            create_with_driver(
                native_window,
                client_w,
                client_h,
                &feature_levels,
                D3D_DRIVER_TYPE_WARP,
            )
        })
    }
}

fn create_with_driver(
    hwnd: HWND_PTR,
    width: i32,
    height: i32,
    feature_levels: &[D3D_FEATURE_LEVEL],
    driver_type: ::windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE,
) -> Result<D3d11Context> {
    let mut swap_chain = None;
    let mut device = None;
    let mut context = None;
    let mut selected_level = D3D_FEATURE_LEVEL_10_0;
    let desc = swap_chain_desc(hwnd, width, height);

    unsafe {
        D3D11CreateDeviceAndSwapChain(
            None,
            driver_type,
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

    crate::core::log::info_fn(format!(
        "D3d11Context: created {width}x{height} swapchain at feature level {:?}",
        selected_level
    ));

    Ok(D3d11Context {
        hwnd,
        _device: device,
        context,
        swap_chain,
        width,
        height,
    })
}

impl IGraphicsContext for D3d11Context {
    fn caps(&self) -> crate::native::traits::present::GraphicsContextCaps {
        GraphicsContextCaps::cpu_upload_present(GraphicsBackend::D3d11, 1.0)
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
    }

    fn make_current(&mut self) {}

    fn swap_buffers(&mut self, _damage: PresentDamage) {
        let _ = unsafe { self.swap_chain.Present(1, DXGI_PRESENT(0)) };
    }

    fn shutdown(&mut self) {}

    fn read_pixels(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) -> Vec<u32> {
        Vec::new()
    }

    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }

    fn present_pixels(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        _damage: PresentDamage,
    ) -> Result<()> {
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
    fn factory_create_d3d11_context_on_real_window() {
        let mut platform = crate::native::create_platform().expect("platform");
        let window = platform
            .window_manager()
            .create_window("D3D11 GPU test", 320, 240)
            .expect("window");
        let surface = window.native_surface_ptr();
        assert!(
            !surface.is_null(),
            "Windows HWND must be exposed as native_surface_ptr"
        );

        let mut ctx = D3d11Context::new(surface, 320, 240).expect("D3d11Context");
        assert_eq!(ctx.graphics_backend(), GraphicsBackend::D3d11);
        assert!(ctx.supports_pixel_present());
        let pixels = vec![0xFF00_0000; (ctx.width() * ctx.height()) as usize];
        ctx.present_pixels(&pixels, ctx.width(), ctx.height(), PresentDamage::Full)
            .expect("present");
        ctx.shutdown();
    }
}
