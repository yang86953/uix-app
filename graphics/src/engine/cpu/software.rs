//! SoftwareEngine — CPU 软件渲染引擎。
//!
//! 组合 PixelSurface + CpuCanvas2D + NoopCanvas3D，
//! 实现新 GraphicsEngine trait。
//! 帧生命周期逻辑（清除策略/裁剪管理）在此直接内联，不再依赖 frame.rs。

use uix_platform::Rect;
use uix_platform::Error;

use crate::color::Color;
use crate::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::engine::cpu::noop_canvas_3d::NoopCanvas3D;
use crate::engine::cpu::pixel_surface::PixelSurface;
use crate::engine::RenderOutcome;
use crate::rasterizer::image::blit_image;
use crate::traits::{Canvas2D, Canvas3D, GraphicsEngine, UpdateStrategy};
use crate::ImageHandle;

/// CPU 软件渲染引擎。
///
/// 组合 PixelSurface + CpuCanvas2D + NoopCanvas3D。
/// 不持有资源管理器——字体/图片/字形归 UI 层。
pub struct SoftwareEngine {
    pub(crate) main_width: i32,
    pub(crate) main_height: i32,

    /// 脏区域清除时使用的背景色（默认透明黑，上层可设为主题背景色）。
    pub clear_color: Color,

    /// 2D 绘制上下文（v2 架构）。
    pub(crate) canvas_2d: CpuCanvas2D,
    /// 3D 绘制上下文（Noop）。
    pub(crate) canvas_3d: NoopCanvas3D,

    /// 离屏渲染表面列表（索引 = ImageHandle.0）。
    offscreens: Vec<Option<CpuCanvas2D>>,
    /// 下一个离屏句柄 ID。
    next_offscreen_id: u32,
}

impl Default for SoftwareEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SoftwareEngine {
    /// 创建新的软件渲染引擎实例。
    pub fn new() -> Self {
        let w = 1;
        let h = 1;
        Self {
            main_width: 0,
            main_height: 0,
            clear_color: Color::from_rgba(0, 0, 0, 0),
            canvas_2d: CpuCanvas2D::new(PixelSurface::new(w, h)),
            canvas_3d: NoopCanvas3D,
            offscreens: Vec::new(),
            next_offscreen_id: 0,
        }
    }
}

