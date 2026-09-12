//! [`NativeGpuCanvas2D`] 定义与生命周期管理 — gpu 子模块。
//!
//! 软回退（scratch）状态、裁剪/变换折叠与 GPU-only 模式判定；绘制操作在
//! [`super::queue`] 入队、[`super::submit`] 提交、[`super::canvas2d`] 选择路径。

// 引入共享像素载荷，避免 soft 分段在 RHI lowering 时重复复制。
use std::sync::Arc;

use crate::core::{Errc, Error, Rect};
use crate::draw::geometry::types::{BlendMode, Transform};
use crate::draw::raster::pixel_surface::PixelSurface;
use crate::draw::raster::shared_rasterizer::SharedRasterizer;

use super::pending::PendingNativeOp;
// 引入同一 graphics backend Module 拥有的 renderer 能力投影。
use super::NativeRasterCaps;
// 复用与 legacy/RHI 提交相同的最小可见 tile 打包规则。
use super::StateSnapshot;
use super::tile::{SoftFallbackTile, pack_visible_soft_fallback_tile};

// 保存一个已经封口、可按固定 blend 语义提交的 CPU soft 段。
#[derive(Debug, Clone)]
pub(crate) struct PackedSoftSegment {
    // 保存紧密排列的 premultiplied BGRA 像素。
    pub(crate) pixels: Arc<Vec<u32>>,
    // 保存该紧密载荷在目标中的逻辑位置。
    pub(crate) tile: SoftFallbackTile,
    // 标记该段必须使用 source + destination 的 Additive pipeline。
    pub(crate) additive: bool,
}

pub(crate) struct NativeGpuCanvas2D {
    pub(super) native_caps: NativeRasterCaps,
    pub(super) gpu_only: bool,
    /// Logical-to-physical scale of the current target. Offscreens stay at 1.
    pub(super) device_pixel_ratio: f32,
    /// Allocated on first soft-path use (#105) — pure-native frames keep no CPU framebuffer.
    pub(crate) soft_fallback: Option<SharedRasterizer>,
    /// Soft buffer has content that must be composited (until full clear).
    pub(crate) soft_has_content: bool,
    // 标记当前可变 soft surface 是否包含尚未封口的像素。
    pub(crate) soft_current_has_content: bool,
    // 保存 blend 切换时已经封口的 soft 段，并保持 painter order。
    pub(crate) pending_soft_segments: Vec<PackedSoftSegment>,
    // 保存当前可变 soft 段归一化后的固定 blend 语义。
    pub(crate) soft_segment_blend: Option<BlendMode>,
    /// At least one soft operation contributed to the current swapchain frame.
    /// Segment commits do not reset this: ordered Picture boundaries may split
    /// one frame into several submissions before the final present.
    pub(super) soft_used_since_present: bool,
    /// Successful swapchain presents since this canvas last used its soft
    /// fallback. A short grace avoids allocation churn in alternating frames.
    pub(super) soft_idle_presents: u8,
    /// Legacy soft-tile upload cannot faithfully compose a destination-dependent
    /// blend; segmented RHI lowering uses this flag only to reject that fallback.
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
            // 新画布没有尚未封口的 soft 像素。
            soft_current_has_content: false,
            // 新画布没有历史 soft 段。
            pending_soft_segments: Vec::new(),
            // 第一次 soft 操作再确定当前段的 blend。
            soft_segment_blend: None,
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

    // 把 Alpha 与 SrcOver 归一化为同一个 premultiplied 合成段。
    fn normalized_soft_blend(blend_mode: BlendMode) -> BlendMode {
        // Additive 必须独立成段，其余公开模式都使用 SrcOver 等价式。
        if matches!(blend_mode, BlendMode::Additive) {
            // 保留加法段标识。
            BlendMode::Additive
        } else {
            // Alpha 与 SrcOver 共用同一固定管线。
            BlendMode::SrcOver
        }
    }

