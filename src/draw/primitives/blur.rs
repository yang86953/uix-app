//! 高斯模糊 — 可分离的 2D 卷积（水平→垂直两趟传递）。
//!
//! 用于软阴影、模糊背景等视觉效果。O(n) 复杂度，n 为像素数。
//!
//! # 优化说明
//! - f32 替代 f64：卷积计算用 f32 精度已足够，减少缓存压力
//! - 预提取通道值：避免每次循环重复位移
//! - 内核栈分配：小内核（≤31）使用栈数组避免堆分配
//! - 内核 LRU 缓存：相同半径重复调用时复用已计算的内核

use crate::core::Rect;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Arc;

const MAX_BLUR_RADIUS: f32 = 256.0;
const MAX_CACHED_KERNELS: usize = 32;

thread_local! {
    /// 小型 LRU：key 使用规范化半径的完整位编码，避免小数半径错误复用。
    #[allow(
        clippy::missing_const_for_thread_local,
        reason = "the initializer already uses an inline const block; Clippy reports the macro expansion"
    )]
    static KERNEL_CACHE: RefCell<VecDeque<(u32, Arc<[f32]>)>> = const { RefCell::new(VecDeque::new()) };
}

fn gaussian_kernel(radius: f32) -> Arc<[f32]> {
    let cache_key = radius.to_bits();
    KERNEL_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(index) = cache.iter().position(|(key, _)| *key == cache_key) {
            if let Some((_, kernel)) = cache.remove(index) {
                cache.push_back((cache_key, Arc::clone(&kernel)));
                return kernel;
            }
        }

        let sigma = radius / 3.0;
        let kernel_radius = radius.ceil() as usize;
        let kernel_size = kernel_radius * 2 + 1;
        let sigma2 = -(1.0 / (2.0 * sigma * sigma));
        let mut kernel = Vec::with_capacity(kernel_size);
        let mut total = 0.0_f32;
        for index in 0..kernel_size {
            let x = (index as i64 - kernel_radius as i64) as f32;
            let weight = (x * x * sigma2).exp();
            kernel.push(weight);
            total += weight;
        }
        let inv_total = 1.0 / total;
        for weight in &mut kernel {
            *weight *= inv_total;
        }

        let kernel = Arc::<[f32]>::from(kernel);
        if cache.len() == MAX_CACHED_KERNELS {
            cache.pop_front();
        }
        cache.push_back((cache_key, Arc::clone(&kernel)));
        kernel
    })
}

#[cfg(test)]
pub(crate) fn kernel_cache_len_for_test() -> usize {
    KERNEL_CACHE.with(|cache| cache.borrow().len())
}

/// 对像素缓冲执行高斯模糊。
///
/// `pixels`: BGRA 32bit premultiplied 像素缓冲。
/// `width`, `height`: 缓冲尺寸。
/// `rect`: 要模糊的区域（像素坐标）。
/// `radius`: 模糊半径（越大越模糊，推荐 1-50；为保证有界执行，上限为 256）。
pub fn gaussian_blur(pixels: &mut [u32], width: i32, height: i32, rect: Rect, radius: f32) {
    if !radius.is_finite()
        || radius < 0.5
        || width <= 0
        || height <= 0
        || !rect.x.is_finite()
        || !rect.y.is_finite()
        || !rect.w.is_finite()
        || !rect.h.is_finite()
        || rect.w <= 0.0
        || rect.h <= 0.0
    {
        return;
    }

    let width = width as usize;
    let height = height as usize;
    let Some(pixel_count) = width.checked_mul(height) else {
        return;
    };
    if pixels.len() < pixel_count {
        return;
    }

    // 计算裁剪区域
    let x0 = (rect.x as f64).clamp(0.0, width as f64) as usize;
    let y0 = (rect.y as f64).clamp(0.0, height as f64) as usize;
    let x1 = (rect.x as f64 + rect.w as f64).clamp(0.0, width as f64) as usize;
    let y1 = (rect.y as f64 + rect.h as f64).clamp(0.0, height as f64) as usize;
    if x0 >= x1 || y0 >= y1 {
        return;
    }

    let radius = radius.min(MAX_BLUR_RADIUS);
    let kernel_radius = radius.ceil() as i64;
    let kernel = gaussian_kernel(radius);

    let stride = width;
    let rw = x1 - x0;
    let rh = y1 - y0;

    // 临时缓冲（整块，用于垂直传递）
    let Some(tmp_len) = rw.checked_mul(rh) else {
        return;
    };
    let mut tmp = Vec::new();
    if tmp.try_reserve_exact(tmp_len).is_err() {
        return;
    }
    tmp.resize(tmp_len, 0u32);

    // ── 水平传递 ──
    for y in y0..y1 {
        let src_row_off = y * stride;
        let tmp_off = (y - y0) * rw;
        for x in x0..x1 {
            let mut r = 0.0_f32;
            let mut g = 0.0_f32;
            let mut b = 0.0_f32;
            let mut a = 0.0_f32;
            for (ki, &kw) in kernel.iter().enumerate() {
                let sx =
                    (x as i64 + ki as i64 - kernel_radius).clamp(x0 as i64, x1 as i64 - 1) as usize;
                let pixel = pixels[src_row_off + sx];
                a += ((pixel >> 24) & 0xFF) as f32 * kw;
                r += ((pixel >> 16) & 0xFF) as f32 * kw;
                g += ((pixel >> 8) & 0xFF) as f32 * kw;
                b += (pixel & 0xFF) as f32 * kw;
            }
            let idx = tmp_off + (x - x0);
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
                let sy = (y as i64 + ki as i64 - kernel_radius).clamp(0, rh as i64 - 1) as usize;
                let pixel = tmp[sy * rw + x];
                a += ((pixel >> 24) & 0xFF) as f32 * kw;
                r += ((pixel >> 16) & 0xFF) as f32 * kw;
                g += ((pixel >> 8) & 0xFF) as f32 * kw;
                b += (pixel & 0xFF) as f32 * kw;
            }
            let dst_idx = (y0 + y) * stride + x0 + x;
            pixels[dst_idx] = ((a as u32).min(255) << 24)
                | ((r as u32).min(255) << 16)
                | ((g as u32).min(255) << 8)
                | (b as u32).min(255);
        }
    }
}
