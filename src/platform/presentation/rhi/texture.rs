//! 跨图形 API 共用的纹理格式、描述与原生尺寸契约。

// 引入统一错误、错误码与结果类型。
use crate::core::error::{Errc, Error, Result};

// 引入同层定义的二维物理尺寸。
use super::RhiExtent;

// 定义 RHI 资源格式，只保留 UIX 当前需要的有限集合。
// 三个变体必须显式保留 Unorm 采样语义，避免后续加入 Srgb 或浮点格式时产生歧义。
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TextureFormat {
    // 定义 premultiplied BGRA 八位格式。
    Bgra8Unorm,
    // 定义 premultiplied RGBA 八位格式。
    Rgba8Unorm,
    // 定义单通道覆盖率格式。
    R8Unorm,
}

// 为有限格式闭集提供 API 无关的存储与目标能力事实。
impl TextureFormat {
    // 返回紧密排列像素的固定字节宽度。
    pub(crate) const fn bytes_per_pixel(self) -> usize {
        // 穷尽映射共享格式，不把字节宽度留给 Adapter 猜测。
        match self {
            // BGRA 使用四个八位通道。
            Self::Bgra8Unorm => 4,
            // RGBA 使用四个八位通道。
            Self::Rgba8Unorm => 4,
            // 覆盖率纹理只使用一个八位通道。
            Self::R8Unorm => 1,
        }
    }

    // 判断格式是否属于两个 Adapter 都能作为颜色目标的闭集。
    pub(crate) const fn supports_render_target(self) -> bool {
        // R8 当前只作为采样覆盖率纹理，两个四通道格式才拥有颜色目标视图。
        matches!(self, Self::Bgra8Unorm | Self::Rgba8Unorm)
    }
}

// 定义不能被调用方拆散修改的纹理创建描述。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextureDesc {
    // 保存纹理的二维物理尺寸。
    extent: RhiExtent,
    // 保存纹理的通用格式。
    format: TextureFormat,
}

// 为纹理描述提供封闭构造、只读事实与共同原生值域验证。
impl TextureDesc {
    // 创建一个 API 无关的二维纹理描述。
    pub(crate) const fn new(extent: RhiExtent, format: TextureFormat) -> Self {
        // 把尺寸与格式绑定成一个不可拆资源事实。
        Self {
            // 保存物理尺寸。
            extent,
            // 保存通用格式。
            format,
        }
    }

    // 返回纹理的二维物理尺寸。
    pub(crate) const fn extent(self) -> RhiExtent {
        // 尺寸按值安全复制。
        self.extent
    }

    // 返回纹理的通用格式。
    pub(crate) const fn format(self) -> TextureFormat {
        // 格式按值安全复制。
        self.format
    }

    // 验证两个 Adapter 都能表达的非空二维尺寸。
    pub(crate) fn validate(self) -> Result<RhiTextureNativeDesc> {
        // 共同值域由共享几何 Component 的有符号原生投影唯一决定。
        let Some((width, height)) = self.extent.native_size_i32() else {
            // 返回不泄漏具体图形 API 的稳定参数错误。
            return Err(invalid_texture(
                "RHI texture extent is outside the common native range",
            ));
        };
        // 返回 OpenGL 可直接消费且 D3D11 已共同接受的尺寸投影。
        Ok(RhiTextureNativeDesc {
            // 保存有符号宽度。
            width,
            // 保存有符号高度。
            height,
        })
    }
}

// 保存纹理尺寸到共同原生值域的已验证投影。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RhiTextureNativeDesc {
    // 保存 OpenGL 入口消费的有符号宽度。
    width: i32,
    // 保存 OpenGL 入口消费的有符号高度。
    height: i32,
}

// 为原生纹理描述提供只读投影。
impl RhiTextureNativeDesc {
    // 返回 OpenGL 可直接消费的二维尺寸。
    pub(crate) const fn size_i32(self) -> (i32, i32) {
        // 两个分量已经通过同一共同门禁。
        (self.width, self.height)
    }
}

// 生成跨 Adapter 共用的纹理描述参数错误。
fn invalid_texture(message: &'static str) -> Error {
    // 使用稳定错误码区分描述违例和平台资源失败。
    Error::new(Errc::InvalidArgument, message)
}
