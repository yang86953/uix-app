//! RenderTarget — 像素缓冲持有者（v2 精简版）。
//!
//! v2 重构后，所有绘制逻辑已迁移到 CpuCanvas2D。
//! RenderTarget 只保留像素缓冲管理和离屏渲染支持。

use super::*;
use uix_core::Rect;
use crate::{Color, Transform};

/// 最大像素缓冲尺寸。
const MAX_PIXEL_DIM: i32 = 16384;

/// 像素缓冲容器。
pub struct RenderTarget {
    pub(crate) pixels: Vec<u32>,
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) clip_rect: Rect,
    pub(crate) clip_int: (i32, i32, i32, i32),
    pub(crate) clip_stack: Vec<Rect>,
    pub(crate) opacity: f32,
    pub(crate) transform: Transform,
    pub(crate) invert: Option<[f64; 6]>,
    pub(crate) blend_mode: BlendMode,
    pub(crate) state_stack: Vec<RenderState>,
    pub(crate) supersample_level: u8,
}

impl RenderTarget {
    pub fn new() -> Self {
        Self {
            pixels: Vec::new(),
            width: 0,
            height: 0,
            clip_rect: Rect::new(0.0, 0.0, f32::MAX, f32::MAX),
            clip_int: (i32::MIN, i32::MIN, i32::MAX, i32::MAX),
            clip_stack: Vec::new(),
            opacity: 1.0,
            transform: Transform::identity(),
            invert: None,
            blend_mode: BlendMode::Alpha,
            state_stack: Vec::new(),
            supersample_level: 0,
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 缓冲访问
// ════════════════════════════════════════════════════════════════════════════

impl RenderTarget {
    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }

    pub fn pixel_buffer(&self) -> &[u32] {
        &self.pixels
    }

    pub fn pixel_buffer_mut(&mut self) -> &mut [u32] {
        &mut self.pixels
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn clip_rect(&self) -> Rect {
        self.clip_rect
    }

    pub fn clip_rect_mut(&mut self) -> &mut Rect {
        &mut self.clip_rect
    }

    pub fn sync_clip_int(&mut self) {
        self.clip_int = (
            (self.clip_rect.x + 0.5).floor() as i32,
            (self.clip_rect.y + 0.5).floor() as i32,
            (self.clip_rect.x + self.clip_rect.w + 0.5).floor() as i32,
            (self.clip_rect.y + self.clip_rect.h + 0.5).floor() as i32,
        );
    }

    pub fn clip_stack_mut(&mut self) -> &mut Vec<Rect> {
        &mut self.clip_stack
    }

    /// 返回状态栈的字节容量。
    pub fn state_stack_bytes(&self) -> usize {
        self.state_stack.capacity() * std::mem::size_of::<RenderState>()
    }

    /// 返回裁剪栈的字节容量。
    pub fn clip_stack_bytes(&self) -> usize {
        self.clip_stack.capacity() * std::mem::size_of::<Rect>()
    }

    /// 返回像素缓冲总字节数。
    pub fn pixel_buffer_bytes(&self) -> usize {
        self.pixels.capacity() * 4
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 初始化 + 像素交换
// ════════════════════════════════════════════════════════════════════════════

impl RenderTarget {
    pub fn initialize(&mut self, width: i32, height: i32) {
        let w = width.clamp(1, MAX_PIXEL_DIM);
        let h = height.clamp(1, MAX_PIXEL_DIM);
        self.width = w;
        self.height = h;
        self.pixels = vec![0x00000000; (w * h) as usize];
        self.clip_rect = Rect::new(0.0, 0.0, w as f32, h as f32);
        self.sync_clip_int();
        self.clip_stack.clear();
        self.state_stack.clear();
        self.invert = Self::compute_inverse(&self.transform);
    }

    pub fn take_pixels(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.pixels)
    }

    pub fn set_pixels(&mut self, pixels: Vec<u32>, w: i32, h: i32) {
        self.width = w;
        self.height = h;
        self.pixels = pixels;
    }

    pub fn swap_pixels(&mut self, pixels: &mut Vec<u32>, w: &mut i32, h: &mut i32) {
        std::mem::swap(&mut self.pixels, pixels);
        std::mem::swap(&mut self.width, w);
        std::mem::swap(&mut self.height, h);
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 变换工具（离屏渲染仍需要）
// ════════════════════════════════════════════════════════════════════════════

impl RenderTarget {
    pub fn is_identity(t: &Transform) -> bool {
        t.m[0] == 1.0 && t.m[1] == 0.0 && t.m[2] == 0.0
            && t.m[3] == 0.0 && t.m[4] == 1.0 && t.m[5] == 0.0
    }

    fn compute_inverse(t: &Transform) -> Option<[f64; 6]> {
        let [a, b, tx, c, d, ty] = t.m.map(|v| v as f64);
        let det = a * d - b * c;
        if det.abs() < 1e-12 {
            return None;
        }
        let inv = 1.0 / det;
        Some([
            inv * d,
            inv * (-b),
            inv * (b * ty - d * tx),
            inv * (-c),
            inv * a,
            inv * (c * tx - a * ty),
        ])
    }
}
