//! [`GpuBackend`] 帧命令绘制降级 — backend 子模块。
//!
//! encoder 命令到 GPU 图元的直接绘制（字形批、描边、图片 blit），以及
//! destination-dependent 操作的 readback → 参考执行 → 上传回退。

use std::sync::Arc;

use crate::core::{Errc, Error, Point, Rect};
use crate::draw::geometry::color::Color;
use crate::draw::painting::{
    FrameCommand, FrameEncoder, FrameGlyphBlit, FrameImage, FrameRasterOp, FrameRect,
    FrameStrokeRect, ReferenceFrame,
};
use crate::draw::raster::pixel_surface::PixelSurface;
use crate::native::present::{
    GpuGlyphBlit, GpuImageBlit, GpuSolidMesh, GpuSolidRect, GpuStrokeRect, IGraphicsContext,
    RasterMode, SoftFallbackTile,
};

use super::super::canvas::NativeGpuCanvas2D;
use super::super::pending::PendingNativeOp;
use super::super::tile::pack_visible_soft_fallback_tile;
use super::helpers::{glyph_visible_bounds, union_frame_rect_wide};
use super::GpuBackend;

impl GpuBackend {
    pub(super) fn draw_frame_glyphs(
        &mut self,
        target_width: i32,
        target_height: i32,
        glyphs: &[FrameGlyphBlit],
        clip: FrameRect,
    ) -> Result<(), Error> {
        let Some(clip) = clip.intersection(FrameRect::new(0, 0, target_width, target_height))
        else {
            return Ok(());
        };
        // Native D3D11/D3D12/OpenGL R8 atlases accept glyphs up to 2048px per side.
        // Arbitrary Canvas inputs above that shared limit use the exact compact
        // fallback instead of turning frame submission into a typed GPU failure.
        let can_draw_native = self.surface.native_caps.glyphs
            && glyphs
                .iter()
                .all(|glyph| glyph.width() <= 2048 && glyph.height() <= 2048);
        if can_draw_native {
            let glyphs = glyphs
                .iter()
                .map(|glyph| {
                    let x = glyph.x() as f32;
                    let y = glyph.y() as f32;
                    let w = glyph.width() as f32;
                    let h = glyph.height() as f32;
                    GpuGlyphBlit {
                        x,
                        y,
                        w,
                        h,
                        corners: GpuGlyphBlit::axis_aligned_corners(x, y, w, h),
                        rgba: [
                            glyph.color().r as f32 / 255.0,
                            glyph.color().g as f32 / 255.0,
                            glyph.color().b as f32 / 255.0,
                            glyph.color().a as f32 / 255.0,
                        ],
                        coverage: Arc::clone(glyph.coverage()),
                        cov_w: glyph.width(),
                        cov_h: glyph.height(),
                        outline_mesh: None,
                    }
                })
                .collect::<Vec<_>>();
            return self.gpu_ctx.draw_glyphs(
                target_width as f32,
                target_height as f32,
                Some((clip.x, clip.y, clip.width, clip.height)),
                &glyphs,
            );
        }

        self.draw_frame_glyph_fallback_clusters(target_width, target_height, glyphs, clip)
    }

