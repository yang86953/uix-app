//! Box shadow rendering — SDF-based Gaussian-like blurred drop shadows.
//!
//! Each shadow layer is rendered by computing the signed distance field (SDF)
//! of the offset rounded rectangle, then mapping it to coverage using a
//! smoothstep falloff over the blur radius. This produces a smooth,
//! natural-looking shadow that closely matches CSS box-shadow behavior.
//!
//! - `sd = 0` at the shape edge → 50% coverage
//! - `sd < -blur` (well inside) → 100% coverage (fully shadowed)
//! - `sd > blur` (well outside) → 0% coverage (no shadow)

use super::core::RenderTarget;
use crate::graphics::{Color, Radius, Rect};

impl RenderTarget {
    /// Draw a single-layer box shadow.
    ///
    /// * `rect` — the shape casting the shadow (before offset)
    /// * `blur_radius` — CSS-like blur radius (spread of the Gaussian falloff)
    /// * `offset_x`, `offset_y` — shadow offset relative to `rect`
    /// * `shadow_color` — RGBA color of the shadow
    /// * `corner_radius` — corner radii of the shape (inherited by shadow)
    pub fn draw_box_shadow(
        &mut self,
        rect: Rect,
        blur_radius: f32,
        offset_x: f32,
        offset_y: f32,
        shadow_color: Color,
        corner_radius: Option<Radius>,
    ) {
        let rad = corner_radius.unwrap_or_default();
        let blur = blur_radius.max(0.0);

        if shadow_color.a == 0 {
            return;
        }

        let color = self.apply_opacity(Self::premul(shadow_color));

        // Offset the shadow rect relative to the original shape
        let shadow_rect = Rect::new(rect.x + offset_x, rect.y + offset_y, rect.w, rect.h);

        // The shadow influence area extends `blur` px beyond the shadow rect
        // (plus 1px for base antialiasing)
        let expand = blur + 1.0;
        let bounds = Rect::new(
            shadow_rect.x - expand,
            shadow_rect.y - expand,
            shadow_rect.w + expand * 2.0,
            shadow_rect.h + expand * 2.0,
        );

        let use_blur = blur > 0.5;

        if let Some(cr) = self.intersect_clip(&bounds) {
            let x0 = cr.x as i32;
            let y0 = cr.y as i32;
            let x1 = (cr.x + cr.w) as i32;
            let y1 = (cr.y + cr.h) as i32;

            // Early-return if the clipped area is empty
            if x0 >= x1 || y0 >= y1 {
                return;
            }

            for py in y0..y1 {
                for px in x0..x1 {
                    let ux = px as f32 + 0.5;
                    let uy = py as f32 + 0.5;
                    let sd = Self::rounded_rect_sdf(ux, uy, &shadow_rect, &rad);

                    let coverage = if use_blur {
                        Self::shadow_coverage(sd, blur)
                    } else {
                        Self::sdf_to_coverage(sd)
                    };

                    if coverage > 0.0 {
                        self.put_pixel_aa(px, py, color, coverage);
                    }
                }
            }
        }
    }

    /// Compute shadow coverage with smooth Gaussian-like falloff.
    ///
    /// The transition spans `2× blur` pixels centered on the shape contour,
    /// producing a smooth drop-shadow edge that matches CSS `box-shadow`.
    ///
    /// * `sd` — signed distance from the shape contour (negative = inside)
    /// * `blur` — blur radius in pixels
    ///
    /// | sd position | raw t | coverage |
    /// |------------|-------|----------|
    /// | `-blur` (deep inside) | 1.0 | 1.0 |
    /// | `0` (at edge)        | 0.5 | 0.5 |
    /// | `+blur` (outside)    | 0.0 | 0.0 |
    fn shadow_coverage(sd: f32, blur: f32) -> f32 {
        debug_assert!(blur > 0.5, "shadow_coverage called with small blur");
        // Linear falloff from sd=-blur (full) to sd=+blur (none)
        let t = ((blur - sd) / (2.0 * blur)).clamp(0.0, 1.0);
        // smoothstep: Hermite interpolation for Gaussian-like falloff
        t * t * (3.0 - 2.0 * t)
    }
}

#[cfg(test)]
mod tests {
    use super::super::core::RenderTarget;
    use crate::graphics::{Color, Radius, Rect};

    fn make_rt(w: i32, h: i32) -> RenderTarget {
        let mut rt = RenderTarget::new();
        rt.initialize(w, h);
        rt
    }

    fn unpack(r: &RenderTarget, x: i32, y: i32) -> (u8, u8, u8, u8) {
        let p = r.pixels()[(y * r.width() + x) as usize];
        (
            (p & 0xFF) as u8,         // B
            ((p >> 8) & 0xFF) as u8,  // G
            ((p >> 16) & 0xFF) as u8, // R
            ((p >> 24) & 0xFF) as u8, // A
        )
    }

    #[test]
    fn box_shadow_invisible_with_transparent_color() {
        let mut rt = make_rt(100, 100);
        rt.draw_box_shadow(
            Rect::new(10.0, 10.0, 50.0, 50.0),
            4.0,
            0.0,
            2.0,
            Color::transparent(),
            Some(Radius::uniform(4.0)),
        );
        // Everything should still be transparent
        for y in 0..100 {
            for x in 0..100 {
                let (_, _, _, a) = unpack(&rt, x, y);
                assert_eq!(a, 0, "pixel ({},{}) should be transparent", x, y);
            }
        }
    }

