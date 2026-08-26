//! 图片解码 — 将 PNG/JPEG 等格式转为引擎像素缓冲（AARRGGBB 预乘 alpha）。

use crate::core::error::{Errc, Error};
/// 将编码图片字节解码为 premultiplied AARRGGBB 像素缓冲。
pub fn decode_to_pixels(data: &[u8]) -> Result<(i32, i32, Vec<u32>), Error> {
    let img = image::load_from_memory(data)
        .map_err(|e| Error::new(Errc::InvalidArgument, format!("图片解码失败: {e}")))?;
    // 消耗已解码图片；PNG 已经是 RGBA8 时直接接管其像素缓冲，避免整图复制。
    let rgba = img.into_rgba8();
    let w = rgba.width() as i32;
    let h = rgba.height() as i32;
    if w <= 0 || h <= 0 {
        return Err(Error::new(Errc::InvalidArgument, "图片尺寸无效"));
    }
    let pixels = rgba
        .chunks_exact(4)
        // 不透明与全透明像素是 UI 图片的高频情形，可跳过三次乘除且保持位级结果。
        .map(premultiply_rgba8)
        .collect();
    Ok((w, h, pixels))
}

// 将一个 RGBA8 像素转换为框架约定的预乘 AARRGGBB。
fn premultiply_rgba8(pixel: &[u8]) -> u32 {
    let red = u32::from(pixel[0]);
    let green = u32::from(pixel[1]);
    let blue = u32::from(pixel[2]);
    let alpha = u32::from(pixel[3]);
    match alpha {
        // 完全透明像素的全部预乘颜色通道均为零。
        0 => 0,
        // 不透明像素只需重排通道，不执行恒等乘除。
        255 => 0xFF00_0000 | (red << 16) | (green << 8) | blue,
        // 半透明像素保持原有整数截断语义。
        _ => {
            let red = red * alpha / 255;
            let green = green * alpha / 255;
            let blue = blue * alpha / 255;
            (alpha << 24) | (red << 16) | (green << 8) | blue
        }
    }
}
