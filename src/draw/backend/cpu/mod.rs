//! CPU 渲染后端 — PixelSurface + CpuCanvas2D。

pub(crate) mod canvas_2d;
// 测试支撑使用 API 无关空实现，避免 Drawing 因目标系统产生不同源码分支。
#[cfg(any(test, feature = "test-harness", feature = "graphics-parity-test"))]
pub(crate) mod noop_canvas_2d;
pub(crate) mod offscreen;
// 将 Picture blur 与后续合成的 CPU 回归拆到独立测试文件。
#[cfg(test)]
// 保持生产模块主体低于单文件行数边界。
#[path = "../../../../tests/unit/draw/backend/cpu/blur_tests.rs"]
// 仅在测试构建中编译离屏模糊契约。
mod blur_tests;

use crate::core::{Error, Point, Rect};

use crate::core::DamageRegion;
use crate::draw::backend::contract::{
    BackendCapabilities, BackendKind, DrawSurface, RenderBackend,
};
use crate::draw::backend::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::backend::cpu::offscreen::CpuOffscreenPool;
use crate::draw::geometry::color::Color;
// Picture 合成需要显式建立不重复应用场景 transform 的 canonical 状态。
use crate::draw::Canvas2D;
use crate::draw::geometry::types::{ImageHandle, Transform};
use crate::draw::painting::{EncodedFrameExecution, EncodedPictureExecution, FrameEncoder};
use crate::draw::raster::pixel_surface::PixelSurface;

/// CPU 主缓冲 DrawSurface 适配器。
pub(crate) struct CpuDrawSurface {
    canvas: CpuCanvas2D,
}

impl CpuDrawSurface {
    /// 创建 CPU 绘制表面。
    ///
    /// # Panics
    ///
    /// 尺寸无法分配时 panic；运行时尺寸应使用 [`Self::try_new`]。
    pub(crate) fn new(width: i32, height: i32) -> Self {
        Self {
            canvas: CpuCanvas2D::new(PixelSurface::new(width, height)),
        }
    }

    pub(crate) fn try_new(width: i32, height: i32) -> Result<Self, Error> {
        Ok(Self {
            canvas: CpuCanvas2D::new(PixelSurface::try_new(width, height)?),
        })
    }

    pub(crate) fn canvas_mut(&mut self) -> &mut CpuCanvas2D {
        &mut self.canvas
    }

    pub(crate) fn surface(&self) -> &PixelSurface {
        self.canvas.surface()
    }

    pub(crate) fn set_clear_color(&mut self, color: Color) {
        self.canvas.surface_mut().set_clear_color(color);
    }
}

impl DrawSurface for CpuDrawSurface {
    fn size(&self) -> crate::core::Size {
        self.canvas.surface_size()
    }

    fn push_clip(&mut self, rect: Rect) {
        self.canvas.push_clip(rect);
    }

    fn pop_clip(&mut self) {
        self.canvas.pop_clip();
    }

    fn clear_all(&mut self) {
        self.canvas.surface_mut().clear_all();
    }

    fn clear_rect_raw(&mut self, x: i32, y: i32, w: i32, h: i32) {
        self.canvas.surface_mut().clear_rect_raw(x, y, w, h);
    }

    fn copy_region(&mut self, src: Rect, dst: Point) {
        self.canvas
            .surface_mut()
            .copy_region(src, dst.x as i32, dst.y as i32);
    }

    fn canvas(&mut self) -> &mut dyn Canvas2D {
        &mut self.canvas
    }

    fn take_deferred_error(&mut self) -> Option<Error> {
        self.canvas.take_deferred_error()
    }
}

/// CPU 软件渲染后端。
pub struct CpuBackend {
    width: i32,
    height: i32,
    clear_color: Color,
    main: CpuDrawSurface,
    pub(crate) offscreens: CpuOffscreenPool,
    active_offscreen: Option<u32>,
}

