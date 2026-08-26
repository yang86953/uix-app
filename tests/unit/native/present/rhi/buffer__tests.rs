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

// 验证只读上传预检值冻结目标身份、字节数、用途与元素 ABI。
#[test]
fn upload_preflight_projects_identity_and_size() {
    // 创建携带稳定句柄、十六字节范围与 float2 步长的顶点预检。
    let preflight = RhiBufferUploadPreflight::vertex(BufferHandle::from_raw(8), 16, 8);
    // 预检值必须保留目标句柄。
    assert_eq!(preflight.buffer(), BufferHandle::from_raw(8));
    // 预检值必须保留原始字节数。
    assert_eq!(preflight.size_bytes(), 16);
    // 真实顶点描述必须接受同一预检值。
    assert!(preflight.validate(BufferDesc::vertex(16, 8)).is_ok());
    // 用途与容量相同但步长不匹配的顶点资源必须拒绝。
    assert!(preflight.validate(BufferDesc::vertex(16, 4)).is_err());
    // 同尺寸 Uniform 目标也必须因类型化用途不匹配而拒绝。
    assert!(preflight.validate(BufferDesc::uniform(16)).is_err());
    // 索引用途化构造器必须接受匹配的真实索引描述。
    assert!(
        RhiBufferUploadPreflight::index(BufferHandle::from_raw(9), 8, 4)
            // 使用两个四字节索引的真实描述完成验证。
            .validate(BufferDesc::index(8, 4))
            // 匹配身份、范围与用途必须通过。
            .is_ok()
    );
    // 索引用途相同但元素步长不一致时也必须共享拒绝。
    assert!(
        RhiBufferUploadPreflight::index(BufferHandle::from_raw(9), 8, 4)
            // 故意提供两字节索引资源描述。
            .validate(BufferDesc::index(8, 2))
            // 预检不得仅因为载荷长度偶然对齐而放行。
            .is_err()
    );
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
