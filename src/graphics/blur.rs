//! 高斯模糊 — 可分离的 2D 卷积（水平→垂直两趟传递）。
//!
//! 用于软阴影、模糊背景等视觉效果。O(n) 复杂度，n 为像素数。

use crate::base::Rect;

/// 对像素缓冲执行高斯模糊。
///
/// `pixels`: BGRA 32bit premultiplied 像素缓冲。
/// `width`, `height`: 缓冲尺寸。
/// `rect`: 要模糊的区域（像素坐标）。
/// `radius`: 模糊半径（越大越模糊，推荐 1-50）。
pub fn gaussian_blur(
    pixels: &mut [u32],
    width: i32, height: i32,
    rect: Rect,
    radius: f32,
) {
    if radius < 0.5 || width <= 0 || height <= 0 {
        return;
    }

    // 计算裁剪区域
    let x0 = rect.x.max(0.0) as i32;
    let y0 = rect.y.max(0.0) as i32;
    let x1 = (rect.x + rect.w).min(width as f32) as i32;
    let y1 = (rect.y + rect.h).min(height as f32) as i32;
    if x0 >= x1 || y0 >= y1 {
        return;
    }

    let sigma = radius / 3.0;
    let kernel_radius = radius.ceil() as i32;
    let kernel_size = (kernel_radius * 2 + 1) as usize;
    let mut kernel = Vec::with_capacity(kernel_size);
    let mut total = 0.0_f64;
    for i in -kernel_radius..=kernel_radius {
        let x = i as f64;
        let weight = (-(x * x) / (2.0 * sigma as f64 * sigma as f64)).exp();
        kernel.push(weight);
        total += weight;
    }
    // 归一化
    for w in &mut kernel {
        *w /= total;
    }

    let stride = width as usize;
    let rw = (x1 - x0) as usize;
    let rh = (y1 - y0) as usize;

    // 临时缓冲（一行）
    let mut tmp_row = vec![0u32; rw];
    // 临时缓冲（整块，用于垂直传递）
    let mut tmp = vec![0u32; rw * rh];

    // ── 水平传递 ──
    for y in y0..y1 {
        let src_row_off = y as usize * stride;
        for x in x0..x1 {
            let mut r = 0.0_f64;
            let mut g = 0.0_f64;
            let mut b = 0.0_f64;
            let mut a = 0.0_f64;
            for (ki, &kw) in kernel.iter().enumerate() {
                let sx = (x + ki as i32 - kernel_radius).clamp(x0, x1 - 1);
                let pixel = pixels[src_row_off + sx as usize];
                a += ((pixel >> 24) & 0xFF) as f64 * kw;
                r += ((pixel >> 16) & 0xFF) as f64 * kw;
                g += ((pixel >> 8) & 0xFF) as f64 * kw;
                b += (pixel & 0xFF) as f64 * kw;
            }
            let idx = (x - x0) as usize;
            tmp_row[idx] = ((a as u32).min(255) << 24)
                | ((r as u32).min(255) << 16)
                | ((g as u32).min(255) << 8)
                | (b as u32).min(255);
        }
        // 复制到临时缓冲
        let tmp_off = (y - y0) as usize * rw;
        tmp[tmp_off..tmp_off + rw].copy_from_slice(&tmp_row);
    }

    // ── 垂直传递 ──
    for x in 0..rw {
        for y in 0..rh {
            let mut r = 0.0_f64;
            let mut g = 0.0_f64;
            let mut b = 0.0_f64;
            let mut a = 0.0_f64;
            for (ki, &kw) in kernel.iter().enumerate() {
                let sy = (y as i32 + ki as i32 - kernel_radius).clamp(0, rh as i32 - 1) as usize;
                let pixel = tmp[sy * rw + x];
                a += ((pixel >> 24) & 0xFF) as f64 * kw;
                r += ((pixel >> 16) & 0xFF) as f64 * kw;
                g += ((pixel >> 8) & 0xFF) as f64 * kw;
                b += (pixel & 0xFF) as f64 * kw;
            }
            let dst_idx = (y0 + y as i32) as usize * stride + (x0 + x as i32) as usize;
            pixels[dst_idx] = ((a as u32).min(255) << 24)
                | ((r as u32).min(255) << 16)
                | ((g as u32).min(255) << 8)
                | (b as u32).min(255);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blur_noop() {
        let mut pixels = vec![0xFFFFFFFFu32; 100];
        gaussian_blur(&mut pixels, 10, 10, Rect::new(0.0, 0.0, 10.0, 10.0), 0.0);
        assert_eq!(pixels.iter().all(|&p| p == 0xFFFFFFFF), true);
    }

    #[test]
    fn test_blur_small() {
        let mut pixels = vec![0u32; 400];
        // 中心一个白点
        pixels[11 * 20 + 10] = 0xFFFFFFFF;
        gaussian_blur(&mut pixels, 20, 20, Rect::new(0.0, 0.0, 20.0, 20.0), 3.0);
        // 模糊后应该有多于 1 个非零像素
        let non_zero = pixels.iter().filter(|&&p| p != 0).count();
        assert!(non_zero > 1);
    }
}
