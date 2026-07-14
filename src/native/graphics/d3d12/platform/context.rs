//! Direct3D 12 swapchain context for Windows.
//!
//! This first slice owns the explicit D3D12 resource lifecycle and synchronization
//! contract. Draw-side activation stays gated until the native solid/soft pipelines
//! are available.

#![allow(nonstandard_style)]

use std::ffi::c_void;
use std::mem::ManuallyDrop;

use crate::core::{Errc, Error, Result};
use crate::native::graphics::platform::windows as win_surface;
use crate::native::traits::present::{
    GpuSolidRect, GraphicsBackend, GraphicsContextCaps, IGraphicsContext, NativeRasterCaps,
    PresentCoherency, PresentDamage, PresentFrame, SoftFallbackTile,
};
use ::windows::core::Interface;
use ::windows::Win32::Foundation::{CloseHandle, E_OUTOFMEMORY, HANDLE, HWND, WAIT_OBJECT_0};
use ::windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use ::windows::Win32::Graphics::Direct3D12::*;
use ::windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use ::windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory2, IDXGIAdapter1, IDXGIFactory4, IDXGIOutput, IDXGISwapChain3,
    DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_CREATE_FACTORY_FLAGS, DXGI_ERROR_DEVICE_HUNG,
    DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_DEVICE_RESET, DXGI_ERROR_DRIVER_INTERNAL_ERROR,
    DXGI_ERROR_REMOTE_OUTOFMEMORY, DXGI_MWA_NO_ALT_ENTER, DXGI_PRESENT, DXGI_SCALING_STRETCH,
    DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_FLIP_DISCARD,
    DXGI_USAGE_RENDER_TARGET_OUTPUT,
};
use ::windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject, INFINITE};

type HWND_PTR = *mut c_void;

const FRAME_COUNT: usize = 2;

use super::pipeline::D3d12Pipeline;

pub(crate) fn d3d12_hresult_code(result: ::windows::core::HRESULT) -> Errc {
    match result {
        DXGI_ERROR_DEVICE_HUNG
        | DXGI_ERROR_DEVICE_REMOVED
        | DXGI_ERROR_DEVICE_RESET
        | DXGI_ERROR_DRIVER_INTERNAL_ERROR => Errc::GraphicsDeviceLost,
        E_OUTOFMEMORY | DXGI_ERROR_REMOTE_OUTOFMEMORY => Errc::GraphicsOutOfMemory,
        _ => Errc::PlatformError,
    }
}

fn d3d12_error(operation: &str, error: ::windows::core::Error) -> Error {
    Error::new(
        d3d12_hresult_code(error.code()),
        format!("D3d12Context: {operation} failed: {error}"),
    )
}

