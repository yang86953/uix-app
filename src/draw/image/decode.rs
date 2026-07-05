//! 图片解码 — 将 PNG/JPEG 等格式转为引擎像素缓冲（AARRGGBB 预乘 alpha）。

use crate::core::error::{Errc, Error};
use crate::draw::Color;

/// 将编码图片字节解码为 premultiplied AARRGGBB 像素缓冲。
pub fn decode_to_pixels(data: &[u8]) -> Result<(i32, i32, Vec<u32>), Error> {
    let img = image::load_from_memory(data).map_err(|e| {
        Error::new(
            Errc::InvalidArgument,
            format!("图片解码失败: {e}"),
        )
    })?;
    let rgba = img.to_rgba8();
    let w = rgba.width() as i32;
    let h = rgba.height() as i32;
    if w <= 0 || h <= 0 {
        return Err(Error::new(Errc::InvalidArgument, "图片尺寸无效"));
    }
    let pixels = rgba
        .chunks_exact(4)
        .map(|px| {
            Color::from_rgba(px[0], px[1], px[2], px[3]).premultiplied()
        })
        .collect();
    Ok((w, h, pixels))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1×1 红色 PNG（标准测试图）。
    const RED_PNG: &[u8] = include_bytes!("../../../tests/fixtures/red_1x1.png");

    #[test]
    fn decode_png_returns_pixels() {
        let (w, h, pixels) = decode_to_pixels(RED_PNG).expect("decode");
        assert_eq!(w, 1);
        assert_eq!(h, 1);
        assert_eq!(pixels.len(), 1);
        let a = (pixels[0] >> 24) & 0xFF;
        assert!(a > 0, "alpha 应大于 0");
    }
}
