//! SoftwareEngine — CPU 软件渲染引擎。
//!
//! 组合 PixelSurface + CpuCanvas2D + NoopCanvas3D，
//! 实现新 GraphicsEngine trait。
//! 帧生命周期逻辑（清除策略/裁剪管理）在此直接内联，不再依赖 frame.rs。

use uix_core::Rect;
use uix_diag::Error;

use crate::color::Color;
use crate::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::engine::cpu::noop_canvas_3d::NoopCanvas3D;
use crate::engine::cpu::pixel_surface::PixelSurface;
use crate::engine::RenderOutcome;
use crate::traits::{Canvas2D, Canvas3D, GraphicsEngine, UpdateStrategy};

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
                    // 计算所有脏矩形的外接矩形作为裁剪区域
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
                // Overlay 策略：不清除，直接覆盖绘制
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

    fn memory_usage(&self) -> usize {
        let surf = self.canvas_2d.surface();
        (surf.width() * surf.height() * 4) as usize
    }

    fn diagnose_memory(&self) {
        log::info!("SoftwareEngine memory: {} bytes", self.memory_usage());
    }
}