fn platform_error(message: impl Into<String>) -> Error {
    Error::new(Errc::PlatformError, message)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum D3d12DriverKind {
    Hardware,
    Warp,
}

impl D3d12DriverKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hardware => "hardware",
            Self::Warp => "warp",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D3d12AdapterInfo {
    pub driver: D3d12DriverKind,
    pub description: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub dedicated_video_memory: u64,
}

impl D3d12AdapterInfo {
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

fn adapter_info(adapter: &IDXGIAdapter1, driver: D3d12DriverKind) -> Result<D3d12AdapterInfo> {
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
        let adapter = match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => adapter,
            Err(_) => break,
        };
        index += 1;
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

fn select_warp_adapter(
    factory: &IDXGIFactory4,
) -> Result<(IDXGIAdapter1, ID3D12Device, D3d12AdapterInfo)> {
    let adapter: IDXGIAdapter1 = unsafe { factory.EnumWarpAdapter() }
        .map_err(|error| d3d12_error("IDXGIFactory4::EnumWarpAdapter", error))?;
    let device = create_device_for_adapter(&adapter)?;
    let info = adapter_info(&adapter, D3d12DriverKind::Warp)?;
    Ok((adapter, device, info))
}

pub(crate) fn swap_chain_desc(width: i32, height: i32) -> DXGI_SWAP_CHAIN_DESC1 {
    DXGI_SWAP_CHAIN_DESC1 {
        Width: width.max(1) as u32,
        Height: height.max(1) as u32,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        Stereo: false.into(),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: FRAME_COUNT as u32,
        Scaling: DXGI_SCALING_STRETCH,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
        AlphaMode: DXGI_ALPHA_MODE_IGNORE,
        Flags: 0,
    }
}

fn buffer_resource_desc(size: u64) -> D3D12_RESOURCE_DESC {
    D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Alignment: 0,
        Width: size.max(1),
        Height: 1,
        DepthOrArraySize: 1,
        MipLevels: 1,
        Format: Default::default(),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
        Flags: D3D12_RESOURCE_FLAG_NONE,
    }
}

fn create_readback_buffer(device: &ID3D12Device, size: u64) -> Result<ID3D12Resource> {
    let heap = D3D12_HEAP_PROPERTIES {
        Type: D3D12_HEAP_TYPE_READBACK,
        CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
        MemoryPoolPreference: D3D12_MEMORY_POOL_UNKNOWN,
        CreationNodeMask: 0,
        VisibleNodeMask: 0,
    };
    let desc = buffer_resource_desc(size);
    let mut resource = None;
    unsafe {
        device.CreateCommittedResource(
            &heap,
            D3D12_HEAP_FLAG_NONE,
            &desc,
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
            &mut resource,
        )
    }
    .map_err(|error| d3d12_error("ID3D12Device::CreateCommittedResource(readback)", error))?;
    resource.ok_or_else(|| platform_error("D3d12Context: readback buffer was not created"))
}

fn create_upload_buffer(device: &ID3D12Device, size: u64) -> Result<ID3D12Resource> {
    let heap = D3D12_HEAP_PROPERTIES {
        Type: D3D12_HEAP_TYPE_UPLOAD,
        CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
        MemoryPoolPreference: D3D12_MEMORY_POOL_UNKNOWN,
        CreationNodeMask: 0,
        VisibleNodeMask: 0,
    };
    let desc = buffer_resource_desc(size);
    let mut resource = None;
    unsafe {
        device.CreateCommittedResource(
            &heap,
            D3D12_HEAP_FLAG_NONE,
            &desc,
            D3D12_RESOURCE_STATE_GENERIC_READ,
            None,
            &mut resource,
        )
    }
    .map_err(|error| d3d12_error("ID3D12Device::CreateCommittedResource(upload)", error))?;
    resource.ok_or_else(|| platform_error("D3d12Context: upload buffer was not created"))
}

fn record_transition(
    list: &ID3D12GraphicsCommandList,
    resource: &ID3D12Resource,
    before: D3D12_RESOURCE_STATES,
    after: D3D12_RESOURCE_STATES,
) {
    if before == after {
        return;
    }
    let transition = D3D12_RESOURCE_TRANSITION_BARRIER {
        pResource: ManuallyDrop::new(Some(resource.clone())),
        Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
        StateBefore: before,
        StateAfter: after,
    };
    let mut barrier = D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            Transition: ManuallyDrop::new(transition),
        },
    };
    unsafe {
        list.ResourceBarrier(std::slice::from_ref(&barrier));
        let transition = &mut *barrier.Anonymous.Transition;
        ManuallyDrop::drop(&mut transition.pResource);
    }
}

fn texture_copy_location_subresource(resource: &ID3D12Resource) -> D3D12_TEXTURE_COPY_LOCATION {
    D3D12_TEXTURE_COPY_LOCATION {
        pResource: ManuallyDrop::new(Some(resource.clone())),
        Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
        Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
            SubresourceIndex: 0,
        },
    }
}

fn texture_copy_location_footprint(
    resource: &ID3D12Resource,
    footprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT,
) -> D3D12_TEXTURE_COPY_LOCATION {
    D3D12_TEXTURE_COPY_LOCATION {
        pResource: ManuallyDrop::new(Some(resource.clone())),
        Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
        Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
            PlacedFootprint: footprint,
        },
    }
}

fn release_copy_location(location: &mut D3D12_TEXTURE_COPY_LOCATION) {
    unsafe {
        ManuallyDrop::drop(&mut location.pResource);
    }
}

pub(crate) fn copy_mapped_bgra_rows(
    mapped: *const u8,
    footprint_offset: usize,
    row_pitch: usize,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) -> Vec<u32> {
    let mut pixels = vec![0u32; width.saturating_mul(height)];
    for row in 0..height {
        // SAFETY: caller provides a mapped D3D12 readback buffer whose footprint
        // covers `(y + row) * row_pitch + (x + width) * 4` bytes.
        let source = unsafe {
            mapped
                .add(footprint_offset + (y + row) * row_pitch + x * 4)
                .cast::<u32>()
        };
        let destination = pixels[row * width..].as_mut_ptr();
        // SAFETY: source and destination are valid for `width` u32 values and do
        // not overlap; destination points into the row allocated above.
        unsafe {
            std::ptr::copy_nonoverlapping(source, destination, width);
        }
    }
    pixels
}

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
    pipeline: Option<D3d12Pipeline>,
    fence: ID3D12Fence,
    fence_event: Option<HANDLE>,
    fence_values: [u64; FRAME_COUNT],
    next_fence_value: u64,
    frame_index: usize,
    recording: bool,
    pending_gpu_resources: Vec<ID3D12Resource>,
    pub(crate) adapter_info: D3d12AdapterInfo,
    logical_width: i32,
    logical_height: i32,
    width: i32,
    height: i32,
    pub(crate) fault: Option<String>,
    shutdown: bool,
}

