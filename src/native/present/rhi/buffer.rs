//! 跨图形 API 共用的 Buffer 描述、上传与原生值域契约。

// 引入统一错误、错误码与结果类型。
use crate::core::error::{Errc, Error, Result};

// 引入同层定义的不透明 Buffer 句柄。
use super::BufferHandle;

// 定义 Buffer 的底层用途，Adapter 只需据此选择绑定旗标。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BufferUsage {
    // 声明顶点 Buffer。
    Vertex,
    // 声明索引 Buffer。
    Index,
    // 声明常量或动态 Uniform Buffer。
    Uniform,
}

// 定义不能被调用方拆散修改的 Buffer 创建描述。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BufferDesc {
    // 保存 Buffer 的字节容量。
    size_bytes: usize,
    // 保存 Buffer 的单元素步长。
    stride_bytes: u32,
    // 保存 Buffer 的底层用途。
    usage: BufferUsage,
}

// 为 Buffer 描述提供用途化构造、只读事实和共同值域验证。
impl BufferDesc {
    // 创建一个按完整顶点元素组织的 Buffer 描述。
    pub(crate) const fn vertex(size_bytes: usize, stride_bytes: u32) -> Self {
        // 由封闭构造器绑定顶点用途。
        Self {
            // 保存调用方请求的容量。
            size_bytes,
            // 保存顶点 ABI 步长。
            stride_bytes,
            // 固定为顶点用途。
            usage: BufferUsage::Vertex,
        }
    }

    // 创建一个按完整索引元素组织的 Buffer 描述。
    pub(crate) const fn index(size_bytes: usize, stride_bytes: u32) -> Self {
        // 由封闭构造器绑定索引用途。
        Self {
            // 保存调用方请求的容量。
            size_bytes,
            // 保存索引 ABI 步长。
            stride_bytes,
            // 固定为索引用途。
            usage: BufferUsage::Index,
        }
    }

    // 创建一个遵循共同常量块 ABI 的 Uniform Buffer 描述。
    pub(crate) const fn uniform(size_bytes: usize) -> Self {
        // Uniform 不暴露无意义的元素步长。
        Self {
            // 保存调用方请求的容量。
            size_bytes,
            // Uniform 的原生结构步长固定为零。
            stride_bytes: 0,
            // 固定为 Uniform 用途。
            usage: BufferUsage::Uniform,
        }
    }

    // 返回 Buffer 的字节容量。
    pub(crate) const fn size_bytes(self) -> usize {
        // 容量按值安全复制。
        self.size_bytes
    }

    // 返回顶点或索引元素步长。
    pub(crate) const fn stride_bytes(self) -> u32 {
        // 步长按值安全复制。
        self.stride_bytes
    }

    // 返回 Buffer 用途。
    pub(crate) const fn usage(self) -> BufferUsage {
        // 枚举按值安全复制。
        self.usage
    }

    // 验证两个 Adapter 都能表达的容量、步长与 Uniform ABI。
    pub(crate) fn validate(self) -> Result<RhiBufferNativeDesc> {
        // OpenGL 入口使用有符号尺寸，因此共同容量不得超过 i32。
        if self.size_bytes == 0 || self.size_bytes > i32::MAX as usize {
            // 返回与具体图形 API 无关的稳定参数错误。
            return Err(invalid_buffer(
                "RHI buffer size is outside the common native range",
            ));
        }
        // 按用途验证元素边界或 Uniform 常量块边界。
        match self.usage {
            // 顶点和索引 Buffer 必须由完整的非零步长元素组成。
            BufferUsage::Vertex | BufferUsage::Index => {
                // GL 顶点入口使用有符号步长，两个 Adapter 共用同一值域。
                if self.stride_bytes == 0 || self.stride_bytes > i32::MAX as u32 {
                    // 拒绝无法成为原生元素 ABI 的步长。
                    return Err(invalid_buffer(
                        "RHI buffer stride is outside the common native range",
                    ));
                }
                // 资源容量不能在最后留下一个不完整元素。
                if self.size_bytes % self.stride_bytes as usize != 0 {
                    // 拒绝由 Adapter 各自猜测尾部字节语义。
                    return Err(invalid_buffer(
                        "RHI buffer capacity is not aligned to its element stride",
                    ));
                }
            }
            // Uniform 在 D3D11 与 OpenGL 路径都使用相同的 16 字节常量块 ABI。
            BufferUsage::Uniform => {
                // Uniform 描述不能携带顶点或索引步长。
                if self.stride_bytes != 0 {
                    // 拒绝用途与步长事实互相矛盾。
                    return Err(invalid_buffer("RHI uniform buffer stride must be zero"));
                }
                // 常量块容量必须按 D3D11 的共同 ABI 对齐。
                if !self.size_bytes.is_multiple_of(16) {
                    // 让 OpenGL 与 D3D11 接受完全相同的 Uniform 描述。
                    return Err(invalid_buffer(
                        "RHI uniform buffer size must be aligned to 16 bytes",
                    ));
                }
            }
        }
        // 一次生成两个 Adapter 直接消费的无损原生投影。
        Ok(RhiBufferNativeDesc {
            // 上方已证明容量不超过 i32，因此转换为 u32 也无损。
            size_bytes_u32: self.size_bytes as u32,
            // 上方已证明容量位于正 i32 值域。
            size_bytes_i32: self.size_bytes as i32,
        })
    }
}

