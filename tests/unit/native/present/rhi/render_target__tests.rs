// 引入被测目标与纹理句柄。
use super::{RenderTargetHandle, TextureHandle};

// 验证 Surface 不占用或伪造 texture 句柄值域。
#[test]
fn surface_target_has_no_texture_identity() {
    // 创建共享 Surface 目标。
    let target = RenderTargetHandle::surface();
    // Surface 变体必须可以被显式识别。
    assert!(target.is_surface());
    // Surface 不能被投影为任意 texture 资源。
    assert_eq!(target.texture(), None);
}

// 验证 texture target 无损保留类型化资源身份。
#[test]
fn texture_target_preserves_resource_identity() {
    // 创建稳定测试 texture 身份。
    let texture = TextureHandle::from_raw(7);
    // 把 texture 提升为 render target。
    let target = RenderTargetHandle::for_test(texture);
    // Texture 目标不得被误判为 Surface。
    assert!(!target.is_surface());
    // 资源投影必须返回同一类型化 texture 身份。
    assert_eq!(target.texture(), Some(texture));
}