impl GraphicsEngine for SoftwareEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.main_width = width;
        self.main_height = height;
        self.canvas_2d = CpuCanvas2D::new(PixelSurface::new(width, height));
        Ok(())
    }

    fn shutdown(&mut self) {
        self.main_width = 0;
        self.main_height = 0;
        self.canvas_2d = CpuCanvas2D::new(PixelSurface::new(1, 1));
    }

    fn resize(&mut self, width: i32, height: i32) {
        self.main_width = width;
        self.main_height = height;
        self.canvas_2d = CpuCanvas2D::new(PixelSurface::new(width, height));
    }

    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        let w = self.main_width;
        let h = self.main_height;

        // 无变化时返回 Idle 跳过渲染
        if !strategy.should_clear() {
            match &strategy {
                UpdateStrategy::DirtyRects(rects) if rects.is_empty() => {
                    return RenderOutcome::Idle;
                }
                _ => {}
            }
        }

        let fw = w as f32;
        let fh = h as f32;
        let full = Rect::new(0.0, 0.0, fw, fh);

        // ── 第一步：设置裁剪区域 ──
        match &strategy {
            UpdateStrategy::FullRedraw => {
                self.canvas_2d.push_clip(full);
            }
            UpdateStrategy::DirtyRects(rects) | UpdateStrategy::Overlay(rects) => {
                if rects.is_empty() {
                    self.canvas_2d.push_clip(full);
                } else {
                    let mut bounds = rects[0];
                    for r in &rects[1..] {
                        bounds = bounds.union(r);
                    }
                    let clip = Rect::new(
                        bounds.x.max(0.0),
                        bounds.y.max(0.0),
                        bounds.w.min(fw - bounds.x.max(0.0)),
                        bounds.h.min(fh - bounds.y.max(0.0)),
                    );
                    self.canvas_2d.push_clip(clip);
                }
            }
        }

        // ── 第二步：需要时清除脏区域 ──
        if strategy.should_clear() {
            match &strategy {
                UpdateStrategy::FullRedraw => {
                    self.canvas_2d.surface_mut().clear_all();
                }
                UpdateStrategy::DirtyRects(rects) => {
                    for r in rects {
                        let x0 = (r.x + 0.5).floor().max(0.0) as i32;
                        let y0 = (r.y + 0.5).floor().max(0.0) as i32;
                        let x1 = (r.x + r.w + 0.5).floor().max(0.0) as i32;
                        let y1 = (r.y + r.h + 0.5).floor().max(0.0) as i32;
                        let cw = (x1 - x0).min(w - x0).max(0);
                        let ch = (y1 - y0).min(h - y0).max(0);
                        if cw > 0 && ch > 0 {
                            self.canvas_2d.surface_mut().clear_rect_raw(x0, y0, cw, ch);
                        }
                    }
                }
                UpdateStrategy::Overlay(_) => {}
            }
        }

        RenderOutcome::Present(None)
    }

    fn end_frame(&mut self) -> RenderOutcome {
        // 弹出 begin_frame 时推入的裁剪区域，恢复上一级裁剪状态。
        self.canvas_2d.pop_clip();
        RenderOutcome::Present(None)
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        &mut self.canvas_2d
    }

    fn canvas_3d(&mut self) -> &mut dyn Canvas3D {
        &mut self.canvas_3d
    }

    // ── 离屏缓冲管理 ──

    fn create_offscreen(&mut self, width: i32, height: i32) -> Option<ImageHandle> {
        if width <= 0 || height <= 0 {
            return None;
        }
        let id = self.next_offscreen_id;
        self.next_offscreen_id += 1;

        // 确保索引位置存在
        let idx = id as usize;
        while self.offscreens.len() <= idx {
            self.offscreens.push(None);
        }

        let canvas = CpuCanvas2D::new(PixelSurface::new(width, height));
        self.offscreens[idx] = Some(canvas);
        Some(ImageHandle(id))
    }

    fn destroy_offscreen(&mut self, handle: ImageHandle) {
        let idx = handle.0 as usize;
        if idx < self.offscreens.len() {
            self.offscreens[idx] = None;
        }
    }

    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        let idx = handle.0 as usize;
        if idx < self.offscreens.len() {
            if let Some(ref mut canvas) = self.offscreens[idx] {
                return Some(canvas as &mut dyn Canvas2D);
            }
        }
        None
    }

    fn blit_offscreen(&mut self, handle: &ImageHandle, dst_rect: Rect) {
        let idx = handle.0 as usize;
        if idx >= self.offscreens.len() {
            return;
        }
        if let Some(ref offscreen_canvas) = self.offscreens[idx] {
            let surf = offscreen_canvas.surface();
            let src_pixels = surf.pixels();
            let src_w = surf.width();
            let src_rect = Rect::new(0.0, 0.0, src_w as f32, surf.height() as f32);
            let size = self.canvas_2d.width();
            let h = self.canvas_2d.height();
            let clip = self.canvas_2d.current_clip();
            let opacity = self.canvas_2d.opacity();
            blit_image(
                self.canvas_2d.pixels_mut(), size, h,
                clip, opacity,
                src_pixels, src_w, src_rect, dst_rect,
            );
        }
    }

    fn memory_usage(&self) -> usize {
        let main = self.canvas_2d.surface();
        let main_bytes = (main.width() * main.height() * 4) as usize;
        let offscreen_bytes: usize = self.offscreens.iter().filter_map(|o| {
            o.as_ref().map(|c| {
                let s = c.surface();
                (s.width() * s.height() * 4) as usize
            })
        }).sum();
        main_bytes + offscreen_bytes
    }

    fn diagnose_memory(&self) {
        log::info!("SoftwareEngine memory: {} bytes", self.memory_usage());
    }
}