    fn draw_frame_glyph_fallback_clusters(
        &mut self,
        target_width: i32,
        target_height: i32,
        glyphs: &[FrameGlyphBlit],
        clip: FrameRect,
    ) -> Result<(), Error> {
        const MAX_CLUSTER_PIXELS: i64 = 1024 * 1024;
        const MAX_UNION_INFLATION: i64 = 4;

        let mut cluster = Vec::<&FrameGlyphBlit>::new();
        let mut cluster_bounds = None;
        let mut covered_area = 0i64;
        for glyph in glyphs {
            let Some(bounds) = glyph_visible_bounds(glyph, clip, target_width, target_height)
            else {
                continue;
            };
            let glyph_area = i64::from(bounds.width) * i64::from(bounds.height);
            let union =
                cluster_bounds.map_or(bounds, |previous| union_frame_rect_wide(previous, bounds));
            let union_area = i64::from(union.width) * i64::from(union.height);
            let next_covered = covered_area.saturating_add(glyph_area);
            if !cluster.is_empty()
                && (union_area > MAX_CLUSTER_PIXELS
                    || union_area > next_covered.saturating_mul(MAX_UNION_INFLATION))
            {
                let Some(previous_bounds) = cluster_bounds else {
                    return Err(Error::new(
                        Errc::InvalidState,
                        "native glyph fallback cluster lost its non-empty bounds",
                    ));
                };
                self.flush_frame_glyph_fallback_cluster(&cluster, previous_bounds)?;
                cluster.clear();
                cluster_bounds = Some(bounds);
                covered_area = glyph_area;
            } else {
                cluster_bounds = Some(union);
                covered_area = next_covered;
            }
            cluster.push(glyph);
        }
        if let Some(bounds) = cluster_bounds {
            self.flush_frame_glyph_fallback_cluster(&cluster, bounds)?;
        }
        Ok(())
    }

    fn flush_frame_glyph_fallback_cluster(
        &mut self,
        glyphs: &[&FrameGlyphBlit],
        bounds: FrameRect,
    ) -> Result<(), Error> {
        const MAX_TILE_SIDE: i32 = 1024;
        let bottom = bounds.y.saturating_add(bounds.height);
        let right = bounds.x.saturating_add(bounds.width);
        let mut y = bounds.y;
        while y < bottom {
            let tile_height = (bottom - y).min(MAX_TILE_SIDE);
            let mut x = bounds.x;
            while x < right {
                let tile_width = (right - x).min(MAX_TILE_SIDE);
                self.flush_frame_glyph_fallback_tile(
                    glyphs,
                    FrameRect::new(x, y, tile_width, tile_height),
                )?;
                x = x.saturating_add(tile_width);
            }
            y = y.saturating_add(tile_height);
        }
        Ok(())
    }

    fn flush_frame_glyph_fallback_tile(
        &mut self,
        glyphs: &[&FrameGlyphBlit],
        bounds: FrameRect,
    ) -> Result<(), Error> {
        let mut surface = PixelSurface::try_new(bounds.width, bounds.height)?;
        let local_clip = Rect::new(0.0, 0.0, bounds.width as f32, bounds.height as f32);
        for glyph in glyphs {
            crate::draw::raster::rasterizer::glyph::blit_glyph(
                surface.pixels_mut(),
                bounds.width,
                bounds.height,
                local_clip,
                1.0,
                glyph.x().saturating_sub(bounds.x),
                glyph.y().saturating_sub(bounds.y),
                glyph.coverage().as_ref(),
                glyph.width() as usize,
                glyph.height() as usize,
                glyph.color(),
            );
        }
        if let Some((packed, local_tile)) =
            pack_visible_soft_fallback_tile(surface.pixels(), bounds.width, bounds.height)
        {
            self.gpu_ctx.blit_soft_fallback_tile(
                &packed,
                SoftFallbackTile::at_destination(
                    bounds.x.saturating_add(local_tile.dst_x),
                    bounds.y.saturating_add(local_tile.dst_y),
                    local_tile.width,
                    local_tile.height,
                ),
            )?;
        }
        Ok(())
    }

    pub(super) fn draw_frame_solid_rect(
        &mut self,
        target_width: i32,
        target_height: i32,
        rect: FrameRect,
        color: Color,
        radius: [f32; 4],
        clip: Option<FrameRect>,
    ) -> Result<(), Error> {
        if rect.width <= 0 || rect.height <= 0 {
            return Ok(());
        }
        self.gpu_ctx.draw_solid_rects(
            target_width as f32,
            target_height as f32,
            clip.map(|clip| (clip.x, clip.y, clip.width, clip.height)),
            &[GpuSolidRect {
                x: rect.x as f32,
                y: rect.y as f32,
                w: rect.width as f32,
                h: rect.height as f32,
                rgba: [
                    color.r as f32 / 255.0,
                    color.g as f32 / 255.0,
                    color.b as f32 / 255.0,
                    color.a as f32 / 255.0,
                ],
                radius,
            }],
        )
    }