impl CpuBackend {
    /// 创建尺寸为零、透明清除色且没有离屏资源的软件后端。
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            clear_color: Color::from_rgba(0, 0, 0, 0),
            main: CpuDrawSurface::new(1, 1),
            offscreens: CpuOffscreenPool::new(),
            active_offscreen: None,
        }
    }

    /// 更新记录的清除色并应用到当前主表面。
    pub fn set_clear_color(&mut self, color: Color) {
        self.clear_color = color;
        self.main.set_clear_color(color);
    }

    /// 返回当前记录的主表面清除色。
    pub fn clear_color(&self) -> Color {
        self.clear_color
    }

    /// 返回当前主表面的只读像素缓冲。
    pub fn pixels(&self) -> &[u32] {
        self.main.surface().pixels()
    }

    /// 返回当前主表面宽度。
    pub fn width(&self) -> i32 {
        self.width
    }

    /// 返回当前主表面高度。
    pub fn height(&self) -> i32 {
        self.height
    }

    fn blit_offscreen_impl(&mut self, handle: &ImageHandle, src_rect: Rect, dst_rect: Rect) {
        let Some(offscreen_canvas) = self.offscreens.get(handle) else {
            return;
        };
        let surf = offscreen_canvas.surface();
        let src_pixels = surf.pixels();
        let src_w = surf.width();
        // Picture bounds 已经位于目标 surface 空间，不能再次应用场景 transform 或 offset。
        let main = self.main.canvas_mut();
        // 保存调用方的 clip、opacity、blend 与坐标状态，合成结束后完整恢复。
        main.save();
        // 主表面 Picture 合成使用 canonical identity，保持旧路径的 surface-space 几何。
        main.set_transform(Transform::identity());
        // 清除调用方 offset，避免缓存 bounds 被平移两次。
        main.set_offset(0.0, 0.0);
        // 通过状态完整的软件采样入口保留当前 SrcOver/Additive、opacity 与 clip。
        main.blit_image(src_pixels, src_w, src_rect, dst_rect);
        // 恢复调用方状态，使后续 painter-order 操作不受 Picture 合成影响。
        main.restore();
    }
}

