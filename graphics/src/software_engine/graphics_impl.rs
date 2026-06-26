use super::engine::{ActiveTarget, SoftwareEngine};
use uix_diag::Error;
use crate::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::engine::cpu::noop_canvas_3d::NoopCanvas3D;
use crate::engine::cpu::pixel_surface::PixelSurface;
use crate::traits::{Canvas2D, Canvas3D, GraphicsEngine, UpdateStrategy};
use crate::engine::RenderOutcome;
use crate::frame::{self, ClearOp};
use crate::DirtyRegion;

impl GraphicsEngine for SoftwareEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.main_width = width;
        self.main_height = height;
        self.rt.initialize(width, height);
        self.canvas_2d = CpuCanvas2D::new(PixelSurface::new(width, height));
        self.active_target = ActiveTarget::Main;
        Ok(())
    }

    fn shutdown(&mut self) {
        self.rt = super::core::RenderTarget::new();
        self.assets.shutdown();
        self.main_width = 0;
        self.main_height = 0;
        self.active_target = ActiveTarget::Main;
    }

    fn resize(&mut self, width: i32, height: i32) {
        self.main_width = width;
        self.main_height = height;
        self.rt.initialize(width, height);
        self.canvas_2d = CpuCanvas2D::new(PixelSurface::new(width, height));
        self.active_target = ActiveTarget::Main;
    }

    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        let dirty = match &strategy {
            UpdateStrategy::FullRedraw => DirtyRegion::full(),
            UpdateStrategy::DirtyRects(rects) => {
                let mut d = DirtyRegion::empty();
                for r in rects {
                    d.add_rect(*r);
                }
                d
            }
            UpdateStrategy::Overlay(_rects) => DirtyRegion::empty(),
        };

        self.pre_frame_clip = frame::frame_begin_clip(
            &dirty,
            self.main_width,
            self.main_height,
            |rect| {
                // 同步 clip 到 canvas_2d
                self.canvas_2d.push_clip(rect);
                self.rt.clip_stack_mut().clear();
                *self.rt.clip_rect_mut() = rect;
                self.rt.sync_clip_int();
                self.pre_frame_clip
            },
        );

        if strategy.should_clear() {
            frame::frame_begin_clear(&dirty, self.clear_color, self.main_width, self.main_height, |op| {
                match op {
                    ClearOp::All(color) => {
                        self.canvas_2d.surface_mut().clear_all();
                    }
                    ClearOp::Rect(x, y, w, h, _color) => {
                        self.canvas_2d.surface_mut().clear_rect_raw(x, y, w, h);
                    }
                }
            });
        }

        RenderOutcome::Present(None)
    }

    fn end_frame(&mut self) -> RenderOutcome {
        frame::frame_end(self.pre_frame_clip, |rect| {
            *self.rt.clip_rect_mut() = rect;
            self.rt.sync_clip_int();
            self.rt.clip_stack_mut().clear();
        });
        RenderOutcome::Present(None)
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        &mut self.canvas_2d
    }

    fn canvas_3d(&mut self) -> &mut dyn Canvas3D {
        &mut self.canvas_3d
    }

    fn memory_usage(&self) -> usize {
        self.rt.pixel_buffer_bytes() + self.assets.offscreen_bytes() + self.assets.image_bytes()
    }

    fn diagnose_memory(&self) {
        log::info!("SoftwareEngine memory: {} bytes", self.memory_usage());
    }
}