    pub(super) fn draw_frame_stroke_rects(
        &mut self,
        target_width: i32,
        target_height: i32,
        strokes: &[FrameStrokeRect],
        clip: FrameRect,
    ) -> Result<(), Error> {
        let mut rects = Vec::new();
        rects.try_reserve_exact(strokes.len()).map_err(|error| {
            Error::new(
                Errc::GraphicsOutOfMemory,
                format!(
                    "native frame stroke batch allocation for {} items failed: {error}",
                    strokes.len()
                ),
            )
        })?;
        for stroke in strokes {
            let rect = stroke.rect();
            if rect.width <= 0 || rect.height <= 0 {
                continue;
            }
            let color = stroke.color();
            let radius = stroke.radius().to_radius();
            rects.push(GpuStrokeRect {
                x: rect.x as f32,
                y: rect.y as f32,
                w: rect.width as f32,
                h: rect.height as f32,
                rgba: [
                    color.r as f32 / 255.0,
                    color.g as f32 / 255.0,
                    color.b as f32 / 255.0,
                    color.a as f32 / 255.0,
                ],
                radius: [radius.tl, radius.tr, radius.br, radius.bl],
                line_width: stroke.line_width().value(),
            });
        }
        if rects.is_empty() {
            return Ok(());
        }
        self.gpu_ctx.draw_stroke_rects(
            target_width as f32,
            target_height as f32,
            Some((clip.x, clip.y, clip.width, clip.height)),
            &rects,
        )
    }

    /// Destination-dependent IR ops cannot be drawn with SrcOver GPU quads.
    /// When the context supports readback + full upload, apply the reference
    /// semantics on CPU pixels and replace the RT — pixel-correct, not an
    /// alpha-over approximation. Otherwise return typed NotImplemented.
    pub(super) fn execute_destination_dependent_frame_op(
        &mut self,
        target_width: i32,
        target_height: i32,
        operation: &FrameRasterOp,
    ) -> Result<(), Error> {
        let expected = (target_width as usize).saturating_mul(target_height as usize);
        let mut pixels = self
            .gpu_ctx
            .read_pixels(0, 0, target_width, target_height)
            .map_err(|error| {
                if error.code() == Errc::NotImplemented {
                    Error::new(
                        Errc::NotImplemented,
                        "GpuBackend: destination-dependent FrameRasterOp requires readback",
                    )
                    .with_source(error)
                } else {
                    error
                }
            })?;
        if pixels.len() != expected {
            return Err(Error::new(
                Errc::InvalidState,
                format!(
                    "GpuBackend: destination-dependent FrameRasterOp readback extent mismatch (got {}, expected {expected})",
                    pixels.len()
                ),
            ));
        }
        crate::draw::painting::encoder::apply_frame_raster_op(
            target_width,
            target_height,
            &mut pixels,
            operation,
        );
        self.gpu_ctx
            .upload_surface_pixels(&pixels, target_width, target_height)
            .map_err(|error| {
                if error.code() == Errc::NotImplemented {
                    Error::new(
                        Errc::NotImplemented,
                        "GpuBackend: destination-dependent FrameRasterOp requires replace upload",
                    )
                    .with_source(error)
                } else {
                    error
                }
            })
    }

