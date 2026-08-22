//! Direct3D 12 swapchain context for Windows.
//!
//! This first slice owns the explicit D3D12 resource lifecycle and synchronization
//! contract. Draw-side activation stays gated until a thin RHI provider is available.

#![allow(nonstandard_style)]

use std::ffi::c_void;

use crate::core::{Errc, Error, Result};
// 导入 context 实现直接消费的共享生命周期契约。
use crate::platform::presentation::GraphicsContextLifecycle;
// 引入唯一拥有 D3D12 Surface token、extent 与重建顺序的共享生命周期。
use crate::native::presentation::graphics::platform::windows as win_surface;
use crate::platform::presentation::rhi::{
    RhiExtent, RhiSubmissionSequence, RhiSurfaceLifecycle, RhiSurfaceRecreateReason,
    RhiSurfaceRecreateTransaction, RhiSurfaceResizeTransaction, ValidatedRhiPresent,
};
use ::windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, WAIT_OBJECT_0};
use ::windows::Win32::Graphics::Direct3D12::*;
use ::windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use ::windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory2, DXGI_CREATE_FACTORY_FLAGS, DXGI_MWA_NO_ALT_ENTER, DXGI_PRESENT,
    DXGI_SWAP_CHAIN_FLAG, IDXGIAdapter1, IDXGIFactory4, IDXGIOutput, IDXGISwapChain3,
};
use ::windows::Win32::System::Threading::{CreateEventW, INFINITE, WaitForSingleObject};
use ::windows::core::Interface;

type HWND_PTR = *mut c_void;

use super::adapter::{
    D3d12AdapterInfo, D3d12DriverKind, select_hardware_adapter, select_warp_adapter,
};
use super::error::{d3d12_error, platform_error};
use super::swap_chain::{FRAME_COUNT, swap_chain_desc};
use super::transfer::{
    copy_mapped_bgra_rows, create_readback_buffer, record_transition, release_copy_location,
    texture_copy_location_footprint, texture_copy_location_subresource,
};

pub struct D3d12Context {
    hwnd: HWND_PTR,
    _factory: IDXGIFactory4,
    _adapter: IDXGIAdapter1,
    device: ID3D12Device,
    queue: ID3D12CommandQueue,
    swap_chain: IDXGISwapChain3,
    rtv_heap: ID3D12DescriptorHeap,
    rtv_stride: u32,
    back_buffers: Vec<ID3D12Resource>,
    back_buffer_states: [D3D12_RESOURCE_STATES; FRAME_COUNT],
    allocators: Vec<ID3D12CommandAllocator>,
    command_list: ID3D12GraphicsCommandList,
    command_list_base: ID3D12CommandList,
    fence: ID3D12Fence,
    fence_event: Option<HANDLE>,
    fence_values: [u64; FRAME_COUNT],
    next_fence_value: u64,
    frame_index: usize,
    recording: bool,
    pending_gpu_resources: Vec<ID3D12Resource>,
    // 唯一拥有 D3D12 Buffer、Texture、Sampler 与 platform 类型化资源表。
    rhi_device: rhi_device::D3d12RhiDevice,
    pub(crate) adapter_info: D3d12AdapterInfo,
    logical_width: i32,
    logical_height: i32,
    // 唯一拥有 Surface generation、物理 extent、事务号与重建状态。
    surface_lifecycle: RhiSurfaceLifecycle,
    // 预留同一组合 context 的共享提交门禁；缺失 Device 时保持无已签发提交。
    rhi_submissions: RhiSubmissionSequence,
    pub(crate) fault: Option<String>,
    shutdown: bool,
}

mod graphics;
mod methods;
mod rhi_device;
mod rhi_surface;
