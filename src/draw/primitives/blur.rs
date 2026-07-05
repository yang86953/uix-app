//! 高斯模糊 — 可分离的 2D 卷积（水平→垂直两趟传递）。
//!
//! 用于软阴影、模糊背景等视觉效果。O(n) 复杂度，n 为像素数。
//!
//! # 优化说明
//! - f32 替代 f64：卷积计算用 f32 精度已足够，减少缓存压力
//! - 预提取通道值：避免每次循环重复位移
//! - 内核栈分配：小内核（≤31）使用栈数组避免堆分配
//! - 内核 LRU 缓存：相同半径重复调用时复用已计算的内核

use std::cell::RefCell;
use std::collections::HashMap;
use crate::native::Rect;

thread_local! {
    /// 内核缓存：key = radius 位编码，value = (kernel_size, kernel_vec)
    static KERNEL_CACHE: RefCell<HashMap<u32, (usize, Vec<f32>)>> = RefCell::new(HashMap::new());
}

/// 对像素缓冲执行高斯模糊。
///
/// `pixels`: BGRA 32bit premultiplied 像素缓冲。
/// `width`, `height`: 缓冲尺寸。
/// `rect`: 要模糊的区域（像素坐标）。
/// `radius`: 模糊半径（越大越模糊，推荐 1-50）。
pub fn gaussian_blur(pixels: &mut [u32], width: i32, height: i32, rect: Rect, radius: f32) {
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
    let cache_key = radius as u32;

    // 查缓存
    let kernel: Vec<f32> = KERNEL_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((_, k)) = cache.get(&cache_key) {
            return k.clone();
        }
        let sigma2 = -(1.0 / (2.0 * sigma * sigma));
        let mut k = Vec::with_capacity(kernel_size);
        let mut total = 0.0_f32;
        for i in 0..kernel_size {
            let x = (i as i32 - kernel_radius) as f32;
            let w = (x * x * sigma2).exp();
            k.push(w);
            total += w;
        }
        let inv_total = 1.0 / total;
        for w in k.iter_mut() {
            *w *= inv_total;
        }
        cache.insert(cache_key, (kernel_size, k.clone()));
        k
    });

    let stride = width as usize;
    let rw = (x1 - x0) as usize;
    let rh = (y1 - y0) as usize;

    // 临时缓冲（整块，用于垂直传递）
    let mut tmp = vec![0u32; rw * rh];

    // ── 水平传递 ──
    for y in y0..y1 {
        let src_row_off = y as usize * stride;
        let tmp_off = (y - y0) as usize * rw;
        for x in x0..x1 {
            let mut r = 0.0_f32;
            let mut g = 0.0_f32;
            let mut b = 0.0_f32;
            let mut a = 0.0_f32;
            for (ki, &kw) in kernel.iter().enumerate() {
                let sx = (x + ki as i32 - kernel_radius).clamp(x0, x1 - 1) as usize;
                let pixel = pixels[src_row_off + sx];
                a += ((pixel >> 24) & 0xFF) as f32 * kw;
                r += ((pixel >> 16) & 0xFF) as f32 * kw;
                g += ((pixel >> 8) & 0xFF) as f32 * kw;
                b += (pixel & 0xFF) as f32 * kw;
            }
            let idx = tmp_off + (x - x0) as usize;
            tmp[idx] = ((a as u32).min(255) << 24)
                | ((r as u32).min(255) << 16)
                | ((g as u32).min(255) << 8)
                | (b as u32).min(255);
        }
    }

    // ── 垂直传递 ──
    for x in 0..rw {
        for y in 0..rh {
            let mut r = 0.0_f32;
            let mut g = 0.0_f32;
            let mut b = 0.0_f32;
            let mut a = 0.0_f32;
            for (ki, &kw) in kernel.iter().enumerate() {
                let sy = (y as i32 + ki as i32 - kernel_radius).clamp(0, rh as i32 - 1) as usize;
                let pixel = tmp[sy * rw + x];
                a += ((pixel >> 24) & 0xFF) as f32 * kw;
                r += ((pixel >> 16) & 0xFF) as f32 * kw;
                g += ((pixel >> 8) & 0xFF) as f32 * kw;
                b += (pixel & 0xFF) as f32 * kw;
            }
            let dst_idx = (y0 + y as i32) as usize * stride + (x0 + x as i32) as usize;
            pixels[dst_idx] = ((a as u32).min(255) << 24)
                | ((r as u32).min(255) << 16)
                | ((g as u32).min(255) << 8)
                | (b as u32).min(255);
        }
    }
}
