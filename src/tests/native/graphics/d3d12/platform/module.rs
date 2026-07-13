use crate::tests::common::*;
use std::ffi::c_void;
use crate::core::Result;

#[cfg(not(feature = "d3d12"))]
#[test]
fn disabled_feature_cannot_create_a_real_context() {
    let error = match crate::native::graphics::d3d12::platform::create(std::ptr::null_mut(), 64, 64)
    {
        Ok(_) => panic!("disabled D3D12 feature must not create a context"),
        Err(error) => error,
    };
    assert!(error.what().contains("requires the d3d12 feature"));
}

#[cfg(all(windows, feature = "d3d12"))]
#[test]
fn enabled_feature_rejects_null_surface() {
    let error = match crate::native::graphics::d3d12::platform::create(std::ptr::null_mut(), 64, 64)
    {
        Ok(_) => panic!("null surface must not create a D3D12 context"),
        Err(error) => error,
    };
    assert!(!error.what().is_empty());
}
