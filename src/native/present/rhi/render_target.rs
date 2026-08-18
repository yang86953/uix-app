//! 图形 API 无关的封闭渲染目标身份。
//!
//! Surface 与 device texture 是互斥变体，不共享裸整数值域；只有该值对象
//! 可以把 TextureHandle 提升为 pass target，Adapter 不再维护 Surface 哨兵。

// 引入统一纹理描述与共享纹理资源身份。
use super::{TextureDesc, TextureHandle};
// 引入共享参数错误类型。
use crate::core::error::{Errc, Error, Result};

// 描述一个 render pass 可以写入的封闭目标种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum RenderTargetHandle {
    // 指向当前 GraphicsSurface acquire 的原生 drawable。
    Surface,
    // 指向当前 GraphicsDevice 资源表中的离屏纹理。
    Texture(TextureHandle),
}

// 为渲染目标提供不暴露裸值的类型化构造与投影。
impl RenderTargetHandle {
    // 创建当前 acquired Surface 的目标身份。
    pub(crate) const fn surface() -> Self {
        // Surface 是独立变体，不占用任何 texture 槽位。
        Self::Surface
    }

    // 把存活 texture 身份提升为离屏 render target。
    pub(super) fn for_texture(texture: TextureHandle, desc: TextureDesc) -> Result<Self> {
        // 只有共享纹理描述证明可渲染时才能提升目标身份。
        if !desc.format().supports_render_target() {
            // sampled-only 纹理统一在共享边界拒绝为输出目标。
            return Err(Error::new(
                Errc::InvalidArgument,
                "RHI texture format is not renderable",
            ));
        }
        // 保存已经由真实资源描述证明的纹理身份。
        Ok(Self::Texture(texture))
    }

    // 测试 fixture 使用显式入口构造稳定目标身份。
    #[cfg(test)]
    pub(crate) const fn for_test(texture: TextureHandle) -> Self {
        // 测试不代表真实资源表的可渲染能力证明。
        Self::Texture(texture)
    }

    // 判断当前目标是否为原生 Surface。
    pub(crate) const fn is_surface(self) -> bool {
        // 只匹配封闭枚举变体。
        matches!(self, Self::Surface)
    }

    // 返回离屏 texture 身份；Surface 不伪造资源句柄。
    pub(crate) const fn texture(self) -> Option<TextureHandle> {
        // 只有 Texture 变体携带 device 资源身份。
        match self {
            // Surface 不属于 texture 资源表。
            Self::Surface => None,
            // Texture 原样返回创建时绑定的句柄。
            Self::Texture(texture) => Some(texture),
        }
    }
}

// 仅验证 Surface 与 texture 目标的封闭身份语义。
#[cfg(test)]
mod tests {
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
}
