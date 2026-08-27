//! Agent 截屏载荷编码：规范像素 → PNG → base64。
//!
//! 回读票据交付的是 Drawing 层规范像素（`0xAARRGGBB`、左上原点、逐行紧密
//! 排列）；本模块只做有界编码，不触碰渲染边界，也不解析协议帧。

use crate::draw::SurfaceReadback;

/// 协议单条截屏回复允许的最大 PNG 载荷。
///
/// 以 8K 物理分辨率的复杂 UI 内容为上限基准；超过时按协议错误拒绝，
/// 不静默截断或缩放。
pub(crate) const MAX_SCREENSHOT_PNG_BYTES: usize = 32 * 1024 * 1024;

/// 截屏编码失败语义；协议层据此映射为稳定错误码。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScreenshotEncodeError {
    /// 回读载荷尺寸非法（零尺寸或像素长度与表面尺寸不符）。
    InvalidReadback,
    /// PNG 编码失败。
    EncodeFailure,
    /// 编码结果超出协议载荷上限。
    PayloadTooLarge,
}

/// 把规范像素编码为 RGB8 PNG，并执行协议载荷上限检查。
pub(crate) fn encode_png_bounded(
    readback: &SurfaceReadback,
) -> Result<Vec<u8>, ScreenshotEncodeError> {
    // 回读契约保证正尺寸与精确载荷长度；此处仍按边界校验防御。
    if readback.width <= 0 || readback.height <= 0 {
        return Err(ScreenshotEncodeError::InvalidReadback);
    }
    let width = readback.width as usize;
    let height = readback.height as usize;
    if readback.pixels.len() != width.saturating_mul(height) {
        return Err(ScreenshotEncodeError::InvalidReadback);
    }
    // `0xAARRGGBB` 紧密像素展开为逐字节 RGB8 行缓冲。
    let mut rgb = Vec::with_capacity(readback.pixels.len() * 3);
    for pixel in &readback.pixels {
        rgb.push((pixel >> 16) as u8);
        rgb.push((pixel >> 8) as u8);
        rgb.push(*pixel as u8);
    }
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, width as u32, height as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|_| ScreenshotEncodeError::EncodeFailure)?;
        writer
            .write_image_data(&rgb)
            .map_err(|_| ScreenshotEncodeError::EncodeFailure)?;
        writer
            .finish()
            .map_err(|_| ScreenshotEncodeError::EncodeFailure)?;
    }
    if png.len() > MAX_SCREENSHOT_PNG_BYTES {
        return Err(ScreenshotEncodeError::PayloadTooLarge);
    }
    Ok(png)
}

/// 标准 base64（RFC 4648 字母表，带填充）。
pub(crate) fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        encoded.push(TABLE[(triple >> 18) as usize & 0x3F] as char);
        encoded.push(TABLE[(triple >> 12) as usize & 0x3F] as char);
        encoded.push(if chunk.len() > 1 {
            TABLE[(triple >> 6) as usize & 0x3F] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            TABLE[triple as usize & 0x3F] as char
        } else {
            '='
        });
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_encode_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn encode_png_bounded_rejects_invalid_readback() {
        // 合法载荷应能编码，随后破坏尺寸与载荷一致性必须被拒绝。
        let mut readback = SurfaceReadback::from_argb(2, 2, vec![0xFF000000; 4]);
        assert!(encode_png_bounded(&readback).is_ok());
        readback.width = 0;
        assert_eq!(
            encode_png_bounded(&readback),
            Err(ScreenshotEncodeError::InvalidReadback)
        );
        readback.width = 2;
        readback.pixels.truncate(3);
        assert_eq!(
            encode_png_bounded(&readback),
            Err(ScreenshotEncodeError::InvalidReadback)
        );
    }

    #[test]
    fn encode_png_bounded_produces_decodable_image() {
        // 2x2 红/绿/蓝/白像素：验证 PNG 头与解码回路。
        let pixels = vec![0xFFFF0000, 0xFF00FF00, 0xFF0000FF, 0xFFFFFFFF];
        let readback = SurfaceReadback::from_argb(2, 2, pixels);
        let png = encode_png_bounded(&readback).expect("valid readback must encode");
        assert_eq!(&png[0..8], b"\x89PNG\r\n\x1a\n");
        let decoder = png::Decoder::new(std::io::Cursor::new(&png[..]));
        let mut reader = decoder.read_info().expect("encoded png must be decodable");
        assert_eq!(reader.info().width, 2);
        assert_eq!(reader.info().height, 2);
        assert_eq!(reader.info().color_type, png::ColorType::Rgb);
        let buffer_size = reader
            .output_buffer_size()
            .expect("in-memory frame has a known buffer size");
        let mut decoded = vec![0u8; buffer_size];
        reader.next_frame(&mut decoded).expect("frame must decode");
        // 红、绿、蓝、白按行展开的 RGB 字节。
        assert_eq!(
            &decoded[..12],
            &[0xFF, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF]
        );
    }
}