// 保存 Buffer 容量到两个原生 API 的已验证投影。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RhiBufferNativeDesc {
    // 保存 D3D11 入口消费的无符号容量。
    size_bytes_u32: u32,
    // 保存 OpenGL 入口消费的有符号容量。
    size_bytes_i32: i32,
}

// 为原生描述投影提供只读访问。
impl RhiBufferNativeDesc {
    // 返回 D3D11 Buffer 容量。
    pub(crate) const fn size_bytes_u32(self) -> u32 {
        // 投影已经通过共同门禁。
        self.size_bytes_u32
    }

    // 返回 OpenGL Buffer 容量。
    pub(crate) const fn size_bytes_i32(self) -> i32 {
        // 投影已经通过共同门禁。
        self.size_bytes_i32
    }
}

// 描述一次从字节零开始、不能拆散句柄与载荷的 Buffer 前缀上传。
#[derive(Debug, Clone, Copy)]
pub(crate) struct RhiBufferUpload<'a> {
    // 保存目标 Buffer 身份。
    buffer: BufferHandle,
    // 保存调用期间有效的不可变上传字节。
    data: &'a [u8],
}

// 为 Buffer 上传提供封闭构造、只读身份与共享范围验证。
impl<'a> RhiBufferUpload<'a> {
    // 创建一次从 Buffer 起点开始的上传。
    pub(crate) const fn new(buffer: BufferHandle, data: &'a [u8]) -> Self {
        // 把资源身份与载荷绑定成一个命令值。
        Self {
            // 保存目标身份。
            buffer,
            // 保存不可变载荷借用。
            data,
        }
    }

    // 返回目标 Buffer 身份。
    pub(crate) const fn buffer(self) -> BufferHandle {
        // 句柄按值安全复制。
        self.buffer
    }

    // 返回未经资源容量验证的原始载荷。
    pub(crate) const fn data(self) -> &'a [u8] {
        // 借用生命周期与上传命令一致。
        self.data
    }

    // 按目标描述验证非空范围、元素边界和 Uniform 完整替换语义。
    pub(crate) fn validate(self, desc: BufferDesc) -> Result<ValidatedRhiBufferUpload<'a>> {
        // 上传值对象独立复用创建门禁，防止未经 Adapter 创建路径的非法描述被窄化。
        desc.validate()?;
        // 空上传不能绕过句柄解析或伪装成资源更新成功。
        if self.data.is_empty() {
            // 统一拒绝两个 Adapter 原先不同的空上传行为。
            return Err(invalid_buffer("RHI buffer upload payload is empty"));
        }
        // 上传前缀必须完整落在资源容量内。
        if self.data.len() > desc.size_bytes {
            // 拒绝越过目标 Buffer 尾部。
            return Err(invalid_buffer("RHI buffer upload exceeds its capacity"));
        }
        // 按用途验证上传元素边界与 Uniform 替换粒度。
        match desc.usage {
            // 顶点和索引上传不能留下半个元素。
            BufferUsage::Vertex | BufferUsage::Index => {
                // 描述先由创建门禁验证，上传仍防止错误描述导致除零。
                if desc.stride_bytes == 0
                    // 载荷必须包含整数个元素。
                    || self.data.len() % desc.stride_bytes as usize != 0
                {
                    // 拒绝由 Adapter 各自补齐或截断元素。
                    return Err(invalid_buffer(
                        "RHI buffer upload is not aligned to its element stride",
                    ));
                }
            }
            // Uniform 只能通过完整常量块替换更新。
            BufferUsage::Uniform => {
                // 两个 Adapter 都必须覆盖整个 Uniform Buffer。
                if self.data.len() != desc.size_bytes {
                    // 禁止 OpenGL 私自接受局部 Uniform 更新。
                    return Err(invalid_buffer(
                        "RHI uniform upload must replace the full buffer",
                    ));
                }
            }
        }
        // 载荷长度不超过共同容量，因此可安全投影到 D3D11 box。
        Ok(ValidatedRhiBufferUpload {
            // 保留调用期间有效的字节切片。
            data: self.data,
            // 描述容量已受 i32 门禁约束，载荷长度也不会超过它。
            size_bytes_u32: self.data.len() as u32,
        })
    }
}

// 保存已经通过资源用途与容量门禁的上传载荷。
#[derive(Debug, Clone, Copy)]
pub(crate) struct ValidatedRhiBufferUpload<'a> {
    // 保存可以直接交给原生 API 的字节切片。
    data: &'a [u8],
    // 保存 D3D11 box 右边界的无损投影。
    size_bytes_u32: u32,
}

