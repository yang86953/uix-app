use crate::tests::common::*;
use std::ffi::c_void;
use std::mem::ManuallyDrop;
use crate::core::{ Result };
use crate::native::traits::present::{ GpuSolidRect };
use ::windows::Win32::Foundation::{FALSE, RECT, TRUE};
use ::windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use ::windows::Win32::Graphics::Direct3D::{D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, ID3DBlob};
use ::windows::Win32::Graphics::Direct3D12::*;
use ::windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use ::windows::core::PCSTR;
use crate::native::graphics::d3d12::platform::pipeline::validated_soft_layout;

#[test]
fn soft_upload_layout_aligns_rows_and_rejects_short_input() {
    assert_eq!(validated_soft_layout(65, 37, 65 * 37), Ok((512, 512 * 37)));
    let error = validated_soft_layout(65, 37, 65 * 37 - 1).expect_err("short input");
    assert_eq!(error.code(), crate::core::Errc::InvalidArgument);
    assert!(validated_soft_layout(i32::MAX, 1, usize::MAX).is_err());
}
