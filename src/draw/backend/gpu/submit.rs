//! GPU-native 提交与软回退上传 — gpu 子模块。

use crate::core::{Errc, Error};
use crate::draw::geometry::types::BlendMode;
use crate::native::present::{
    IGraphicsContext, NativeRasterCaps, PresentTestResult, RasterMode, SoftFallbackTile,
};

use super::{NativeGpuCanvas2D, SOFT_FALLBACK_IDLE_PRESENT_GRACE};
use super::pending::{PendingNativeOp, StateSnapshot};
use super::tile::pack_visible_soft_fallback_tile;

impl NativeGpuCanvas2D {
    pub(crate) fn submit_native(
        &mut self,
        gpu_ctx: &mut dyn IGraphicsContext,
    ) -> Result<(), Error> {
        let vw = self.surface_w as f32;
        let vh = self.surface_h as f32;
        let mut start = 0;
        while start < self.pending_native.len() {
            let scissor = self.pending_native[start].scissor();
            let mut end = start + 1;
            while end < self.pending_native.len()
                && self.pending_native[start].same_kind(&self.pending_native[end])
                && self.pending_native[end].scissor() == scissor
            {
                end += 1;
            }

            match &self.pending_native[start] {
                PendingNativeOp::SolidRect(_) => {
                    let batch = self.pending_native[start..end]
                        .iter()
                        .map(|op| match op {
                            PendingNativeOp::SolidRect(op) => op.rect,
                            _ => unreachable!("native batch kind changed"),
                        })
                        .collect::<Vec<_>>();
                    gpu_ctx.draw_solid_rects(vw, vh, Some(scissor), &batch)?;
                }
                PendingNativeOp::StrokeRect(_) => {
                    let batch = self.pending_native[start..end]
                        .iter()
                        .map(|op| match op {
                            PendingNativeOp::StrokeRect(op) => op.rect,
                            _ => unreachable!("native batch kind changed"),
                        })
                        .collect::<Vec<_>>();
                    gpu_ctx.draw_stroke_rects(vw, vh, Some(scissor), &batch)?;
                }
                PendingNativeOp::Glyph(_) => {
                    let batch = self.pending_native[start..end]
                        .iter()
                        .map(|op| match op {
                            PendingNativeOp::Glyph(op) => op.glyph.clone(),
                            _ => unreachable!("native batch kind changed"),
                        })
                        .collect::<Vec<_>>();
                    gpu_ctx.draw_glyphs(vw, vh, Some(scissor), &batch)?;
                }
                PendingNativeOp::LinearGradient(_) => {
                    let batch = self.pending_native[start..end]
                        .iter()
                        .map(|op| match op {
                            PendingNativeOp::LinearGradient(op) => op.rect,
                            _ => unreachable!("native batch kind changed"),
                        })
                        .collect::<Vec<_>>();
                    gpu_ctx.draw_linear_gradients(vw, vh, Some(scissor), &batch)?;
                }
                PendingNativeOp::RadialGradient(_) => {
                    let batch = self.pending_native[start..end]
                        .iter()
                        .map(|op| match op {
                            PendingNativeOp::RadialGradient(op) => op.grad,
                            _ => unreachable!("native batch kind changed"),
                        })
                        .collect::<Vec<_>>();
                    gpu_ctx.draw_radial_gradients(vw, vh, Some(scissor), &batch)?;
                }
                PendingNativeOp::Sector(_) => {
                    let batch = self.pending_native[start..end]
                        .iter()
                        .map(|op| match op {
                            PendingNativeOp::Sector(op) => op.sector,
                            _ => unreachable!("native batch kind changed"),
                        })
                        .collect::<Vec<_>>();
                    gpu_ctx.draw_sectors(vw, vh, Some(scissor), &batch)?;
                }
                PendingNativeOp::SolidMesh(_) => {
                    let batch = self.pending_native[start..end]
                        .iter()
                        .map(|op| match op {
                            PendingNativeOp::SolidMesh(op) => op.mesh.clone(),
                            _ => unreachable!("native batch kind changed"),
                        })
                        .collect::<Vec<_>>();
                    gpu_ctx.draw_solid_meshes(vw, vh, Some(scissor), &batch)?;
                }
                PendingNativeOp::BoxShadow(_) => {
                    let batch = self.pending_native[start..end]
                        .iter()
                        .map(|op| match op {
                            PendingNativeOp::BoxShadow(op) => op.shadow,
                            _ => unreachable!("native batch kind changed"),
                        })
                        .collect::<Vec<_>>();
                    gpu_ctx.draw_box_shadows(vw, vh, Some(scissor), &batch)?;
                }
                PendingNativeOp::ImageBlit(_) => {
                    let batch = self.pending_native[start..end]
                        .iter()
                        .map(|op| match op {
                            PendingNativeOp::ImageBlit(op) => op.blit.clone(),
                            _ => unreachable!("native batch kind changed"),
                        })
                        .collect::<Vec<_>>();
                    gpu_ctx.draw_image_blits(vw, vh, Some(scissor), &batch)?;
                }
            }
            start = end;
        }
        Ok(())
    }