// 为已验证上传提供 Adapter 只读投影。
impl<'a> ValidatedRhiBufferUpload<'a> {
    // 返回已经完成范围校验的上传字节。
    pub(crate) const fn data(self) -> &'a [u8] {
        // 借用可以在同步原生调用期间直接使用。
        self.data
    }

    // 返回 D3D11 更新区域的右边界。
    pub(crate) const fn size_bytes_u32(self) -> u32 {
        // 上传固定从零开始，因此长度就是右边界。
        self.size_bytes_u32
    }
}

// 生成跨 Adapter 共用的 Buffer 参数错误。
fn invalid_buffer(message: &'static str) -> Error {
    // 使用稳定错误码区分调用契约问题和平台失败。
    Error::new(Errc::InvalidArgument, message)
}

// 验证 Buffer 描述与上传值对象不会因 Adapter 不同而改变语义。
#[cfg(test)]
mod tests {
    // 引入当前模块的私有构造以覆盖非法状态门禁。
    use super::*;

    // 验证顶点描述生成两个原生 API 共用的无损投影。
    #[test]
    fn vertex_desc_projects_common_native_sizes() {
        // 创建两个 float2 顶点的合法描述。
        let desc = BufferDesc::vertex(16, 8);
        // 执行唯一共享门禁。
        let native = desc.validate().expect("vertex desc should validate");
        // 两个原生值域必须表达同一个容量事实。
        assert_eq!(native.size_bytes_u32(), 16);
        // OpenGL 投影不能重新解释容量。
        assert_eq!(native.size_bytes_i32(), 16);
        // 描述访问器保持调用方输入。
        assert_eq!(desc.usage(), BufferUsage::Vertex);
    }

    // 验证创建门禁统一拒绝非法容量、步长和 Uniform 对齐。
    #[test]
    fn desc_rejects_cross_backend_divergence() {
        // 零容量在两个 Adapter 上都非法。
        assert!(BufferDesc::vertex(0, 8).validate().is_err());
        // 超过 OpenGL 有符号值域的容量不能只被 D3D11 接受。
        assert!(
            BufferDesc::vertex(i32::MAX as usize + 1, 1)
                .validate()
                .is_err()
        );
        // 非 Uniform Buffer 必须携带有效元素步长。
        assert!(BufferDesc::vertex(16, 0).validate().is_err());
        // 容量必须由完整元素组成。
        assert!(BufferDesc::index(6, 4).validate().is_err());
        // Uniform 必须符合共同的 16 字节常量块 ABI。
        assert!(BufferDesc::uniform(20).validate().is_err());
    }

    // 验证顶点上传允许复用较大容量但必须保持完整元素。
    #[test]
    fn vertex_upload_accepts_aligned_prefix() {
        // 创建可容纳四个 float2 顶点的资源描述。
        let desc = BufferDesc::vertex(32, 8);
        // 只上传前两个完整顶点。
        let bytes = [0_u8; 16];
        // 将句柄与载荷绑定成不可拆分命令。
        let upload = RhiBufferUpload::new(BufferHandle::from_raw(7), &bytes);
        // 共享门禁生成 D3D11 直接消费的右边界。
        let validated = upload
            .validate(desc)
            .expect("aligned prefix should validate");
        // 上传身份不能在验证过程中丢失。
        assert_eq!(upload.buffer(), BufferHandle::from_raw(7));
        // 已验证载荷与原始字节完全一致。
        assert_eq!(validated.data(), bytes);
        // 前缀右边界来自共享投影而非 Adapter 窄化。
        assert_eq!(validated.size_bytes_u32(), 16);
    }

    // 验证上传门禁统一拒绝空载荷、半元素、越界与局部 Uniform。
    #[test]
    fn upload_rejects_backend_specific_shortcuts() {
        // 空上传不能在 D3D11 上提前成功。
        assert!(
            RhiBufferUpload::new(BufferHandle::from_raw(1), &[])
                // 使用合法顶点描述验证空载荷。
                .validate(BufferDesc::vertex(16, 8))
                // 结果必须是参数错误。
                .is_err()
        );
        // 三字节载荷不是完整的四字节索引。
        assert!(
            RhiBufferUpload::new(BufferHandle::from_raw(1), &[0; 3])
                // 使用四字节索引步长。
                .validate(BufferDesc::index(16, 4))
                // 两个 Adapter 都应拒绝。
                .is_err()
        );
        // 超过资源容量的载荷不能进入原生 API。
        assert!(
            RhiBufferUpload::new(BufferHandle::from_raw(1), &[0; 24])
                // 目标只分配十六字节。
                .validate(BufferDesc::vertex(16, 8))
                // 共享范围门禁必须失败。
                .is_err()
        );
        // OpenGL 不再独自接受局部 Uniform 更新。
        assert!(
            RhiBufferUpload::new(BufferHandle::from_raw(1), &[0; 16])
                // 目标常量块为三十二字节。
                .validate(BufferDesc::uniform(32))
                // 必须要求完整替换。
                .is_err()
        );
    }
}
