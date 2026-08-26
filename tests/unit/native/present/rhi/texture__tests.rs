// 引入当前模块全部私有契约。
use super::*;

// 验证合法描述生成共同有符号原生尺寸。
#[test]
fn desc_projects_common_native_size() {
    // 创建一个有效 RGBA 纹理描述。
    let desc = TextureDesc::new(RhiExtent::new(4, 5), TextureFormat::Rgba8Unorm);
    // 执行唯一共享描述门禁。
    let native = desc.validate().expect("texture desc should validate");
    // OpenGL 投影必须保持原始宽高。
    assert_eq!(native.size_i32(), (4, 5));
    // 描述访问器必须保持原始尺寸。
    assert_eq!(desc.extent(), RhiExtent::new(4, 5));
    // 描述访问器必须保持原始格式。
    assert_eq!(desc.format(), TextureFormat::Rgba8Unorm);
}

// 验证空尺寸和超出共同值域的尺寸统一失败。
#[test]
fn desc_rejects_cross_backend_extent_divergence() {
    // 零宽度不能只由某个 Adapter 解释为空资源。
    assert!(
        TextureDesc::new(RhiExtent::new(0, 1), TextureFormat::R8Unorm)
            // 执行共享门禁。
            .validate()
            // 结果必须是参数错误。
            .is_err()
    );
    // 超过 OpenGL 有符号值域的宽度不能只被 D3D11 接受。
    assert!(
        TextureDesc::new(
            // 构造超出共同宽度值域的尺寸。
            RhiExtent::new(i32::MAX as u32 + 1, 1),
            // 格式本身保持合法。
            TextureFormat::Bgra8Unorm,
        )
        // 执行共享门禁。
        .validate()
        // 结果必须是参数错误。
        .is_err()
    );
}

// 验证像素宽度与颜色目标能力只有一个共享所有者。
#[test]
fn formats_own_storage_and_render_target_facts() {
    // 两个四通道格式都固定为四字节像素。
    assert_eq!(TextureFormat::Bgra8Unorm.bytes_per_pixel(), 4);
    // RGBA 也必须使用相同的四字节宽度。
    assert_eq!(TextureFormat::Rgba8Unorm.bytes_per_pixel(), 4);
    // 覆盖率格式固定为单字节像素。
    assert_eq!(TextureFormat::R8Unorm.bytes_per_pixel(), 1);
    // BGRA 可以成为两个 Adapter 的颜色目标。
    assert!(TextureFormat::Bgra8Unorm.supports_render_target());
    // RGBA 也可以成为颜色目标。
    assert!(TextureFormat::Rgba8Unorm.supports_render_target());
    // R8 只属于采样覆盖率路径。
    assert!(!TextureFormat::R8Unorm.supports_render_target());
}