impl Default for CpuBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderBackend for CpuBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Cpu
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::cpu()
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        let main = CpuDrawSurface::try_new(width, height)?;
        self.width = main.surface().width();
        self.height = main.surface().height();
        self.main = main;
        Ok(())
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.width = 0;
        self.height = 0;
        self.main = CpuDrawSurface::new(1, 1);
        self.active_offscreen = None;
        self.offscreens.clear();
        Ok(())
    }

    fn surface(&mut self) -> &mut dyn DrawSurface {
        &mut self.main
    }

    // 让 CPU 像素分配错误保持 typed OOM，而不是退化为 None。
    fn try_create_offscreen(
        // 借用 CPU backend 的唯一可变 owner。
        &mut self,
        // 接收 Picture 的逻辑宽度。
        width: i32,
        // 接收 Picture 的逻辑高度。
        height: i32,
        // 返回正常无资源、成功 handle 或 typed 分配失败。
    ) -> Result<Option<ImageHandle>, Error> {
        // 复用 CPU 离屏池的检查式创建边界。
        self.offscreens.try_create(width, height)
    }

    // CPU 释放不会调用原生 API，但仍满足统一检查式契约。
    fn try_destroy_offscreen(&mut self, handle: ImageHandle) -> Result<(), Error> {
        if self.active_offscreen == Some(handle.0) {
            self.active_offscreen = None;
        }
        self.offscreens.destroy(handle);
        self.offscreens.compact();
        // 槽位回收完成后再报告释放成功。
        Ok(())
    }

    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        self.offscreens.canvas_mut(handle)
    }

    fn copy_offscreen_pixels(&self, handle: &ImageHandle) -> Option<(Vec<u32>, i32)> {
        self.offscreens.copy_pixels(handle)
    }

    fn try_execute_encoded_picture(
        &mut self,
        handle: &ImageHandle,
        encoder: &FrameEncoder,
    ) -> Result<EncodedPictureExecution, Error> {
        let target = self.offscreens.get_mut(handle).ok_or_else(|| {
            Error::new(
                crate::core::Errc::InvalidState,
                "Picture offscreen target disappeared before FrameEncoder execution",
            )
        })?;
        let surface = target.surface_mut();
        if surface.width() != encoder.width() || surface.height() != encoder.height() {
            return Err(Error::new(
                crate::core::Errc::InvalidState,
                format!(
                    "FrameEncoder {}x{} does not match Picture target {}x{}",
                    encoder.width(),
                    encoder.height(),
                    surface.width(),
                    surface.height()
                ),
            ));
        }
        encoder.execute_into_pixels(surface.pixels_mut());
        Ok(EncodedPictureExecution::Executed)
    }

    fn try_execute_encoded_frame(
        &mut self,
        encoder: &FrameEncoder,
    ) -> Result<EncodedFrameExecution, Error> {
        if (self.width, self.height) != (encoder.width(), encoder.height()) {
            return Err(Error::new(
                crate::core::Errc::InvalidState,
                format!(
                    "FrameEncoder {}x{} does not match main CPU target {}x{}",
                    encoder.width(),
                    encoder.height(),
                    self.width,
                    self.height
                ),
            ));
        }
        encoder.execute_into_pixels(self.main.canvas_mut().surface_mut().pixels_mut());
        Ok(EncodedFrameExecution::Executed)
    }

    fn try_begin_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        let Some(canvas) = self.offscreens.get_mut(handle) else {
            return Err(Error::new(
                crate::core::Errc::InvalidState,
                "Picture offscreen target does not exist",
            ));
        };
        // Picture rasterization is replace semantics even for transparent
        // pixels. Filling with transparent through Canvas2D would be an
        // alpha-over no-op and could retain stale cached content.
        let width = canvas.surface().width();
        let height = canvas.surface().height();
        canvas.surface_mut().clear_all();
        canvas.reset_state_for_extent(width, height);
        self.active_offscreen = Some(handle.0);
        Ok(())
    }

    fn try_flush_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        let canvas = self.offscreens.get_mut(handle).ok_or_else(|| {
            Error::new(
                crate::core::Errc::InvalidState,
                "Picture offscreen target disappeared before flush",
            )
        })?;
        if let Some(error) = canvas.take_deferred_error() {
            return Err(error);
        }
        Ok(())
    }

    fn try_end_offscreen_paint(&mut self) -> Result<(), Error> {
        let active = self.active_offscreen.take();
        if let Some(id) = active {
            let handle = ImageHandle(id);
            if let Some(canvas) = self.offscreens.get_mut(&handle) {
                if let Some(error) = canvas.take_deferred_error() {
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    fn try_blit_offscreen_src(
        &mut self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
    ) -> Result<(), Error> {
        if self.offscreens.get(handle).is_none() {
            return Err(Error::new(
                crate::core::Errc::InvalidState,
                "Picture offscreen target does not exist before blit",
            ));
        }
        if let Some(dst_id) = self.active_offscreen {
            if dst_id == handle.0 {
                return Err(Error::new(
                    crate::core::Errc::InvalidArgument,
                    "Picture offscreen target cannot blit into itself",
                ));
            }
            let dst_handle = ImageHandle(dst_id);
            if !self
                .offscreens
                .blit_into(handle, &dst_handle, src_rect, dst_rect)
            {
                return Err(Error::new(
                    crate::core::Errc::InvalidState,
                    "Picture offscreen source or target disappeared before nested blit",
                ));
            }
            return Ok(());
        }
        self.blit_offscreen_impl(handle, src_rect, dst_rect);
        Ok(())
    }

    fn try_blur_offscreen(
        &mut self,
        handle: &ImageHandle,
        region: Rect,
        radius: f32,
    ) -> Result<(), Error> {
        if !radius.is_finite() || radius < 0.5 {
            return Ok(());
        }
        let canvas = self.offscreens.get_mut(handle).ok_or_else(|| {
            Error::new(
                crate::core::Errc::InvalidState,
                "Picture offscreen target does not exist before blur",
            )
        })?;
        let width = canvas.surface().width();
        let height = canvas.surface().height();
        let pixels = canvas.surface_mut().pixels_mut();
        crate::draw::raster::rasterizer::blur::gaussian_blur(pixels, width, height, region, radius);
        Ok(())
    }

    fn present(&mut self, _damage: &DamageRegion) -> Result<(), Error> {
        if let Some(error) = self.main.take_deferred_error() {
            return Err(error);
        }
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl CpuBackend {
    /// 将离屏缓冲内容 blit 到任意 Canvas2D（支持嵌套 Picture 合成）。
    pub fn blit_offscreen_to_canvas(
        &self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
        canvas: &mut dyn crate::draw::Canvas2D,
    ) {
        let Some(offscreen_canvas) = self.offscreens.get(handle) else {
            return;
        };
        let surf = offscreen_canvas.surface();
        canvas.blit_image(surf.pixels(), surf.width(), src_rect, dst_rect);
    }

    /// 复制离屏像素（避免与 offscreen_canvas 可变借用冲突）。
    pub fn copy_offscreen_pixels(&self, handle: &ImageHandle) -> Option<(Vec<u32>, i32)> {
        self.offscreens.copy_pixels(handle)
    }

    /// 返回主表面与全部离屏表面占用的像素内存字节数。
    pub fn memory_usage(&self) -> usize {
        let surf = self.main.surface();
        surf.memory_usage()
            .saturating_add(self.offscreens.memory_usage())
    }
}
