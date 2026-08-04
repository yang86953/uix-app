//! [`NativeGpuCanvas2D`] 定义与生命周期管理 — gpu 子模块。
//!
//! 软回退（scratch）状态、裁剪/变换折叠与 GPU-only 模式判定；绘制操作在
//! [`super::queue`] 入队、[`super::submit`] 提交、[`super::canvas2d`] 选择路径。

use std::sync::Arc;
use std::time::Duration;

use crate::core::{Errc, Error, Rect};
use crate::draw::geometry::types::{BlendMode, Transform};
use crate::draw::painting::FrameRect;
use crate::draw::raster::pixel_surface::PixelSurface;
use crate::draw::raster::shared_rasterizer::SharedRasterizer;
use crate::native::present::NativeRasterCaps;

use super::pending::PendingNativeOp;
use super::{StateSnapshot, SOFT_FALLBACK_IDLE_PRESENT_GRACE, SOFT_FALLBACK_IDLE_TIME_GRACE};

pub struct NativeGpuCanvas2D {
    pub(super) native_caps: NativeRasterCaps,
    pub(super) gpu_only: bool,
    /// Logical-to-physical scale of the current target. Offscreens stay at 1.
    pub(super) device_pixel_ratio: f32,
    /// Allocated on first soft-path use (#105) — pure-native frames keep no CPU framebuffer.
    pub(crate) soft_fallback: Option<SharedRasterizer>,
    /// Soft buffer has content that must be composited (until full clear).
    pub(crate) soft_has_content: bool,
    /// At least one soft operation contributed to the current swapchain frame.
    /// Segment commits do not reset this: ordered Picture boundaries may split
    /// one frame into several submissions before the final present.
    pub(super) soft_used_since_present: bool,
    /// Successful swapchain presents since this canvas last used its soft
    /// fallback. A short grace avoids allocation churn in alternating frames.
    pub(super) soft_idle_presents: u8,
    /// Destination-dependent blend cannot be faithfully composed from a
    /// transparent CPU segment over native output.
    pub(super) soft_uses_destination_blend: bool,
    /// Immediate Canvas2D calls that have no `Result` return channel record
    /// an error here. The frame boundary consumes it before any present.
    pub(super) deferred_error: Option<Error>,
    #[cfg(test)]
    /// Zero-length target used only by tests that probe rejected direct writes.
    pub(super) rejected_pixels: [u32; 0],
    pub(crate) pending_native: Vec<PendingNativeOp>,
    pub(super) clip_rect: Rect,
    pub(super) clip_stack: Vec<Rect>,
    // 与 clip_stack 对齐，true 表示对应项需要同步 pop soft path mask。
    pub(super) clip_kind_stack: Vec<bool>,
    pub(super) opacity: f32,
    pub(super) offset_x: f32,
    pub(super) offset_y: f32,
    pub(super) transform: Transform,
    pub(super) blend_mode: BlendMode,
    pub(super) state_stack: Vec<StateSnapshot>,
    pub(super) surface_w: i32,
    pub(super) surface_h: i32,
    /// Soft-upload byte count from the last successful soft fallback blit (diagnostics).
    pub(crate) last_soft_upload_bytes: usize,
}


impl NativeGpuCanvas2D {
    pub(crate) fn new(width: i32, height: i32, native_caps: NativeRasterCaps) -> Self {
        Self::new_with_mode(width, height, native_caps, false)
    }

    pub(crate) fn new_gpu_only(width: i32, height: i32, native_caps: NativeRasterCaps) -> Self {
        Self::new_with_mode(width, height, native_caps, true)
    }

    pub(super) fn new_with_mode(
        width: i32,
        height: i32,
        native_caps: NativeRasterCaps,
        gpu_only: bool,
    ) -> Self {
        let w = width.max(1);
        let h = height.max(1);
        Self {
            native_caps,
            gpu_only,
            device_pixel_ratio: 1.0,
            soft_fallback: None,
            soft_has_content: false,
            soft_used_since_present: false,
            soft_idle_presents: 0,
            soft_uses_destination_blend: false,
            deferred_error: None,
            #[cfg(test)]
            rejected_pixels: [],
            pending_native: Vec::new(),
            clip_rect: Rect::new(0.0, 0.0, w as f32, h as f32),
            clip_stack: Vec::new(),
            clip_kind_stack: Vec::new(),
            opacity: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            transform: Transform::identity(),
            blend_mode: BlendMode::default(),
            state_stack: Vec::new(),
            surface_w: w,
            surface_h: h,
            last_soft_upload_bytes: 0,
        }
    }

    pub(crate) fn set_device_pixel_ratio(&mut self, device_pixel_ratio: f32) {
        self.device_pixel_ratio = if device_pixel_ratio.is_finite() && device_pixel_ratio > 0.0 {
            device_pixel_ratio
        } else {
            1.0
        };
    }

    #[cfg(all(test, feature = "d3d11"))]
    pub(crate) fn pending_mesh_count(&self) -> usize {
        self.pending_native
            .iter()
            .filter(|op| matches!(op, PendingNativeOp::SolidMesh(_)))
            .count()
    }

