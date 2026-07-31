//! D3D12 readback/upload 与 resource transition 辅助函数。

use std::mem::ManuallyDrop;

use crate::core::Result;
use ::windows::Win32::Graphics::Direct3D12::*;
use ::windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC;

use super::error::{d3d12_error, platform_error};

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

pub(super) fn create_readback_buffer(device: &ID3D12Device, size: u64) -> Result<ID3D12Resource> {
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

pub(super) fn create_upload_buffer(device: &ID3D12Device, size: u64) -> Result<ID3D12Resource> {
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

pub(super) fn record_transition(
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

pub(super) fn texture_copy_location_subresource(
    resource: &ID3D12Resource,
) -> D3D12_TEXTURE_COPY_LOCATION {
    D3D12_TEXTURE_COPY_LOCATION {
        pResource: ManuallyDrop::new(Some(resource.clone())),
        Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
        Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
            SubresourceIndex: 0,
        },
    }
}

pub(super) fn texture_copy_location_footprint(
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

pub(super) fn release_copy_location(location: &mut D3D12_TEXTURE_COPY_LOCATION) {
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
