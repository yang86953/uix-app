//! 字形批：R8 coverage atlas 与 RGBA8 MSDF atlas 的槽位分配与顶点入队。

use std::sync::Arc;

use crate::core::{Errc, Error, Result};
use crate::native::graphics::wgpu_backend::draw_stream::{
    grow_zeroed, ndc, BatchKind, DrawStream, GlyphVertex, GLYPH_ATLAS_HEIGHT, GLYPH_ATLAS_WIDTH,
};
use crate::native::graphics::wgpu_backend::glyph_cover::GlyphCoverDraw;
use crate::native::traits::present::GpuGlyphBlit;

/// 将字形 blit 记入绘制流（R8 coverage 或轮廓 MSDF）。
pub(super) fn record_glyphs(
    stream: &mut DrawStream,
    viewport: (f32, f32),
    scissor: Option<(i32, i32, i32, i32)>,
    glyphs: &[GpuGlyphBlit],
) -> Result<()> {
    // 保持提交顺序：R8 coverage 与 MSDF 交错时按 kind 切批。
    let mut active_kind: Option<BatchKind> = None;
    let mut batch_first = 0u32;
    for glyph in glyphs {
        if glyph.cov_w == 0 || glyph.cov_h == 0 {
            continue;
        }
        if glyph.cov_w > GLYPH_ATLAS_WIDTH || glyph.cov_h > GLYPH_ATLAS_HEIGHT {
            return Err(Error::new(
                Errc::InvalidArgument,
                "wgpu glyph coverage does not fit the shared atlas",
            ));
        }
        let outline = glyph
            .outline_mesh
            .as_ref()
            .filter(|mesh| crate::draw::font::glyph_outline::is_outline_edges(mesh))
            .cloned();
        let expected = (glyph.cov_w as usize).saturating_mul(glyph.cov_h as usize);
        if outline.is_none() && glyph.coverage.len() < expected {
            return Err(Error::new(
                Errc::InvalidArgument,
                "wgpu glyph coverage does not fit the shared atlas",
            ));
        }
        let kind = if outline.is_some() {
            BatchKind::GlyphMsdf
        } else {
            BatchKind::Glyph
        };
        if active_kind != Some(kind) {
            if let Some(prev) = active_kind {
                let verts = match prev {
                    BatchKind::Glyph => stream.glyphs.len() as u32,
                    BatchKind::GlyphMsdf => stream.msdf_glyphs.len() as u32,
                    _ => 0,
                };
                let count = verts.saturating_sub(batch_first);
                stream.push_batch(prev, batch_first, count, viewport, scissor);
            }
            active_kind = Some(kind);
            batch_first = match kind {
                BatchKind::Glyph => stream.glyphs.len() as u32,
                BatchKind::GlyphMsdf => stream.msdf_glyphs.len() as u32,
                _ => 0,
            };
        }
        let key = if let Some(ref mesh) = outline {
            (mesh.as_ptr() as usize, glyph.cov_w, glyph.cov_h)
        } else {
            (glyph.coverage.as_ptr() as usize, glyph.cov_w, glyph.cov_h)
        };
        let (atlas_x, atlas_y) = if outline.is_some() {
            if let Some(location) = stream.msdf_locations.get(&key) {
                *location
            } else {
                if stream.msdf_cursor_x + glyph.cov_w > GLYPH_ATLAS_WIDTH {
                    stream.msdf_cursor_x = 0;
                    stream.msdf_cursor_y =
                        stream.msdf_cursor_y.saturating_add(stream.msdf_row_height);
                    stream.msdf_row_height = 0;
                }
                if stream.msdf_cursor_y + glyph.cov_h > GLYPH_ATLAS_HEIGHT {
                    return Err(Error::new(
                        Errc::GraphicsOutOfMemory,
                        "wgpu per-frame MSDF glyph atlas is full",
                    ));
                }
                let location = (stream.msdf_cursor_x, stream.msdf_cursor_y);
                if let Some(mesh) = outline.clone() {
                    // 槽位由 GPU MSDF cover pass 写入 RGBA8。
                    stream.glyph_covers.push(GlyphCoverDraw {
                        atlas_x: location.0,
                        atlas_y: location.1,
                        width: glyph.cov_w,
                        height: glyph.cov_h,
                        mesh: Arc::clone(&mesh),
                    });
                    stream.glyph_mesh_sources.push(mesh);
                }
                stream.msdf_cursor_x = stream.msdf_cursor_x.saturating_add(glyph.cov_w + 2);
                stream.msdf_row_height = stream.msdf_row_height.max(glyph.cov_h + 2);
                stream.msdf_used_height = stream.msdf_used_height.max(location.1 + glyph.cov_h);
                stream.msdf_locations.insert(key, location);
                location
            }
        } else if let Some(location) = stream.glyph_locations.get(&key) {
            *location
        } else {
            if stream.glyph_cursor_x + glyph.cov_w > GLYPH_ATLAS_WIDTH {
                stream.glyph_cursor_x = 0;
                stream.glyph_cursor_y =
                    stream.glyph_cursor_y.saturating_add(stream.glyph_row_height);
                stream.glyph_row_height = 0;
            }
            if stream.glyph_cursor_y + glyph.cov_h > GLYPH_ATLAS_HEIGHT {
                return Err(Error::new(
                    Errc::GraphicsOutOfMemory,
                    "wgpu per-frame glyph atlas is full",
                ));
            }
            let location = (stream.glyph_cursor_x, stream.glyph_cursor_y);
            let needed = ((location.1 + glyph.cov_h) * GLYPH_ATLAS_WIDTH) as usize;
            grow_zeroed(&mut stream.glyph_pixels, needed);
            for row in 0..glyph.cov_h as usize {
                let destination =
                    (location.1 as usize + row) * GLYPH_ATLAS_WIDTH as usize + location.0 as usize;
                let source = row * glyph.cov_w as usize;
                stream.glyph_pixels[destination..destination + glyph.cov_w as usize]
                    .copy_from_slice(&glyph.coverage[source..source + glyph.cov_w as usize]);
            }
            stream.glyph_sources.push(Arc::clone(&glyph.coverage));
            stream.glyph_cursor_x = stream.glyph_cursor_x.saturating_add(glyph.cov_w + 1);
            stream.glyph_row_height = stream.glyph_row_height.max(glyph.cov_h + 1);
            stream.glyph_used_height = stream.glyph_used_height.max(location.1 + glyph.cov_h);
            stream.glyph_locations.insert(key, location);
            location
        };
        let u0 = atlas_x as f32 / GLYPH_ATLAS_WIDTH as f32;
        let v0 = atlas_y as f32 / GLYPH_ATLAS_HEIGHT as f32;
        let u1 = (atlas_x + glyph.cov_w) as f32 / GLYPH_ATLAS_WIDTH as f32;
        let v1 = (atlas_y + glyph.cov_h) as f32 / GLYPH_ATLAS_HEIGHT as f32;
        let positions = [
            glyph.corners[0],
            glyph.corners[1],
            glyph.corners[2],
            glyph.corners[0],
            glyph.corners[2],
            glyph.corners[3],
        ];
        let uvs = [[u0, v0], [u1, v0], [u1, v1], [u0, v0], [u1, v1], [u0, v1]];
        let dest = if kind == BatchKind::GlyphMsdf {
            &mut stream.msdf_glyphs
        } else {
            &mut stream.glyphs
        };
        for (position, uv) in positions.into_iter().zip(uvs) {
            dest.push(GlyphVertex {
                position: ndc(position[0], position[1], viewport),
                uv,
                color: glyph.rgba,
            });
        }
    }
    if let Some(prev) = active_kind {
        let verts = match prev {
            BatchKind::Glyph => stream.glyphs.len() as u32,
            BatchKind::GlyphMsdf => stream.msdf_glyphs.len() as u32,
            _ => 0,
        };
        let count = verts.saturating_sub(batch_first);
        stream.push_batch(prev, batch_first, count, viewport, scissor);
    }
    Ok(())
}
