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

// 描述一次只读 Buffer 上传预检，隔离资源句柄与字节载荷生命周期。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RhiBufferUploadPreflight {
    // 保存目标 Buffer 身份。
    buffer: BufferHandle,
    // 保存待验证的上传字节数。
    size_bytes: usize,
    // 保存类型化命令要求的 Buffer 用途。
    usage: BufferUsage,
    // 保存类型化命令已经冻结的元素步长，Uniform 固定为零。
    element_stride_bytes: u32,
}

// 为只读上传预检提供封闭构造、投影和真实描述验证。
impl RhiBufferUploadPreflight {
    // 创建只允许写入顶点 Buffer 的类型化预检值。
    pub(crate) const fn vertex(
        // 保存目标顶点 Buffer 身份。
        buffer: BufferHandle,
        // 保存本次类型化载荷的精确字节数。
        size_bytes: usize,
        // 保存顶点布局派生的唯一元素步长。
        element_stride_bytes: u32,
    ) -> Self {
        // 通过封闭构造器冻结顶点用途与元素 ABI。
        Self::new(
            // 传递不透明顶点 Buffer 身份。
            buffer,
            // 传递类型化载荷长度。
            size_bytes,
            // 固定顶点资源角色。
            BufferUsage::Vertex,
            // 传递由顶点布局权威派生的步长。
            element_stride_bytes,
        )
    }

    // 创建只允许写入索引 Buffer 的类型化预检值。
    pub(crate) const fn index(
        // 保存目标索引 Buffer 身份。
        buffer: BufferHandle,
        // 保存本次类型化载荷的精确字节数。
        size_bytes: usize,
        // 保存索引格式派生的唯一元素步长。
        element_stride_bytes: u32,
    ) -> Self {
        // 通过封闭构造器冻结索引用途与元素 ABI。
        Self::new(
            // 传递不透明索引 Buffer 身份。
            buffer,
            // 传递类型化载荷长度。
            size_bytes,
            // 固定索引资源角色。
            BufferUsage::Index,
            // 传递由索引格式权威派生的步长。
            element_stride_bytes,
        )
    }

    // 创建只允许写入 Uniform Buffer 的类型化预检值。
    pub(crate) const fn uniform(buffer: BufferHandle, size_bytes: usize) -> Self {
        // 通过封闭构造器冻结 Uniform 用途与零步长 ABI。
        Self::new(
            // 传递不透明 Uniform Buffer 身份。
            buffer,
            // 传递固定布局载荷长度。
            size_bytes,
            // 固定 Uniform 资源角色。
            BufferUsage::Uniform,
            // Uniform 描述不使用元素步长。
            0,
        )
    }

    // 创建不持有字节借用且冻结期望用途和元素 ABI 的内部预检值。
    const fn new(
        // 接收由资源表解析的目标身份。
        buffer: BufferHandle,
        // 接收不需要字节分配的载荷长度。
        size_bytes: usize,
        // 接收类型化命令已经冻结的资源角色。
        usage: BufferUsage,
        // 接收由上层类型化布局派生的元素步长。
        element_stride_bytes: u32,
    ) -> Self {
        // 把目标身份、载荷长度、用途与元素 ABI 绑定成一个不可拆值对象。
        Self {
            // 保存目标 Buffer 身份。
            buffer,
            // 保存待验证的上传字节数。
            size_bytes,
            // 保存类型化调用方要求的用途。
            usage,
            // 保存调用方值对象已冻结的元素步长。
            element_stride_bytes,
        }
    }

    // 返回目标 Buffer 身份。
    pub(crate) const fn buffer(self) -> BufferHandle {
        // 句柄按值安全复制。
        self.buffer
    }

    // 返回待验证的上传字节数。
    pub(crate) const fn size_bytes(self) -> usize {
        // 长度按值安全复制。
        self.size_bytes
    }

    // 按真实 Buffer 描述验证上传范围、用途和元素 ABI 语义。
    pub(crate) fn validate(self, desc: BufferDesc) -> Result<()> {
        // 类型化 FramePlan 角色必须与资源创建时冻结的真实用途一致。
        if self.usage != desc.usage {
            // 禁止 Adapter 把顶点、索引或 Uniform 上传解释成另一种角色。
            return Err(invalid_buffer(
                "RHI buffer upload usage does not match its target",
            ));
        }
        // 用途一致后还必须比较载荷布局与真实资源的元素 ABI。
        if self.element_stride_bytes != desc.stride_bytes {
            // 禁止 Adapter 按真实资源的另一步长重新解释类型化载荷。
            return Err(invalid_buffer(
                "RHI buffer upload element stride does not match its target",
            ));
        }
        // 用途与元素 ABI 一致后复用原始上传也消费的唯一范围验证函数。
        validate_upload_size(self.size_bytes, desc)
    }
}

// 统一验证原始上传和类型化预检共用的容量、元素与完整替换语义。
fn validate_upload_size(size_bytes: usize, desc: BufferDesc) -> Result<()> {
    // 复用唯一 Buffer 描述创建门禁。
    desc.validate()?;
    // 空上传不能绕过句柄解析或伪装成资源更新成功。
    if size_bytes == 0 {
        // 统一拒绝两个 Adapter 原先不同的空上传行为。
        return Err(invalid_buffer("RHI buffer upload payload is empty"));
    }
    // 上传前缀必须完整落在资源容量内。
    if size_bytes > desc.size_bytes {
        // 拒绝越过目标 Buffer 尾部。
        return Err(invalid_buffer("RHI buffer upload exceeds its capacity"));
    }
    // 按用途验证上传元素边界与 Uniform 替换粒度。
    match desc.usage {
        // 顶点和索引 Buffer 只能接收完整元素。
        BufferUsage::Vertex | BufferUsage::Index => {
            // 描述先由创建门禁验证，仍防止错误描述导致除零。
            if desc.stride_bytes == 0
                // 载荷必须包含整数个元素。
                || size_bytes % desc.stride_bytes as usize != 0
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
            if size_bytes != desc.size_bytes {
                // 禁止 OpenGL 私自接受局部 Uniform 更新。
                return Err(invalid_buffer(
                    "RHI uniform upload must replace the full buffer",
                ));
            }
        }
    }
    // 真实描述已经证明该上传范围可交付 Adapter。
    Ok(())
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
        // 与类型化预检复用同一唯一范围验证函数。
        validate_upload_size(self.data.len(), desc)?;
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
