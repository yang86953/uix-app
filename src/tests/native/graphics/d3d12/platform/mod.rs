#[test]
fn disabled_feature_cannot_create_a_real_context() {
    let error = match super::create(std::ptr::null_mut(), 64, 64) {
        Ok(_) => panic!("disabled D3D12 feature must not create a context"),
        Err(error) => error,
    };
    assert!(error.what().contains("requires the d3d12 feature"));
}
