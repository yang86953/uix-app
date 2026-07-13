//! 图片解码 — 将 PNG/JPEG 等格式转为引擎像素缓冲（AARRGGBB 预乘 alpha）。

use crate::core::error::{Errc, Error};
use crate::draw::Color;

/// 将编码图片字节解码为 premultiplied AARRGGBB 像素缓冲。
pub fn decode_to_pixels(data: &[u8]) -> Result<(i32, i32, Vec<u32>), Error> {
    let img = image::load_from_memory(data)
        .map_err(|e| Error::new(Errc::InvalidArgument, format!("图片解码失败: {e}")))?;
    let rgba = img.to_rgba8();
    let w = rgba.width() as i32;
    let h = rgba.height() as i32;
    if w <= 0 || h <= 0 {
        return Err(Error::new(Errc::InvalidArgument, "图片尺寸无效"));
    }
    let pixels = rgba
        .chunks_exact(4)
        .map(|px| Color::from_rgba(px[0], px[1], px[2], px[3]).premultiplied())
        .collect();
    Ok((w, h, pixels))
}

