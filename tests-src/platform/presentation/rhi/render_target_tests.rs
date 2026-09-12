//! RenderTarget 测试构造辅助（自 src/platform/presentation/rhi/render_target.rs 移入，零调用探针原样保留待接线）。
//! 经 #[path] 引用，不进发布包。
#![allow(dead_code)]

use super::*;

impl RenderTargetHandle {
// 测试 fixture 使用显式入口构造稳定目标身份。
    #[cfg(test)]
    pub(crate) const fn for_test(texture: TextureHandle) -> Self {
        // 测试不代表真实资源表的可渲染能力证明。
        Self::Texture(texture)
    }
}