    pub(crate) fn current_blend_mode(&self) -> BlendMode {
        self.blend_mode
    }

    pub(crate) fn submit_soft(&mut self, gpu_ctx: &mut dyn IGraphicsContext) -> Result<(), Error> {
        if !self.soft_has_content {
            return Ok(());
        }
        if self.soft_uses_destination_blend {
            return Err(Error::new(
                Errc::NotImplemented,
                "NativeGpuCanvas2D: CPU fallback cannot emulate destination-dependent Additive blend",
            ));
        }
        let w = self.surface_w;
        let h = self.surface_h;
        let packed = {
            let pixels = self.ensure_soft().surface().pixels();
            pack_visible_soft_fallback_tile(pixels, w, h)
        };
        let Some((pixels, tile)) = packed else {
            return Ok(());
        };
        self.last_soft_upload_bytes = (tile.width as usize)
            .saturating_mul(tile.height as usize)
            .saturating_mul(std::mem::size_of::<u32>());
        gpu_ctx.blit_soft_fallback_tile(&pixels, tile)?;
        Ok(())
    }

    pub(crate) fn commit_presented_frame(&mut self) {
        self.pending_native.clear();
        if self.soft_has_content {
            self.ensure_soft().surface_mut().clear_all();
            self.soft_has_content = false;
            self.soft_uses_destination_blend = false;
        }
    }

    /// Completes one successful swapchain present, then ages this canvas's idle
    /// full-size soft allocation.
    pub(crate) fn finish_presented_frame(&mut self) -> bool {
        self.commit_presented_frame();
        self.age_soft_fallback_after_present()
    }

    /// Ages an offscreen canvas at the final swapchain success boundary without
    /// committing any unflushed Picture commands.
    pub(crate) fn age_soft_fallback_after_present(&mut self) -> bool {
        if self.soft_used_since_present {
            self.soft_used_since_present = false;
            self.soft_idle_presents = 0;
            return true;
        }
        if self.soft_fallback.is_none() || self.soft_has_content || self.deferred_error.is_some() {
            return false;
        }
        self.soft_idle_presents = self.soft_idle_presents.saturating_add(1);
        if self.soft_idle_presents >= SOFT_FALLBACK_IDLE_PRESENT_GRACE {
            self.soft_fallback = None;
            self.soft_idle_presents = 0;
        }
        false
    }

    pub(super) fn release_idle_soft_fallback(&mut self) {
        if self.soft_has_content || self.soft_used_since_present || self.deferred_error.is_some() {
            return;
        }
        self.soft_fallback = None;
        self.soft_idle_presents = 0;
    }

    /// A successful Picture flush has copied every soft pixel into its durable
    /// GPU render target. Unlike the swapchain canvas, an eligible Picture is
    /// static by policy, so keeping a second full-size CPU surface for a future
    /// repaint is not worth the resident memory.
    pub(super) fn release_committed_picture_staging(&mut self) {
        if self.soft_has_content || !self.pending_native.is_empty() || self.deferred_error.is_some()
        {
            return;
        }
        self.soft_fallback = None;
        self.soft_used_since_present = false;
        self.soft_idle_presents = 0;
        self.soft_uses_destination_blend = false;
    }
}