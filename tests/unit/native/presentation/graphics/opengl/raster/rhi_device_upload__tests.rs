// 引入被测辅助函数。
use super::normalize_upload_payload;
// 引入共享纹理格式。
use crate::platform::presentation::rhi::TextureFormat;

// 锁定 BGRA 载荷进入 RGBA8 存储前只交换红蓝通道。
#[test]
fn normalizes_bgra_payload_to_rgba_storage() {
    // 构造两个包含不同通道值的 BGRA 像素。
    let bgra = [0x33, 0x22, 0x11, 0x44, 0x77, 0x66, 0x55, 0x88];
    // 执行 Adapter 私有规范化。
    let rgba = normalize_upload_payload(TextureFormat::Bgra8Unorm, &bgra);
    // 每个像素只交换首尾颜色通道，alpha 不变。
    assert_eq!(
        rgba.as_ref(),
        [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]
    );
}

// 锁定已经是 RGBA 的载荷保持零复制与原序。
#[test]
fn borrows_rgba_payload_without_channel_changes() {
    // 构造一个 RGBA 像素。
    let rgba = [0x11, 0x22, 0x33, 0x44];
    // 执行无需转换的 Adapter 路径。
    let normalized = normalize_upload_payload(TextureFormat::Rgba8Unorm, &rgba);
    // RGBA 载荷必须保持原始字节顺序。
    assert_eq!(normalized.as_ref(), rgba);
    // 未转换路径必须继续借用调用方切片。
    assert!(matches!(normalized, std::borrow::Cow::Borrowed(_)));
}