    pub(crate) fn ensure_soft(&mut self) -> &mut SharedRasterizer {
        let width = self.surface_w;
        let height = self.surface_h;
        let deferred_error = &mut self.deferred_error;
        self.soft_fallback.get_or_insert_with(|| {
            let surface = match PixelSurface::try_new(width, height) {
                Ok(surface) => surface,
                Err(error) => {
                    if deferred_error.is_none() {
                        *deferred_error = Some(error);
                    }
                    PixelSurface::one_pixel()
                }
            };
            SharedRasterizer::new(surface)
        })
    }

    pub(super) fn resize(&mut self, width: i32, height: i32) {
        let w = width.max(1);
        let h = height.max(1);
        self.surface_w = w;
        self.surface_h = h;
        self.clip_rect = Rect::new(0.0, 0.0, w as f32, h as f32);
        self.clip_stack.clear();
        self.clip_kind_stack.clear();
        self.state_stack.clear();
        self.pending_native.clear();
        // Drop soft buffer on resize; recreate lazily at the new size.
        self.soft_fallback = None;
        self.soft_has_content = false;
        self.soft_used_since_present = false;
        self.soft_idle_presents = 0;
        self.soft_uses_destination_blend = false;
        self.deferred_error = None;
    }

    pub(crate) fn clear_soft(&mut self) {
        if let Some(soft) = self.soft_fallback.as_mut() {
            soft.surface_mut().clear_all();
        }
        self.soft_has_content = false;
        self.soft_uses_destination_blend = false;
        self.pending_native.clear();
    }

    pub(super) fn mark_soft(&mut self) {
        if self.gpu_only {
            self.reject_unsupported("CPU soft raster fallback in GPU-only mode");
            return;
        }
        self.soft_has_content = true;
        self.soft_used_since_present = true;
        self.soft_idle_presents = 0;
    }

    pub(crate) fn reset_for_repaint(&mut self) {
        let fallback_extent_mismatch = self.soft_fallback.as_ref().is_some_and(|soft| {
            soft.surface().width() != self.surface_w || soft.surface().height() != self.surface_h
        });
        if fallback_extent_mismatch {
            // Allocation failure installs a 1×1 safety surface. Do not let a
            // later repaint mistake that placeholder for a valid full target:
            // dropping it makes the next soft draw retry the typed allocation.
            self.soft_fallback = None;
        } else if let Some(soft) = self.soft_fallback.as_mut() {
            soft.surface_mut().clear_all();
            soft.reset_state_for_extent(self.surface_w, self.surface_h);
        }
        self.soft_has_content = false;
        self.soft_uses_destination_blend = false;
        self.deferred_error = None;
        self.pending_native.clear();
        self.clip_rect = Rect::new(0.0, 0.0, self.surface_w as f32, self.surface_h as f32);
        self.clip_stack.clear();
        self.clip_kind_stack.clear();
        self.opacity = 1.0;
        self.offset_x = 0.0;
        self.offset_y = 0.0;
        self.transform = Transform::identity();
        self.blend_mode = BlendMode::default();
        self.state_stack.clear();
    }

    pub(crate) fn take_deferred_error(&mut self) -> Option<Error> {
        self.deferred_error.take()
    }

    pub(super) fn reject_unsupported(&mut self, operation: &str) {
        if self.deferred_error.is_none() {
            self.deferred_error = Some(Error::new(
                Errc::NotImplemented,
                format!("NativeGpuCanvas2D does not implement {operation}"),
            ));
        }
    }

    pub(super) fn reject_path_clip(&mut self) {
        self.reject_unsupported("path clip");
    }

    pub(super) fn clear_soft_rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        if let Some(soft) = self.soft_fallback.as_mut() {
            soft.surface_mut().clear_rect_raw(x, y, w, h);
        }
    }

    pub(super) fn sync_fallback_state(&mut self) {
        if self.gpu_only {
            self.reject_unsupported("CPU soft raster fallback in GPU-only mode");
            return;
        }
        let transform = self.transform;
        let opacity = self.opacity;
        let blend_mode = self.blend_mode;
        let offset_x = self.offset_x;
        let offset_y = self.offset_y;
        let soft = self.ensure_soft();
        soft.set_transform(transform);
        soft.set_opacity(opacity);
        soft.set_blend_mode(blend_mode);
        soft.set_offset(offset_x, offset_y);
        self.soft_uses_destination_blend |= matches!(blend_mode, BlendMode::Additive);
    }

    pub(super) fn with_soft_clip<F>(&mut self, f: F)
    where
        F: FnOnce(&mut SharedRasterizer),
    {
        if self.gpu_only {
            self.reject_unsupported("CPU soft raster fallback in GPU-only mode");
            return;
        }
        self.sync_fallback_state();
        let clip = self.clip_rect;
        let soft = self.ensure_soft();
        soft.push_clip_surface(clip);
        f(soft);
        soft.pop_clip();
    }
}