    #[test]
    fn box_shadow_zero_blur_is_sharp() {
        let mut rt = make_rt(100, 100);
        let shadow_color = Color::from_rgba(0, 0, 0, 128);
        rt.draw_box_shadow(
            Rect::new(20.0, 20.0, 30.0, 30.0),
            0.0,
            0.0,
            0.0,
            shadow_color,
            None,
        );
        // Center of shadow rect should have shadow with ~50% alpha
        let (_, _, _, a) = unpack(&rt, 35, 35);
        assert!(a > 40, "center should have visible shadow, got a={}", a);
    }

    #[test]
    fn box_shadow_offset_shifts_shadow() {
        let mut rt = make_rt(100, 100);
        let shadow_color = Color::from_rgba(0, 0, 0, 255);
        rt.draw_box_shadow(
            Rect::new(20.0, 20.0, 30.0, 30.0),
            2.0,
            5.0,
            10.0,
            shadow_color,
            None,
        );
        // Below the original rect (at y=35) should have shadow due to +10 offset
        let (_, _, _, a_inside_offset) = unpack(&rt, 35, 35);
        // At the original position (no offset) should have less shadow
        let (_, _, _, a_inside_no_offset) = unpack(&rt, 35, 25);
        assert!(
            a_inside_offset > 0,
            "offset area should have shadow, got a={}",
            a_inside_offset
        );
        // The shadow should be more visible at the offset position
        assert!(
            a_inside_offset >= a_inside_no_offset || a_inside_no_offset == 0,
            "shadow should be at least as strong at offset position"
        );
    }

    #[test]
    fn box_shadow_rounded_corners() {
        let mut rt = make_rt(80, 80);
        let shadow_color = Color::from_rgba(0, 0, 0, 200);
        rt.draw_box_shadow(
            Rect::new(20.0, 20.0, 40.0, 40.0),
            3.0,
            0.0,
            2.0,
            shadow_color,
            Some(Radius::uniform(8.0)),
        );
        // Center should be fully shadowed
        let (_, _, _, a) = unpack(&rt, 40, 41);
        assert!(a > 100, "center should be shadowed, got a={}", a);
    }

    #[test]
    fn box_shadow_does_not_leak_outside_clip() {
        let mut rt = make_rt(50, 50);
        rt.push_clip_rect(Rect::new(10.0, 10.0, 20.0, 20.0));
        rt.draw_box_shadow(
            Rect::new(0.0, 0.0, 100.0, 100.0),
            10.0,
            0.0,
            0.0,
            Color::from_rgba(0, 0, 0, 255),
            None,
        );
        rt.pop_clip_rect();
        // Outside the clip rect should remain transparent
        let (_, _, _, a_outside) = unpack(&rt, 5, 5);
        assert_eq!(a_outside, 0, "outside clip should be transparent");
        // Inside clip rect should have shadow
        let (_, _, _, a_inside) = unpack(&rt, 15, 15);
        assert!(a_inside > 0, "inside clip should have shadow");
    }

    #[test]
    fn box_shadow_large_blur_spreads_wide() {
        let mut rt = make_rt(120, 120);
        // Shadow rect: (42, 42, 20, 20) with blur=20
        rt.draw_box_shadow(
            Rect::new(40.0, 40.0, 20.0, 20.0),
            20.0,
            2.0,
            2.0,
            Color::from_rgba(0, 0, 0, 60),
            Some(Radius::uniform(4.0)),
        );
        // At the shadow rect top-left corner (sd=0) — should be ~50% coverage
        let (_, _, _, a_edge) = unpack(&rt, 42, 42);
        assert!(
            a_edge > 10,
            "at edge should have visible shadow, got a={}",
            a_edge
        );
        // Near the center of the shadow rect (deep inside) — should be strongest
        let (_, _, _, a_center) = unpack(&rt, 52, 52);
        assert!(
            a_center >= a_edge,
            "shadow strongest near center ({} vs {})",
            a_center,
            a_edge
        );
        // Outside the influence zone should have no shadow
        let (_, _, _, a_outside) = unpack(&rt, 5, 5);
        assert_eq!(a_outside, 0, "far outside should have no shadow");
    }

    /// Test that multiple overlapping layers produce a cumulative effect.
    #[test]
    fn box_shadow_multilayer_cumulative() {
        let mut rt_one = make_rt(100, 100);
        let mut rt_two = make_rt(100, 100);

        // One layer of alpha=80
        rt_one.draw_box_shadow(
            Rect::new(30.0, 30.0, 40.0, 40.0),
            4.0,
            2.0,
            2.0,
            Color::from_rgba(0, 0, 0, 80),
            Some(Radius::uniform(4.0)),
        );

        // Two layers of alpha=80 each (same params)
        rt_two.draw_box_shadow(
            Rect::new(30.0, 30.0, 40.0, 40.0),
            4.0,
            2.0,
            2.0,
            Color::from_rgba(0, 0, 0, 80),
            Some(Radius::uniform(4.0)),
        );
        rt_two.draw_box_shadow(
            Rect::new(30.0, 30.0, 40.0, 40.0),
            4.0,
            2.0,
            2.0,
            Color::from_rgba(0, 0, 0, 80),
            Some(Radius::uniform(4.0)),
        );

        let (_, _, _, a_one) = unpack(&rt_one, 50, 50);
        let (_, _, _, a_two) = unpack(&rt_two, 50, 50);
        // Two layers composited should be stronger than one layer alone
        assert!(
            a_two > a_one,
            "two layers should be stronger than one ({} vs {})",
            a_two,
            a_one
        );
    }
}
