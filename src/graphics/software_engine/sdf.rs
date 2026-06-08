use super::core::RenderTarget;
use crate::graphics::{Radius, Rect};

impl RenderTarget {
    /// Signed distance field for a rounded rectangle with per-corner radii.
    ///
    /// Uses the Inigo Quilez SDF formula, adapted to support independent
    /// corner radii (tl, tr, bl, br). Pixel quadrant determines which radius
    /// applies. The SDF is C⁰ continuous across quadrant boundaries.
    pub fn rounded_rect_sdf(ux: f32, uy: f32, r: &Rect, rad: &Radius) -> f32 {
        // 直角矩形（所有圆角为零）→ 简化为 box SDF
        if rad.tl == 0.0 && rad.tr == 0.0 && rad.bl == 0.0 && rad.br == 0.0 {
            let dx = (r.x - ux).max(ux - (r.x + r.w)).max(0.0);
            let dy = (r.y - uy).max(uy - (r.y + r.h)).max(0.0);
            let outside = (dx * dx + dy * dy).sqrt();
            let inside = (r.x - ux)
                .max(ux - (r.x + r.w))
                .max((r.y - uy).max(uy - (r.y + r.h)));
            return if inside < 0.0 { inside } else { outside };
        }

        let cx = r.x + r.w * 0.5;
        let cy = r.y + r.h * 0.5;
        let half_w = r.w * 0.5;
        let half_h = r.h * 0.5;
        let px = ux - cx;
        let py = uy - cy;

        let cr = if px < 0.0 {
            if py < 0.0 {
                rad.tl
            } else {
                rad.bl
            }
        } else {
            if py < 0.0 {
                rad.tr
            } else {
                rad.br
            }
        };

        let qx = px.abs() - half_w + cr;
        let qy = py.abs() - half_h + cr;
        let qx_clamped = qx.max(0.0);
        let qy_clamped = qy.max(0.0);
        let outside = (qx_clamped * qx_clamped + qy_clamped * qy_clamped).sqrt();
        let inside = qx.max(qy).min(0.0);
        outside + inside - cr
    }

    /// Signed distance from point to a line segment.
    pub fn line_segment_sdf(ux: f32, uy: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let length_sq = dx * dx + dy * dy;
        if length_sq < 1e-12 {
            let dx0 = ux - x1;
            let dy0 = uy - y1;
            return (dx0 * dx0 + dy0 * dy0).sqrt();
        }
        let t = ((ux - x1) * dx + (uy - y1) * dy) / length_sq;
        let t = t.clamp(0.0, 1.0);
        let px = x1 + t * dx;
        let py = y1 + t * dy;
        ((ux - px).powi(2) + (uy - py).powi(2)).sqrt()
    }

    /// Convert SDF value to pixel coverage with configurable antialiasing half-width.
    ///
    /// This is used for filled shapes, where the contour at sd=0 should be 50%
    /// coverage and interior pixels may reach full coverage as sd becomes negative.
    pub fn sdf_to_coverage_aa(sd: f32, aa_half: f32) -> f32 {
        ((aa_half - sd) / (2.0 * aa_half)).clamp(0.0, 1.0)
    }

    /// Convert SDF value to pixel coverage (AA half-width = 0.5).
    pub fn sdf_to_coverage(sd: f32) -> f32 {
        Self::sdf_to_coverage_aa(sd, 0.5)
    }
}
