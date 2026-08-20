//! D3D12 adapter 探测与 device 创建。

use crate::core::Result;
use ::windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use ::windows::Win32::Graphics::Direct3D12::{D3D12CreateDevice, ID3D12Device};
use ::windows::Win32::Graphics::Dxgi::{DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIAdapter1, IDXGIFactory4};

use super::error::{d3d12_error, platform_error};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum D3d12DriverKind {
    Hardware,
    Warp,
}

impl D3d12DriverKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Hardware => "hardware",
            Self::Warp => "warp",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct D3d12AdapterInfo {
    pub(crate) driver: D3d12DriverKind,
    pub(crate) description: String,
    pub(crate) vendor_id: u32,
    pub(crate) device_id: u32,
    pub(crate) dedicated_video_memory: u64,
}

impl D3d12AdapterInfo {
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

fn adapter_info(adapter: &IDXGIAdapter1, driver: D3d12DriverKind) -> Result<D3d12AdapterInfo> {
    // SAFETY: adapter 是存活且带引用计数的 DXGI COM 接口，GetDesc1 同步返回完整描述值。
    let desc = unsafe { adapter.GetDesc1() }
        .map_err(|error| d3d12_error("IDXGIAdapter1::GetDesc1", error))?;
    let description_len = desc
        .Description
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(desc.Description.len());
    Ok(D3d12AdapterInfo {
        driver,
        description: String::from_utf16_lossy(&desc.Description[..description_len]),
        vendor_id: desc.VendorId,
        device_id: desc.DeviceId,
        dedicated_video_memory: desc.DedicatedVideoMemory as u64,
    })
}

fn create_device_for_adapter(adapter: &IDXGIAdapter1) -> Result<ID3D12Device> {
    let mut device = None;
    // SAFETY: adapter 在同步调用期间存活，device 输出槽有效且由 windows crate 负责 COM 引用计数。
    unsafe { D3D12CreateDevice(adapter, D3D_FEATURE_LEVEL_11_0, &mut device) }
        .map_err(|error| d3d12_error("D3D12CreateDevice", error))?;
    device.ok_or_else(|| platform_error("D3d12Context: D3D12CreateDevice returned no device"))
}

pub(crate) fn select_hardware_adapter(
    factory: &IDXGIFactory4,
) -> Result<(IDXGIAdapter1, ID3D12Device, D3d12AdapterInfo)> {
    let mut index = 0u32;
    let mut failures = Vec::new();
    loop {
        // SAFETY: factory 是存活 DXGI COM 接口，index 按顺序枚举，成功结果自带引用计数。
        let adapter = match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => adapter,
            Err(_) => break,
        };
        index += 1;
        // SAFETY: adapter 是本轮成功枚举的存活 COM 接口，GetDesc1 同步返回完整描述值。
        let desc = match unsafe { adapter.GetDesc1() } {
            Ok(desc) => desc,
            Err(error) => {
                failures.push(format!("adapter#{index} desc: {error}"));
                continue;
            }
        };
        if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
            continue;
        }
        match create_device_for_adapter(&adapter) {
            Ok(device) => {
                let info = adapter_info(&adapter, D3d12DriverKind::Hardware)?;
                return Ok((adapter, device, info));
            }
            Err(error) => failures.push(format!("adapter#{index}: {}", error.short_what())),
        }
    }
    Err(platform_error(format!(
        "D3d12Context: no hardware adapter accepted feature level 11_0; failures=[{}]",
        failures.join("; ")
    )))
}

pub(super) fn select_warp_adapter(
    factory: &IDXGIFactory4,
) -> Result<(IDXGIAdapter1, ID3D12Device, D3d12AdapterInfo)> {
    // SAFETY: factory 是存活 DXGI COM 接口，EnumWarpAdapter 返回带引用计数的 IDXGIAdapter1。
    let adapter: IDXGIAdapter1 = unsafe { factory.EnumWarpAdapter() }
        .map_err(|error| d3d12_error("IDXGIFactory4::EnumWarpAdapter", error))?;
    let device = create_device_for_adapter(&adapter)?;
    let info = adapter_info(&adapter, D3d12DriverKind::Warp)?;
    Ok((adapter, device, info))
}