    pub(super) fn execute_frame_image_blit(
        &mut self,
        encoder: &FrameEncoder,
        image: &crate::draw::painting::FrameImage,
        src: FrameRect,
        dst: crate::draw::painting::FrameSampledRect,
        opacity: f32,
        additive: bool,
    ) -> Result<(), Error> {
        if !opacity.is_finite() || opacity <= 0.0 {
            return Ok(());
        }
        // Additive 必须走 GPU 纹理 pipeline；soft tile 上传是 SrcOver，不能冒充。
        if self.surface.native_caps.soft_blit && !additive {
            let frame_opacity = crate::draw::painting::FrameOpacity::from_canvas(opacity);
            if let Some((source, destination)) =
                encoder.picture_blit_reference_tile(image, src, dst, frame_opacity)
            {
                return self.alpha_blit_frame_encoder_source(&source, destination);
            }
            return Ok(());
        }
        // 严格 GPU：整块源 crop 上传为纹理四边形，目标可为亚像素 / 缩放。
        self.gpu_texture_blit_frame_image(image, src, dst, opacity, additive)
    }

    fn gpu_texture_blit_frame_image(
        &mut self,
        image: &crate::draw::painting::FrameImage,
        src: FrameRect,
        dst: crate::draw::painting::FrameSampledRect,
        opacity: f32,
        additive: bool,
    ) -> Result<(), Error> {
        if src.width <= 0
            || src.height <= 0
            || dst.width() <= 0.0
            || dst.height() <= 0.0
            || !src.is_within(image.width(), image.height())
        {
            return Ok(());
        }
        let pixel_count = usize::try_from(
            i64::from(src.width).saturating_mul(i64::from(src.height)),
        )
        .map_err(|_| {
            Error::new(
                Errc::GraphicsOutOfMemory,
                "FrameEncoder GPU image blit crop exceeds addressable memory",
            )
        })?;
        let mut retained = Vec::new();
        retained.try_reserve_exact(pixel_count).map_err(|error| {
            Error::new(
                Errc::GraphicsOutOfMemory,
                format!("FrameEncoder GPU image blit crop allocation failed: {error}"),
            )
        })?;
        let stride = image.width() as usize;
        let copy_width = src.width as usize;
        let pixels = image.pixels();
        for y in src.y..src.y + src.height {
            let row = y as usize * stride + src.x as usize;
            retained.extend_from_slice(&pixels[row..row + copy_width]);
        }
        let viewport_w = self.surface.width as f32;
        let viewport_h = self.surface.height as f32;
        let blit = GpuImageBlit {
            x: dst.x(),
            y: dst.y(),
            w: dst.width(),
            h: dst.height(),
            corners: GpuGlyphBlit::axis_aligned_corners(
                dst.x(),
                dst.y(),
                dst.width(),
                dst.height(),
            ),
            opacity: opacity.clamp(0.0, 1.0),
            additive,
            pixels: std::sync::Arc::<[u32]>::from(retained),
            pixel_w: src.width as u32,
            pixel_h: src.height as u32,
        };
        self.gpu_ctx
            .draw_image_blits(viewport_w, viewport_h, None, &[blit])?;
        self.surface.canvas.last_soft_upload_bytes = 0;
        Ok(())
    }

    pub(super) fn alpha_blit_frame_encoder_source(
        &mut self,
        source: &ReferenceFrame,
        destination: FrameRect,
    ) -> Result<(), Error> {
        if let Some((pixels, tile)) =
            pack_visible_soft_fallback_tile(source.pixels(), source.width(), source.height())
        {
            let tile = SoftFallbackTile::at_destination(
                destination.x.checked_add(tile.dst_x).ok_or_else(|| {
                    Error::new(
                        Errc::InvalidState,
                        "FrameEncoder soft tile destination x overflowed",
                    )
                })?,
                destination.y.checked_add(tile.dst_y).ok_or_else(|| {
                    Error::new(
                        Errc::InvalidState,
                        "FrameEncoder soft tile destination y overflowed",
                    )
                })?,
                tile.width,
                tile.height,
            );
            self.gpu_ctx.blit_soft_fallback_tile(&pixels, tile)?;
            self.surface.canvas.last_soft_upload_bytes =
                pixels.len().saturating_mul(std::mem::size_of::<u32>());
        }
        Ok(())
    }
}
