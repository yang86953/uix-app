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
#[path = "../../../tests-src/app/agent/agent_screenshot_tests.rs"]
mod tests;

