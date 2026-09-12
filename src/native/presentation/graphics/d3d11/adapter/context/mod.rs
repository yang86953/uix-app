//! Direct3D 11 graphics context for Windows.
//!
//! Caps: [`RasterMode::GpuNative`] × [`PresentMode::Swapchain`] (#169).
//! Native solid/rounded fill + stroke + glyph atlas text + linear/radial
//! gradients + simple path meshes + box/ambient shadow; unsupported Canvas2D
//! ops soft-raster and alpha-blit (same hybrid pattern as the native GL path).
//! 主路径使用 `DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL` 与 `Present1`，构造期
//! 能力不足时回退 `DXGI_SWAP_EFFECT_DISCARD`；状态边界把
//! `DXGI_STATUS_OCCLUDED` 映射为 `Errc::GraphicsOccluded`，并以
//! `Present(0, DXGI_PRESENT_TEST)` 做无帧数据的退出探测。

#![allow(nonstandard_style)]

use std::ffi::c_void;

use super::pipeline::{D3d11IndexBinding, D3d11Pipeline};
use super::swapchain::d3d_error;
// 向 context 子模块公开 D3D11 swapchain 的冻结实例与创建入口。
pub(crate) use super::swapchain::{D3d11SwapChain, create_swap_chain};
pub(crate) use super::swapchain::{map_dxgi_device_removed_reason, map_dxgi_resize_result};
use crate::core::{Errc, Error, Result};
// 导入 context 实现直接消费的共享生命周期契约。
use crate::native::presentation::graphics::platform::windows as win_surface;
use crate::platform::presentation::GraphicsContextLifecycle;
// 引入唯一拥有 D3D11 Surface token、extent 与重建顺序的共享生命周期。
use crate::platform::presentation::rhi::RhiSurfaceLifecycle;
use ::windows::Win32::Foundation::HMODULE;
use ::windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL,
    D3D_FEATURE_LEVEL_10_0, D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
};
use ::windows::Win32::Graphics::Direct3D11::{
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ,
    D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
    D3D11_VIEWPORT, D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11RenderTargetView,
    ID3D11Texture2D,
};
use ::windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use ::windows::Win32::Graphics::Dxgi::IDXGIDevice;
use ::windows::core::Interface;

type HWND_PTR = *mut c_void;

pub(crate) const D3D11_FEATURE_LEVELS: [D3D_FEATURE_LEVEL; 4] = [
    D3D_FEATURE_LEVEL_11_1,
    D3D_FEATURE_LEVEL_11_0,
    D3D_FEATURE_LEVEL_10_1,
    D3D_FEATURE_LEVEL_10_0,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// D3D11 驱动种类只用于 crate 内部诊断与配方选择。
pub(crate) enum D3d11DriverKind {
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

    // 返回 crate 内部诊断使用的稳定驱动名称。
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Hardware => "hardware",
            Self::Warp => "warp",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
// D3D11 适配器事实只在 crate 内部图形诊断中传播。
pub(crate) struct D3d11AdapterInfo {
    // 保存实际创建上下文使用的驱动种类。
    pub(crate) driver: D3d11DriverKind,
    // 保存 DXGI 提供的适配器描述。
    pub(crate) description: String,
    // 保存 PCI 厂商标识。
    pub(crate) vendor_id: u32,
    // 保存 PCI 设备标识。
    pub(crate) device_id: u32,
    // 保存专用显存字节数。
    pub(crate) dedicated_video_memory: u64,
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

    // 生成 crate 内部日志使用的稳定适配器摘要。
    pub(crate) fn diagnostic_summary(&self) -> String {
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
    // SAFETY: dxgi_device 是由存活 D3D11 device 查询得到的 COM 接口，GetAdapter 同步返回带引用计数的接口。
    let adapter = unsafe { dxgi_device.GetAdapter() }
        .map_err(|err| d3d_error("IDXGIDevice::GetAdapter", err))?;
    // SAFETY: adapter 是上一步成功取得的存活 COM 接口，GetDesc 只写入返回值并同步完成。
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

// D3D11 原生上下文只由 crate 内部图形配方拥有。
pub(crate) struct D3d11Context {
    device: ID3D11Device,
    pub(crate) context: ID3D11DeviceContext,
    // 保存构造期冻结为 tracked 或 legacy 的唯一 swapchain owner。
    swap_chain: D3d11SwapChain,
    pub(crate) rtv: Option<ID3D11RenderTargetView>,
    pipeline: D3d11Pipeline,
    pub(crate) adapter_info: D3d11AdapterInfo,
    logical_width: i32,
    logical_height: i32,
    // 唯一拥有 Surface generation、物理 extent、事务号与重建状态。
    surface_lifecycle: RhiSurfaceLifecycle,
    // 记录 checked shutdown 是否已经完整提交，阻止关闭后重新取得 RHI。
    shutdown: bool,
    /// 迁移期薄 RHI 的 D3D11 资源与 pass 状态。
    // RHI 设备只在 context 及其实现子模块内流转，不扩大到整个 crate。
    pub(in crate::native::presentation::graphics::d3d11::platform::context) rhi_device:
        D3d11RhiDevice,
    /// test-harness 安排的下一次 owner-thread device-lost 预检。
    #[cfg(feature = "test-harness")]
    rhi_device_lost_for_test: bool,
    /// test-harness 安排的下一次 owner-thread surface-lost acquire。
    #[cfg(feature = "test-harness")]
    rhi_surface_lost_for_test: bool,
}

mod graphics;
mod methods;
mod rhi;
mod rhi_device;
mod rhi_health;

// 真实 GPU parity runner 由 RHI device 子模块拥有，确保可机械访问原生资源而不扩大生产公开面。
#[cfg(uix_gpu_parity_d3d11)]
pub(crate) fn run_gpu_parity_test() {
    rhi_device::run_gpu_parity_test();
}

// 导入 D3D11 薄 RHI 的 device 状态。
use self::rhi_device::D3d11RhiDevice;
