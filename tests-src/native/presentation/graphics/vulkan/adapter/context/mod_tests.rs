//! parity 源文本路径辅助（自 src/native/presentation/graphics/vulkan/adapter/context/mod.rs 移入，零调用探针原样保留待接线）。
//! 经 #[path] 引用，不进发布包。
#![allow(dead_code)]

use super::*;
/// 架构守卫的拆分后源边界：`surface.rs` 持有 `create_win32_surface`、
/// `create_wayland_surface`、`create_metal_surface` 与 `portability_enumeration`；
/// `adapter.rs` 持有 `portability_subset`。返回源码仅供测试核对真实所有权。
#[cfg(test)]
pub(crate) const fn platform_contract_sources() -> (&'static str, &'static str) {
    (include_str!("../../../../../../../src/native/presentation/graphics/vulkan/adapter/surface.rs"), include_str!("../../../../../../../src/native/presentation/graphics/vulkan/adapter/adapter.rs"))
}

#[cfg(test)]
pub(crate) const fn drawable_contract_source() -> &'static str {
    include_str!("../../../../../../../src/native/presentation/graphics/vulkan/adapter/drawable.rs")
}