    // 打包当前可变 surface，但不消费任何 staging 状态。
    fn packed_current_soft_segment(&self) -> Option<PackedSoftSegment> {
        // 没有当前段内容时不制造空 texture。
        if !self.soft_current_has_content {
            // 保持空段无提交语义。
            return None;
        }
        // 取得当前 soft surface 的最小可见 tile。
        let (pixels, tile) = self.soft_fallback.as_ref().and_then(|soft| {
            // 使用目标逻辑尺寸裁剪并紧密打包。
            pack_visible_soft_fallback_tile(soft.surface().pixels(), self.surface_w, self.surface_h)
        })?;
        // 返回带固定 blend 事实的不可变段。
        Some(PackedSoftSegment {
            // 转为共享载荷供 RHI 临时 texture 上传。
            pixels: Arc::new(pixels),
            // 保留紧密 tile 的目标位置。
            tile,
            // 当前段只有归一化 Additive 时才使用加法管线。
            additive: matches!(self.soft_segment_blend, Some(BlendMode::Additive)),
        })
    }

    // 返回所有已封口段及当前段的只读快照，保持原始 painter order。
    pub(crate) fn packed_soft_segments(&self) -> Vec<PackedSoftSegment> {
        // 先复用已封口段的共享像素载荷。
        let mut segments = self.pending_soft_segments.clone();
        // 当前可变段始终位于所有已封口段之后。
        if let Some(current) = self.packed_current_soft_segment() {
            // 追加当前段完成本次提交快照。
            segments.push(current);
        }
        // 返回不消费 staging 的完整顺序视图。
        segments
    }

    // 在 blend 切换时封口当前段并清空可变像素 surface。
    fn seal_current_soft_segment(&mut self) {
        // 先打包当前内容，避免清理 surface 后丢失像素。
        let segment = self.packed_current_soft_segment();
        // 只有存在可见像素时才保存提交段。
        if let Some(segment) = segment {
            // 保持该段在后续段之前提交。
            self.pending_soft_segments.push(segment);
        }
        // 清空可变 surface，让下一个 blend 段从透明背景开始。
        if let Some(soft) = self.soft_fallback.as_mut() {
            // 只清像素，不破坏 transform、clip 与状态栈。
            soft.surface_mut().clear_all();
        }
        // 当前 surface 已经没有未封口内容。
        self.soft_current_has_content = false;
    }

    // 在一次 soft 操作前建立与其 blend 匹配的连续段。
    pub(super) fn prepare_soft_segment(&mut self, blend_mode: BlendMode) {
        // 把公开 blend 模式收敛为两个可由 RHI 精确合成的类别。
        let normalized = Self::normalized_soft_blend(blend_mode);
        // 相同 blend 的连续操作继续写入当前段。
        if self.soft_segment_blend == Some(normalized) {
            // 不产生无意义的分段边界。
            return;
        }
        // 已有像素必须在切换 blend 前封口。
        if self.soft_current_has_content {
            // 保存前一段并透明初始化可变 surface。
            self.seal_current_soft_segment();
        }
        // 后续 soft 操作使用新的固定 blend 事实。
        self.soft_segment_blend = Some(normalized);
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
        // resize 丢弃当前段内容。
        self.soft_current_has_content = false;
        // resize 丢弃旧尺寸下的已封口段。
        self.pending_soft_segments.clear();
        // 新尺寸由下一次 soft 操作重新选择 blend。
        self.soft_segment_blend = None;
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
        // 完整清理同时清除当前段标记。
        self.soft_current_has_content = false;
        // 完整清理丢弃尚未提交的历史段。
        self.pending_soft_segments.clear();
        // 后续操作重新建立 blend 段。
        self.soft_segment_blend = None;
        self.soft_uses_destination_blend = false;
        self.pending_native.clear();
    }

    pub(super) fn mark_soft(&mut self) {
        if self.gpu_only {
            self.reject_unsupported("CPU soft raster fallback in GPU-only mode");
            return;
        }
        self.soft_has_content = true;
        // 当前可变 surface 已经包含本次 soft 操作。
        self.soft_current_has_content = true;
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
        // repaint 会透明清理当前 soft surface。
        self.soft_current_has_content = false;
        // repaint 不保留上一帧已经封口的段。
        self.pending_soft_segments.clear();
        // 下一次 soft 操作重新确定段 blend。
        self.soft_segment_blend = None;
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
        // 在写入像素前按 blend 变化封口前一段。
        self.prepare_soft_segment(blend_mode);
        // 取得与当前段对应的可变 soft renderer。
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

// cfg(test) 完整辅助实现位于 tests-src，仅测试构建编译。
#[cfg(test)]
#[path = "../../../../tests-src/draw/backend/gpu/canvas_tests.rs"]
mod canvas_tests;