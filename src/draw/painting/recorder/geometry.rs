//! 录制几何与编码辅助 — recorder 子模块。

use crate::core::{Errc, Error, Rect};
use crate::draw::painting::{FrameEncoderError, FrameRect};

/// 把一次透明的全表面 scratch 操作打包进最小的 alpha 可见 tile。给定
/// `scan_bounds` 时只扫描该 AABB——逐操作 flush 原本会在每个字形 / 圆角后
/// 重新扫描整个窗口。
pub(super) fn pack_visible_scratch_tile(
    pixels: &[u32],
    width: i32,
    height: i32,
    scan_bounds: Option<FrameRect>,
) -> Option<(Vec<u32>, FrameRect)> {
    if width <= 0 || height <= 0 {
        return None;
    }
    let expected = (width as usize).checked_mul(height as usize)?;
    if pixels.len() < expected {
        return None;
    }

    let (scan_left, scan_top, scan_right, scan_bottom) = match scan_bounds {
        Some(b) if b.width > 0 && b.height > 0 => {
            let left = b.x.max(0).min(width);
            let top = b.y.max(0).min(height);
            let right = (b.x + b.width).max(0).min(width);
            let bottom = (b.y + b.height).max(0).min(height);
            if left >= right || top >= bottom {
                return None;
            }
            (left, top, right, bottom)
        }
        _ => (0, 0, width, height),
    };

    let mut left = scan_right;
    let mut top = scan_bottom;
    let mut right = scan_left;
    let mut bottom = scan_top;
    for y in scan_top..scan_bottom {
        let row = y as usize * width as usize;
        for x in scan_left..scan_right {
            if pixels[row + x as usize] & 0xff00_0000 == 0 {
                continue;
            }
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    if left >= right || top >= bottom {
        return None;
    }

    let tile_width = right - left;
    let tile_height = bottom - top;
    let mut packed = Vec::with_capacity((tile_width as usize).checked_mul(tile_height as usize)?);
    for y in top..bottom {
        let row = y as usize * width as usize;
        packed.extend_from_slice(&pixels[row + left as usize..row + right as usize]);
    }
    Some((packed, FrameRect::new(left, top, tile_width, tile_height)))
}

/// 把本地 AABB（含 pad 与 clip 限制）映射为 surface 空间的整数打包边界。
pub(super) fn surface_pack_bounds(
    local: Rect,
    pad: f32,
    offset: (f32, f32),
    clip: Rect,
    width: i32,
    height: i32,
) -> Option<FrameRect> {
    if !local.x.is_finite()
        || !local.y.is_finite()
        || !local.w.is_finite()
        || !local.h.is_finite()
        || local.w <= 0.0
        || local.h <= 0.0
    {
        return None;
    }
    let pad = pad.max(0.0);
    let x0 = (local.x + offset.0 - pad).floor();
    let y0 = (local.y + offset.1 - pad).floor();
    let x1 = (local.x + offset.0 + local.w + pad).ceil();
    let y1 = (local.y + offset.1 + local.h + pad).ceil();
    let cx0 = clip.x.floor();
    let cy0 = clip.y.floor();
    let cx1 = (clip.x + clip.w).ceil();
    let cy1 = (clip.y + clip.h).ceil();
    let left = x0.max(cx0).max(0.0) as i32;
    let top = y0.max(cy0).max(0.0) as i32;
    let right = x1.min(cx1).min(width as f32) as i32;
    let bottom = y1.min(cy1).min(height as f32) as i32;
    if left >= right || top >= bottom {
        return None;
    }
    Some(FrameRect::new(left, top, right - left, bottom - top))
}

/// 把整像素 `Rect` 转为 `FrameRect`；非有限或分数坐标返回参数错误。
pub(super) fn rect_to_frame(rect: Rect) -> Result<FrameRect, Error> {
    if !rect.x.is_finite()
        || !rect.y.is_finite()
        || !rect.w.is_finite()
        || !rect.h.is_finite()
        || rect.x.fract() != 0.0
        || rect.y.fract() != 0.0
        || rect.w.fract() != 0.0
        || rect.h.fract() != 0.0
    {
        return Err(Error::new(
            Errc::InvalidArgument,
            "Picture source/destination must use finite integral FrameEncoder coordinates",
        ));
    }
    Ok(FrameRect::new(
        rect.x as i32,
        rect.y as i32,
        rect.w as i32,
        rect.h as i32,
    ))
}

/// 把 `FrameEncoderError` 统一映射为 graphics `Error`（InvalidState）。
pub(super) fn frame_encoder_error(error: FrameEncoderError) -> Error {
    Error::new(
        Errc::InvalidState,
        format!("could not record FrameEncoder command: {error}"),
    )
}