impl D3d12Context {
    pub(crate) fn new(native_window: *mut c_void, width: i32, height: i32) -> Result<Self> {
        if native_window.is_null() {
            return Err(platform_error("D3d12Context: native window handle is null"));
        }
        let factory: IDXGIFactory4 = unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)) }
            .map_err(|error| d3d12_error("CreateDXGIFactory2", error))?;
        match Self::create_with_factory(
            native_window,
            width,
            height,
            factory.clone(),
            D3d12DriverKind::Hardware,
        ) {
            Ok(context) => Ok(context),
            Err(hardware_error) => {
                crate::core::log::warn_fn(format!(
                    "D3d12Context: hardware device unavailable; retrying with WARP: {}",
                    hardware_error.what()
                ));
                Self::create_with_factory(
                    native_window,
                    width,
                    height,
                    factory,
                    D3d12DriverKind::Warp,
                )
                .map_err(|warp_error| {
                    platform_error(format!(
                        "D3d12Context: hardware and WARP creation failed; hardware=[{}]; warp=[{}]",
                        hardware_error.what(),
                        warp_error.what()
                    ))
                })
            }
        }
    }

    pub(crate) fn new_with_driver(
        native_window: *mut c_void,
        width: i32,
        height: i32,
        driver: D3d12DriverKind,
    ) -> Result<Self> {
        if native_window.is_null() {
            return Err(platform_error("D3d12Context: native window handle is null"));
        }
        let factory: IDXGIFactory4 = unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)) }
            .map_err(|error| d3d12_error("CreateDXGIFactory2", error))?;
        Self::create_with_factory(native_window, width, height, factory, driver)
    }

    fn create_with_factory(
        native_window: *mut c_void,
        width: i32,
        height: i32,
        factory: IDXGIFactory4,
        driver: D3d12DriverKind,
    ) -> Result<Self> {
        let drawable = win_surface::drawable_size(native_window, width, height);
        let (adapter, device, adapter_info) = match driver {
            D3d12DriverKind::Hardware => select_hardware_adapter(&factory)?,
            D3d12DriverKind::Warp => select_warp_adapter(&factory)?,
        };
        let queue_desc = D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
            Priority: D3D12_COMMAND_QUEUE_PRIORITY_NORMAL.0,
            Flags: D3D12_COMMAND_QUEUE_FLAG_NONE,
            NodeMask: 0,
        };
        let queue: ID3D12CommandQueue = unsafe { device.CreateCommandQueue(&queue_desc) }
            .map_err(|error| d3d12_error("ID3D12Device::CreateCommandQueue", error))?;
        let desc = swap_chain_desc(drawable.width, drawable.height);
        let swap_chain1 = unsafe {
            factory.CreateSwapChainForHwnd(
                &queue,
                HWND(native_window),
                &desc,
                None,
                None::<&IDXGIOutput>,
            )
        }
        .map_err(|error| d3d12_error("IDXGIFactory4::CreateSwapChainForHwnd", error))?;
        unsafe { factory.MakeWindowAssociation(HWND(native_window), DXGI_MWA_NO_ALT_ENTER) }
            .map_err(|error| d3d12_error("IDXGIFactory4::MakeWindowAssociation", error))?;
        let swap_chain: IDXGISwapChain3 = swap_chain1
            .cast()
            .map_err(|error| d3d12_error("IDXGISwapChain1::cast<IDXGISwapChain3>", error))?;

        let rtv_heap_desc = D3D12_DESCRIPTOR_HEAP_DESC {
            Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
            NumDescriptors: FRAME_COUNT as u32,
            Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
            NodeMask: 0,
        };
        let rtv_heap: ID3D12DescriptorHeap = unsafe { device.CreateDescriptorHeap(&rtv_heap_desc) }
            .map_err(|error| d3d12_error("ID3D12Device::CreateDescriptorHeap(RTV)", error))?;
        let rtv_stride =
            unsafe { device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) };

        let mut allocators = Vec::with_capacity(FRAME_COUNT);
        for _ in 0..FRAME_COUNT {
            let allocator: ID3D12CommandAllocator =
                unsafe { device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT) }
                    .map_err(|error| d3d12_error("ID3D12Device::CreateCommandAllocator", error))?;
            allocators.push(allocator);
        }
        let command_list: ID3D12GraphicsCommandList = unsafe {
            device.CreateCommandList(
                0,
                D3D12_COMMAND_LIST_TYPE_DIRECT,
                &allocators[0],
                None::<&ID3D12PipelineState>,
            )
        }
        .map_err(|error| d3d12_error("ID3D12Device::CreateCommandList", error))?;
        unsafe { command_list.Close() }
            .map_err(|error| d3d12_error("ID3D12GraphicsCommandList::Close(initial)", error))?;
        let command_list_base: ID3D12CommandList = command_list
            .cast()
            .map_err(|error| d3d12_error("command list cast", error))?;
        let pipeline = D3d12Pipeline::new(&device, FRAME_COUNT)?;
        let fence: ID3D12Fence = unsafe { device.CreateFence(0, D3D12_FENCE_FLAG_NONE) }
            .map_err(|error| d3d12_error("ID3D12Device::CreateFence", error))?;
        let fence_event = unsafe { CreateEventW(None, false, false, None) }
            .map_err(|error| d3d12_error("CreateEventW(fence)", error))?;

        let mut context = Self {
            hwnd: native_window,
            _factory: factory,
            _adapter: adapter,
            device,
            queue,
            swap_chain,
            rtv_heap,
            rtv_stride,
            back_buffers: Vec::with_capacity(FRAME_COUNT),
            back_buffer_states: [D3D12_RESOURCE_STATE_PRESENT; FRAME_COUNT],
            allocators,
            command_list,
            command_list_base,
            pipeline: Some(pipeline),
            fence,
            fence_event: Some(fence_event),
            fence_values: [0; FRAME_COUNT],
            next_fence_value: 1,
            frame_index: 0,
            recording: false,
            pending_gpu_resources: Vec::new(),
            adapter_info,
            logical_width: drawable.logical_width,
            logical_height: drawable.logical_height,
            width: drawable.width,
            height: drawable.height,
            fault: None,
            shutdown: false,
        };
        context.rebuild_back_buffers()?;
        crate::core::log::info_fn(format!(
            "D3d12Context: created {}x{} flip-discard swapchain; {}",
            drawable.width,
            drawable.height,
            context.adapter_info.diagnostic_summary()
        ));
        Ok(context)
    }

    fn rtv_handle(&self, index: usize) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        let mut handle = unsafe { self.rtv_heap.GetCPUDescriptorHandleForHeapStart() };
        handle.ptr += index * self.rtv_stride as usize;
        handle
    }

    fn rebuild_back_buffers(&mut self) -> Result<()> {
        let mut back_buffers = Vec::with_capacity(FRAME_COUNT);
        for index in 0..FRAME_COUNT {
            let buffer: ID3D12Resource = unsafe { self.swap_chain.GetBuffer(index as u32) }
                .map_err(|error| d3d12_error("IDXGISwapChain::GetBuffer", error))?;
            unsafe {
                self.device
                    .CreateRenderTargetView(&buffer, None, self.rtv_handle(index));
            }
            back_buffers.push(buffer);
        }
        self.back_buffers = back_buffers;
        self.back_buffer_states = [D3D12_RESOURCE_STATE_PRESENT; FRAME_COUNT];
        self.frame_index = unsafe { self.swap_chain.GetCurrentBackBufferIndex() } as usize;
        Ok(())
    }

    fn ensure_healthy(&self) -> Result<()> {
        if let Some(fault) = self.fault.as_ref() {
            return Err(Error::new(
                Errc::InvalidState,
                format!("D3d12Context is faulted: {fault}"),
            ));
        }
        if self.shutdown {
            return Err(Error::new(Errc::InvalidState, "D3d12Context is shut down"));
        }
        Ok(())
    }

    fn latch_fault(&mut self, operation: &str, error: &Error) {
        if self.fault.is_none() {
            self.fault = Some(format!("{operation}: {}", error.what()));
        }
    }

    fn wait_for_fence(&self, value: u64) -> Result<()> {
        if value == 0 || unsafe { self.fence.GetCompletedValue() } >= value {
            return Ok(());
        }
        let event = self
            .fence_event
            .ok_or_else(|| platform_error("D3d12Context: fence event is closed"))?;
        unsafe { self.fence.SetEventOnCompletion(value, event) }
            .map_err(|error| d3d12_error("ID3D12Fence::SetEventOnCompletion", error))?;
        let wait = unsafe { WaitForSingleObject(event, INFINITE) };
        if wait != WAIT_OBJECT_0 {
            return Err(platform_error(format!(
                "D3d12Context: fence wait returned {wait:?}"
            )));
        }
        Ok(())
    }

    fn signal(&mut self) -> Result<u64> {
        let value = self.next_fence_value;
        self.next_fence_value = self.next_fence_value.saturating_add(1);
        unsafe { self.queue.Signal(&self.fence, value) }
            .map_err(|error| d3d12_error("ID3D12CommandQueue::Signal", error))?;
        Ok(value)
    }

    fn wait_for_gpu(&mut self) -> Result<()> {
        let value = self.signal()?;
        self.wait_for_fence(value)
    }

    fn begin_commands(&mut self) -> Result<()> {
        self.ensure_healthy()?;
        if self.recording {
            return Ok(());
        }
        if self.frame_index >= self.back_buffers.len() || self.frame_index >= self.allocators.len()
        {
            let error = platform_error(format!(
                "D3d12Context: invalid frame resources index={} buffers={} allocators={}",
                self.frame_index,
                self.back_buffers.len(),
                self.allocators.len()
            ));
            self.latch_fault("begin_commands", &error);
            return Err(error);
        }
        if let Err(error) = self.wait_for_fence(self.fence_values[self.frame_index]) {
            self.latch_fault("wait_for_frame", &error);
            return Err(error);
        }
        if let Some(pipeline) = self.pipeline.as_mut() {
            pipeline.begin_frame(self.frame_index);
        }
        let allocator = &self.allocators[self.frame_index];
        if let Err(error) = unsafe { allocator.Reset() } {
            let error = d3d12_error("ID3D12CommandAllocator::Reset", error);
            self.latch_fault("reset allocator", &error);
            return Err(error);
        }
        let reset_result = unsafe {
            self.command_list
                .Reset(allocator, None::<&ID3D12PipelineState>)
        };
        if let Err(error) = reset_result {
            let error = d3d12_error("ID3D12GraphicsCommandList::Reset", error);
            self.latch_fault("reset command list", &error);
            return Err(error);
        }
        let state = self.back_buffer_states[self.frame_index];
        if state != D3D12_RESOURCE_STATE_RENDER_TARGET {
            record_transition(
                &self.command_list,
                &self.back_buffers[self.frame_index],
                state,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
            );
            self.back_buffer_states[self.frame_index] = D3D12_RESOURCE_STATE_RENDER_TARGET;
        }
        let rtv = self.rtv_handle(self.frame_index);
        unsafe {
            self.command_list
                .OMSetRenderTargets(1, Some(&rtv), true, None);
        }
        self.recording = true;
        Ok(())
    }

    fn execute_recording(&mut self) -> Result<()> {
        if !self.recording {
            return Ok(());
        }
        if let Err(error) = unsafe { self.command_list.Close() } {
            let error = d3d12_error("ID3D12GraphicsCommandList::Close", error);
            self.latch_fault("close command list", &error);
            return Err(error);
        }
        unsafe {
            self.queue
                .ExecuteCommandLists(&[Some(self.command_list_base.clone())]);
        }
        self.recording = false;
        Ok(())
    }

    fn execute_recording_and_wait(&mut self) -> Result<()> {
        self.execute_recording()?;
        if let Err(error) = self.wait_for_gpu() {
            self.latch_fault("execute_recording_and_wait", &error);
            return Err(error);
        }
        Ok(())
    }

    fn transition_current_buffer_to_present_and_wait(&mut self) -> Result<()> {
        self.begin_commands()?;
        let buffer_index = self.frame_index;
        let state = self.back_buffer_states[buffer_index];
        record_transition(
            &self.command_list,
            &self.back_buffers[buffer_index],
            state,
            D3D12_RESOURCE_STATE_PRESENT,
        );
        self.back_buffer_states[buffer_index] = D3D12_RESOURCE_STATE_PRESENT;
        self.execute_recording_and_wait()
    }

    fn present_result(&mut self) -> Result<()> {
        self.begin_commands()?;
        let buffer_index = self.frame_index;
        let state = self.back_buffer_states[buffer_index];
        record_transition(
            &self.command_list,
            &self.back_buffers[buffer_index],
            state,
            D3D12_RESOURCE_STATE_PRESENT,
        );
        self.back_buffer_states[buffer_index] = D3D12_RESOURCE_STATE_PRESENT;
        self.execute_recording()?;

        let present = unsafe { self.swap_chain.Present(1, DXGI_PRESENT(0)) };
        let fence_value = match self.signal() {
            Ok(value) => value,
            Err(error) => {
                self.latch_fault("present signal", &error);
                return Err(error);
            }
        };
        self.fence_values[buffer_index] = fence_value;
        self.frame_index = unsafe { self.swap_chain.GetCurrentBackBufferIndex() } as usize;
        self.latch_present_result(
            present
                .ok()
                .map_err(|error| d3d12_error("IDXGISwapChain::Present", error)),
        )
    }

    /// A failed DXGI Present leaves the submitted command list in an uncertain
    /// display state. Keep the context alive only long enough for terminal
    /// cleanup; all later recording, resize, and readback work must fail.
    pub(crate) fn latch_present_result(&mut self, result: Result<()>) -> Result<()> {
        if let Err(error) = &result {
            self.latch_fault("present", error);
        }
        result
    }

    pub(crate) fn resize_result(&mut self, width: i32, height: i32) -> Result<()> {
        self.ensure_healthy()?;
        let drawable = win_surface::drawable_size(self.hwnd, width, height);
        if drawable.width == self.width && drawable.height == self.height {
            return Ok(());
        }
        // ResizeBuffers requires every reference released. Normalize the current
        // buffer to PRESENT first so a failed resize can safely rebuild and keep
        // the same state tracking for the original swapchain buffers.
        self.transition_current_buffer_to_present_and_wait()?;
        let old_width = self.width;
        let old_height = self.height;
        self.back_buffers.clear();
        let resize_result = unsafe {
            self.swap_chain.ResizeBuffers(
                FRAME_COUNT as u32,
                drawable.width as u32,
                drawable.height as u32,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_SWAP_CHAIN_FLAG(0),
            )
        };
        if let Err(error) = resize_result {
            let resize_error = d3d12_error("IDXGISwapChain::ResizeBuffers", error);
            self.width = old_width;
            self.height = old_height;
            if let Err(rebuild_error) = self.rebuild_back_buffers() {
                let combined = platform_error(format!(
                    "{}; restoring old back buffers also failed: {}",
                    resize_error.what(),
                    rebuild_error.what()
                ));
                self.latch_fault("resize recovery", &combined);
                return Err(combined);
            }
            return Err(resize_error);
        }
        self.logical_width = drawable.logical_width;
        self.logical_height = drawable.logical_height;
        self.width = drawable.width;
        self.height = drawable.height;
        self.fence_values = [0; FRAME_COUNT];
        if let Err(error) = self.rebuild_back_buffers() {
            self.latch_fault("rebuild resized back buffers", &error);
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn read_pixels_result(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<Vec<u32>> {
        let x0 = x.clamp(0, self.width);
        let y0 = y.clamp(0, self.height);
        let x1 = x.saturating_add(width).clamp(x0, self.width);
        let y1 = y.saturating_add(height).clamp(y0, self.height);
        let read_w = x1 - x0;
        let read_h = y1 - y0;
        if read_w <= 0 || read_h <= 0 {
            return Ok(Vec::new());
        }

        self.ensure_healthy()?;
        if self.frame_index >= self.back_buffers.len() {
            return Err(platform_error(format!(
                "D3d12Context: readback frame index {} has only {} buffers",
                self.frame_index,
                self.back_buffers.len()
            )));
        }
        let buffer = self.back_buffers[self.frame_index].clone();
        let desc = unsafe { buffer.GetDesc() };
        let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        let mut total_bytes = 0u64;
        unsafe {
            self.device.GetCopyableFootprints(
                &desc,
                0,
                1,
                0,
                Some(&mut footprint),
                None,
                None,
                Some(&mut total_bytes),
            );
        }
        let readback = create_readback_buffer(&self.device, total_bytes)?;
        self.begin_commands()?;
        let previous_state = self.back_buffer_states[self.frame_index];
        record_transition(
            &self.command_list,
            &buffer,
            previous_state,
            D3D12_RESOURCE_STATE_COPY_SOURCE,
        );
        self.back_buffer_states[self.frame_index] = D3D12_RESOURCE_STATE_COPY_SOURCE;
        let mut source = texture_copy_location_subresource(&buffer);
        let mut destination = texture_copy_location_footprint(&readback, footprint);
        unsafe {
            self.command_list
                .CopyTextureRegion(&destination, 0, 0, 0, &source, None);
        }
        release_copy_location(&mut source);
        release_copy_location(&mut destination);
        record_transition(
            &self.command_list,
            &buffer,
            D3D12_RESOURCE_STATE_COPY_SOURCE,
            previous_state,
        );
        self.back_buffer_states[self.frame_index] = previous_state;
        self.pending_gpu_resources.push(readback.clone());
        self.execute_recording_and_wait()?;
        self.pending_gpu_resources.pop();

        let read_range = D3D12_RANGE {
            Begin: 0,
            End: total_bytes as usize,
        };
        let mut mapped = std::ptr::null_mut();
        unsafe { readback.Map(0, Some(&read_range), Some(&mut mapped)) }
            .map_err(|error| d3d12_error("ID3D12Resource::Map(readback)", error))?;
        if mapped.is_null() {
            unsafe { readback.Unmap(0, None) };
            return Err(platform_error("D3d12Context: readback Map returned null"));
        }
        let pixels = copy_mapped_bgra_rows(
            mapped.cast(),
            footprint.Offset as usize,
            footprint.Footprint.RowPitch as usize,
            x0 as usize,
            y0 as usize,
            read_w as usize,
            read_h as usize,
        );
        let written = D3D12_RANGE { Begin: 0, End: 0 };
        unsafe { readback.Unmap(0, Some(&written)) };
        Ok(pixels)
    }

    fn drain_for_shutdown(&mut self) -> Result<()> {
        let recording_error = if !self.recording {
            None
        } else if self.fault.is_none() {
            self.execute_recording().err()
        } else {
            match unsafe { self.command_list.Close() } {
                Ok(()) => {
                    self.recording = false;
                    None
                }
                Err(error) => Some(d3d12_error(
                    "ID3D12GraphicsCommandList::Close(discard faulted recording)",
                    error,
                )),
            }
        };
        let value = match self.signal() {
            Ok(value) => value,
            Err(signal_error) => {
                let known = self.fence_values.iter().copied().max().unwrap_or(0);
                let known_wait = self.wait_for_fence(known).err();
                let device_removed = unsafe { self.device.GetDeviceRemovedReason() }.err();
                return Err(platform_error(format!(
                    "D3d12Context: shutdown could not signal a terminal fence: {}; recording_close={}; known_fence_wait={}; device_removed_reason={}",
                    signal_error.what(),
                    recording_error
                        .as_ref()
                        .map(|error| error.what())
                        .unwrap_or_else(|| "ok".to_string()),
                    known_wait
                        .as_ref()
                        .map(|error| error.what())
                        .unwrap_or_else(|| "ok".to_string()),
                    device_removed
                        .as_ref()
                        .map(ToString::to_string)
                        .as_deref()
                        .unwrap_or("none")
                )));
            }
        };
        if let Err(wait_error) = self.wait_for_fence(value) {
            let device_removed = unsafe { self.device.GetDeviceRemovedReason() }.err();
            return Err(platform_error(format!(
                "D3d12Context: shutdown terminal fence wait failed: {}; recording_close={}; device_removed_reason={}",
                wait_error.what(),
                recording_error
                    .as_ref()
                    .map(|error| error.what())
                    .unwrap_or_else(|| "ok".to_string()),
                device_removed
                    .as_ref()
                    .map(ToString::to_string)
                    .as_deref()
                    .unwrap_or("none")
            )));
        }
        if let Some(recording_error) = recording_error {
            return Err(platform_error(format!(
                "D3d12Context: terminal fence drained but the command list could not close: {}",
                recording_error.what()
            )));
        }
        Ok(())
    }

    fn retain_gpu_objects_after_undrained_drop(&self) {
        std::mem::forget(self._factory.clone());
        std::mem::forget(self._adapter.clone());
        std::mem::forget(self.device.clone());
        std::mem::forget(self.queue.clone());
        std::mem::forget(self.swap_chain.clone());
        std::mem::forget(self.rtv_heap.clone());
        for buffer in &self.back_buffers {
            std::mem::forget(buffer.clone());
        }
        for allocator in &self.allocators {
            std::mem::forget(allocator.clone());
        }
        for resource in &self.pending_gpu_resources {
            std::mem::forget(resource.clone());
        }
        if let Some(pipeline) = self.pipeline.as_ref() {
            pipeline.retain_gpu_objects_after_undrained_drop();
        }
        std::mem::forget(self.command_list.clone());
        std::mem::forget(self.command_list_base.clone());
        std::mem::forget(self.fence.clone());
    }

    pub(crate) fn shutdown_result(&mut self) -> Result<()> {
        if self.shutdown {
            return Ok(());
        }
        if let Err(error) = self.drain_for_shutdown() {
            self.latch_fault("shutdown drain", &error);
            return Err(error);
        }
        self.pending_gpu_resources.clear();
        self.pipeline.take();
        self.back_buffers.clear();
        if let Some(event) = self.fence_event.take() {
            unsafe {
                let _ = CloseHandle(event);
            }
        }
        self.shutdown = true;
        Ok(())
    }
}

impl IGraphicsContext for D3d12Context {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(
            GraphicsBackend::D3d12,
            PresentCoherency::FullOnly,
            self.device_pixel_ratio(),
        )
    }

    fn native_raster_caps(&self) -> NativeRasterCaps {
        NativeRasterCaps {
            clear_target: true,
            soft_blit: true,
            solid_rects: true,
            ..NativeRasterCaps::default()
        }
    }

    fn initialize(&mut self, _native_window: *mut c_void, _width: i32, _height: i32) -> Result<()> {
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        self.resize_result(width, height)
    }

    fn make_current(&mut self) -> Result<()> {
        self.begin_commands()
    }

    fn swap_buffers(&mut self, _damage: PresentDamage) -> Result<()> {
        self.present_result()
    }

    fn present(&mut self, frame: &PresentFrame<'_>) -> Result<()> {
        match frame {
            PresentFrame::Swapchain { .. } => self.present_result(),
            PresentFrame::PixelBuffer { .. } => Err(Error::new(
                Errc::InvalidArgument,
                "D3d12Context: PixelBuffer present is not supported",
            )),
        }
    }

    fn try_shutdown(&mut self) -> Result<()> {
        self.shutdown_result()
    }

    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Result<Vec<u32>> {
        self.read_pixels_result(x, y, width, height)
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
        self.begin_commands()?;
        unsafe {
            self.command_list.ClearRenderTargetView(
                self.rtv_handle(self.frame_index),
                &[r, g, b, a],
                None,
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
        if rects.is_empty() {
            return Ok(());
        }
        self.begin_commands()?;
        self.pipeline
            .as_ref()
            .ok_or_else(|| platform_error("D3d12Context: raster pipeline is shut down"))?
            .draw_solid_rects(&self.command_list, viewport_w, viewport_h, scissor, rects)
    }

    fn blit_soft_fallback(&mut self, pixels: &[u32], width: i32, height: i32) -> Result<()> {
        self.ensure_healthy()?;
        if width != self.width || height != self.height {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "D3d12Context: soft blit dimensions {width}x{height} do not match drawable {}x{}",
                    self.width, self.height
                ),
            ));
        }
        let expected = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| Error::new(Errc::InvalidArgument, "soft blit pixel count overflow"))?;
        if pixels.len() < expected {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "D3d12Context: soft blit buffer too small, got {}, need {expected}",
                    pixels.len()
                ),
            ));
        }
        self.begin_commands()?;
        self.pipeline
            .as_mut()
            .ok_or_else(|| platform_error("D3d12Context: raster pipeline is shut down"))?
            .blit_soft_fallback(
                &self.device,
                &self.command_list,
                self.frame_index,
                pixels,
                width,
                height,
            )
    }

    /// Full-target replace upload of premultiplied AARRGGBB pixels. Used by
    /// destination-dependent FrameEncoder ops after CPU reference apply — must
    /// not alpha-over the previous RT contents.
    fn upload_surface_pixels(&mut self, pixels: &[u32], width: i32, height: i32) -> Result<()> {
        self.ensure_healthy()?;
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        if width != self.width || height != self.height {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "D3d12Context: upload_surface_pixels {width}x{height} does not match drawable {}x{}",
                    self.width, self.height
                ),
            ));
        }
        let expected = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| {
                Error::new(
                    Errc::InvalidArgument,
                    "D3d12Context: upload_surface_pixels pixel count overflow",
                )
            })?;
        if pixels.len() < expected {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "D3d12Context: upload_surface_pixels buffer too small, got {}, need {expected}",
                    pixels.len()
                ),
            ));
        }
        if self.frame_index >= self.back_buffers.len() {
            return Err(platform_error(format!(
                "D3d12Context: upload frame index {} has only {} buffers",
                self.frame_index,
                self.back_buffers.len()
            )));
        }
        let buffer = self.back_buffers[self.frame_index].clone();
        let desc = unsafe { buffer.GetDesc() };
        let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        let mut total_bytes = 0u64;
        unsafe {
            self.device.GetCopyableFootprints(
                &desc,
                0,
                1,
                0,
                Some(&mut footprint),
                None,
                None,
                Some(&mut total_bytes),
            );
        }
        let upload = create_upload_buffer(&self.device, total_bytes)?;
        let empty_read = D3D12_RANGE { Begin: 0, End: 0 };
        let mut mapped = std::ptr::null_mut();
        unsafe { upload.Map(0, Some(&empty_read), Some(&mut mapped)) }
            .map_err(|error| d3d12_error("ID3D12Resource::Map(upload)", error))?;
        if mapped.is_null() {
            unsafe { upload.Unmap(0, None) };
            return Err(platform_error("D3d12Context: upload Map returned null"));
        }
        let row_bytes = width as usize * std::mem::size_of::<u32>();
        let row_pitch = footprint.Footprint.RowPitch as usize;
        let offset = footprint.Offset as usize;
        for row in 0..height as usize {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    pixels.as_ptr().add(row * width as usize).cast::<u8>(),
                    mapped.cast::<u8>().add(offset + row * row_pitch),
                    row_bytes,
                );
            }
        }
        let written = D3D12_RANGE {
            Begin: 0,
            End: total_bytes as usize,
        };
        unsafe { upload.Unmap(0, Some(&written)) };

        self.begin_commands()?;
        let previous_state = self.back_buffer_states[self.frame_index];
        record_transition(
            &self.command_list,
            &buffer,
            previous_state,
            D3D12_RESOURCE_STATE_COPY_DEST,
        );
        self.back_buffer_states[self.frame_index] = D3D12_RESOURCE_STATE_COPY_DEST;
        let mut destination = texture_copy_location_subresource(&buffer);
        let mut source = texture_copy_location_footprint(&upload, footprint);
        unsafe {
            self.command_list
                .CopyTextureRegion(&destination, 0, 0, 0, &source, None);
        }
        release_copy_location(&mut destination);
        release_copy_location(&mut source);
        record_transition(
            &self.command_list,
            &buffer,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
        );
        self.back_buffer_states[self.frame_index] = D3D12_RESOURCE_STATE_RENDER_TARGET;
        self.pending_gpu_resources.push(upload);
        self.execute_recording_and_wait()?;
        self.pending_gpu_resources.pop();
        Ok(())
    }

    fn blit_soft_fallback_tile(&mut self, pixels: &[u32], tile: SoftFallbackTile) -> Result<()> {
        self.ensure_healthy()?;
        self.begin_commands()?;
        self.pipeline
            .as_mut()
            .ok_or_else(|| platform_error("D3d12Context: raster pipeline is shut down"))?
            .blit_soft_fallback_tile(
                &self.device,
                &self.command_list,
                self.frame_index,
                pixels,
                self.width,
                self.height,
                tile,
            )
    }
}

impl Drop for D3d12Context {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown_result() {
            crate::core::log::error_fn(format!(
                "D3d12Context: undrained Drop retained GPU COM objects: {}",
                error.short_what()
            ));
            self.retain_gpu_objects_after_undrained_drop();
        }
    }
}
