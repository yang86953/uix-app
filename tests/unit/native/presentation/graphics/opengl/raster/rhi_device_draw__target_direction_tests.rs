// 引入父模块私有目标方向 helper。
use super::super::target_y_sign;
// 引入封闭 render target 与 texture 句柄。
use crate::platform::presentation::rhi::{RenderTargetHandle, TextureHandle};

// 锁定原生 surface 把左上逻辑坐标映射到 GL 高 Y。
#[test]
fn surface_target_uses_window_top_direction() {
    // 构造共享封闭的原生 Surface 目标。
    let surface = RenderTargetHandle::surface();
    // surface 顶部必须通过负符号从逻辑 Y-down 映射到 NDC Y-up。
    assert_eq!(target_y_sign(surface), -1.0);
}

// 锁定 texture target 把逻辑顶部保存为可直接采样的 v=0 行。
#[test]
fn texture_target_uses_top_left_storage_direction() {
    // 构造普通非零 texture render target 身份。
    let texture = RenderTargetHandle::for_test(TextureHandle::from_raw(1));
    // texture 目标必须保留正符号，让逻辑顶部写入 GL 第零行。
    assert_eq!(target_y_sign(texture), 1.0);
}
