use super::core::RenderTarget;
use crate::base::Rect;

// ════════════════════════════════════════════════════════════════════════════
// RenderTarget — 唯一在 pixels.rs 中实现的方法
//
// core.rs 包含了 put_pixel_aa/put_pixel_raw/premul/apply_opacity 等基础
// 像素操作。此文件只放 core.rs 中没有的独特实现。
// ════════════════════════════════════════════════════════════════════════════

impl RenderTarget {
    /// Scroll (shift) pixel content within `viewport` by `dx` and `dy` pixels.
    /// dx>0 = 右滚（内容左移），dy>0 = 下滚（内容上移）。
    ///
    /// 支持水平和垂直双向滚动，先垂直后水平移位。
    pub fn scroll_region(&mut self, viewport: Rect, dx: f32, dy: f32) {
        let vx = viewport.x as i32;
        let vy = viewport.y as i32;
        let vw = viewport.w as i32;
        let vh = viewport.h as i32;
        let stride = self.width;
        let ddx = dx.round() as i32;
        let ddy = dy.round() as i32;

        if (ddx == 0 && ddy == 0) || vw <= 0 || vh <= 0 { return; }

        // 垂直移位
        if ddy != 0 && ddy.abs() < vh {
            if ddy > 0 {
                for y in vy..vy + vh - ddy {
                    let dst = (y * stride + vx) as usize;
                    let src = ((y + ddy) * stride + vx) as usize;
                    self.pixels.copy_within(src..src + vw as usize, dst);
                }
            } else {
                let abs_d = -ddy;
                for y in (vy + abs_d..vy + vh).rev() {
                    let dst = (y * stride + vx) as usize;
                    let src = ((y - abs_d) * stride + vx) as usize;
                    self.pixels.copy_within(src..src + vw as usize, dst);
                }
            }
        }

        // 水平移位
        if ddx != 0 && ddx.abs() < vw {
            if ddx > 0 {
                for y in vy..vy + vh {
                    let base = (y * stride) as usize;
                    let dst = base + vx as usize;
                    let src = base + (vx + ddx) as usize;
                    let count = (vw - ddx) as usize;
                    self.pixels.copy_within(src..src + count, dst);
                }
            } else {
                let abs_d = -ddx;
                for y in vy..vy + vh {
                    let base = (y * stride) as usize;
                    let src = base + vx as usize;
                    let dst = base + (vx + abs_d) as usize;
                    let count = (vw - abs_d) as usize;
                    self.pixels.copy_within(src..src + count, dst);
                }
            }
        }
    }
}
