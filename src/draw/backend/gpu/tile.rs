//! 软回退 CPU segment 的最小平铺 — gpu 子模块。

use crate::native::present::SoftFallbackTile;

pub(crate) fn pack_visible_soft_fallback_tile(
    pixels: &[u32],
    width: i32,
    height: i32,
) -> Option<(Vec<u32>, SoftFallbackTile)> {
    if width <= 0 || height <= 0 {
        return None;
    }
    let expected = (width as usize).checked_mul(height as usize)?;
    if pixels.len() < expected {
        return None;
    }
    let mut left = width;
    let mut top = height;
    let mut right = 0;
    let mut bottom = 0;
    let mut visible = false;
    for y in 0..height {
        let row = y as usize * width as usize;
        for x in 0..width {
            if pixels[row + x as usize] & 0xff00_0000 == 0 {
                continue;
            }
            visible = true;
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    if !visible {
        return None;
    }
    let tile = SoftFallbackTile::at_destination(left, top, right - left, bottom - top);
    let mut packed = Vec::with_capacity(tile.required_pixels()?);
    for y in top..bottom {
        let row = y as usize * width as usize;
        packed.extend_from_slice(&pixels[row + left as usize..row + right as usize]);
    }
    Some((packed, tile))
}
