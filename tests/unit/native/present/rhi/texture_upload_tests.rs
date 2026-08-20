//! 纹理上传资源身份、区域、载荷与紧密布局的共享契约测试。

// 引入父 Transfer Widget 的上传值对象及其共享依赖。
use super::{
    RhiExtent, RhiTextureRegion, RhiTextureUpload, TextureDesc, TextureFormat, TextureHandle,
};

// 创建稳定的测试纹理身份。
const TEXTURE: TextureHandle = TextureHandle::from_raw(1);

// 创建指定尺寸和格式的共享纹理描述。
const fn texture(width: u32, height: u32, format: TextureFormat) -> TextureDesc {
    // 返回没有原生 API 状态的资源事实。
    TextureDesc::new(RhiExtent::new(width, height), format)
}

// 验证纹理上传原子保存身份、区域、载荷与紧密行跨度。
#[test]
fn upload_returns_checked_region_payload_and_row_pitch() {
    // 创建两个 RGBA 像素的紧密载荷。
    let pixels = [0_u8; 8];
    // 创建写入目标第二列的二像素区域上传。
    let upload = RhiTextureUpload::new(
        // 绑定稳定纹理身份。
        TEXTURE,
        // 区域宽二、高一并保持在四像素资源内。
        RhiTextureRegion::from_xy(1, 0, RhiExtent::new(2, 1)),
        // 绑定完整像素载荷。
        &pixels,
    );
    // 使用四通道资源描述执行共享门禁。
    let validated = upload
        // 目标资源为四乘四 RGBA。
        .validate(texture(4, 4, TextureFormat::Rgba8Unorm))
        // 合法上传必须通过。
        .expect("texture upload should validate");
    // 上传命令必须保持目标纹理身份。
    assert_eq!(upload.texture(), TEXTURE);
    // 目标区域必须保持原点与尺寸。
    assert_eq!(validated.bounds().native_rect_u32(), (1, 0, 3, 1));
    // 载荷不能在验证过程中改变。
    assert_eq!(validated.data(), pixels);
    // 两个四字节像素形成八字节紧密行跨度。
    assert_eq!(validated.row_pitch(), 8);
}

// 验证空载荷、短载荷和尾部字节统一失败。
#[test]
fn upload_rejects_backend_specific_payload_shortcuts() {
    // 空载荷不能在任何 Adapter 中成为成功 no-op。
    assert!(
        RhiTextureUpload::full(TEXTURE, RhiExtent::new(1, 1), &[])
            // 单通道目标需要一个字节。
            .validate(texture(1, 1, TextureFormat::R8Unorm))
            // 共享门禁必须拒绝。
            .is_err()
    );
    // 三字节载荷不能只在某个四通道 Adapter 中被补齐。
    assert!(
        RhiTextureUpload::full(TEXTURE, RhiExtent::new(1, 1), &[0; 3])
            // 四通道目标精确需要四字节。
            .validate(texture(1, 1, TextureFormat::Bgra8Unorm))
            // 共享门禁必须拒绝。
            .is_err()
    );
    // 五字节载荷的尾部字节也不能被静默忽略。
    assert!(
        RhiTextureUpload::full(TEXTURE, RhiExtent::new(1, 1), &[0; 5])
            // 四通道目标仍只需要四字节。
            .validate(texture(1, 1, TextureFormat::Rgba8Unorm))
            // 共享门禁必须拒绝。
            .is_err()
    );
}
