//! 软回退 CPU segment 的最小平铺 — gpu 子模块。

// 保存通用 renderer 内部使用的紧边界 CPU staging 位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SoftFallbackTile {
    // 保存目标左上角的逻辑 x 坐标。
    pub(crate) dst_x: i32,
    // 保存目标左上角的逻辑 y 坐标。
    pub(crate) dst_y: i32,
    // 保存紧密像素载荷的宽度。
    pub(crate) width: i32,
    // 保存紧密像素载荷的高度。
    pub(crate) height: i32,
}

// 为通用 renderer 的 soft segment 提供纯几何辅助。
impl SoftFallbackTile {
    // 由目标边界创建一个紧密 staging 描述。
    pub(crate) const fn at_destination(
        // 接收目标左上角逻辑 x 坐标。
        dst_x: i32,
        // 接收目标左上角逻辑 y 坐标。
        dst_y: i32,
        // 接收紧密载荷宽度。
        width: i32,
        // 接收紧密载荷高度。
        height: i32,
        // 返回只属于 draw backend 的 staging 描述。
    ) -> Self {
        // 保存调用方已经裁出的目标边界。
        Self {
            // 记录目标 x 坐标。
            dst_x,
            // 记录目标 y 坐标。
            dst_y,
            // 记录载荷宽度。
            width,
            // 记录载荷高度。
            height,
        }
    }

    // 计算紧密载荷需要的精确像素数。
    pub(crate) fn required_pixels(self) -> Option<usize> {
        // 非正尺寸不能形成可提交的 soft segment。
        if self.width <= 0 || self.height <= 0 {
            // 让调用方把非法或空边界当作不可打包。
            return None;
        }
        // 使用有符号宽高乘积并检查 usize 转换，避免分配溢出。
        usize::try_from(i64::from(self.width) * i64::from(self.height)).ok()
    }
}

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
