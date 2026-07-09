//! Shared dirty-region and present-damage geometry contracts.

use crate::core::geometry::Rect;

const DIRTY_MERGE_THRESHOLD: usize = 16;

/// Dirty region tracking for incremental rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct DirtyRegion {
    pub rects: Vec<Rect>,
    pub full_frame: bool,
    pub clear_required: bool,
}

impl DirtyRegion {
    pub fn full() -> Self {
        Self {
            rects: Vec::new(),
            full_frame: true,
            clear_required: true,
        }
    }

    pub fn empty() -> Self {
        Self {
            rects: Vec::new(),
            full_frame: false,
            clear_required: false,
        }
    }

    pub fn area(rect: Rect) -> Self {
        Self {
            rects: if rect.w > 0.0 && rect.h > 0.0 {
                vec![rect]
            } else {
                Vec::new()
            },
            full_frame: false,
            clear_required: true,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::empty();
    }

    pub fn is_empty(&self) -> bool {
        !self.full_frame && self.rects.is_empty() && !self.clear_required
    }

    pub fn add_rect(&mut self, rect: Rect) {
        if rect.w <= 0.0 || rect.h <= 0.0 || self.full_frame {
            return;
        }
        self.clear_required = true;
        if self.rects.len() >= DIRTY_MERGE_THRESHOLD - 1 {
            let bounds = self.bounds().union(&rect);
            self.rects.clear();
            self.rects.push(bounds);
        } else {
            self.rects.push(rect);
        }
    }

    pub fn bounds(&self) -> Rect {
        if self.rects.is_empty() {
            return Rect::zero();
        }
        let mut bounds = self.rects[0];
        for &rect in &self.rects[1..] {
            bounds = bounds.union(&rect);
        }
        bounds
    }

    /// 绘制/清屏用脏区：多块 dirty 时升为并集 AABB。
    ///
    /// `begin_frame` 的 clip 已是并集；若仍按离散 rect 清屏与剪枝，
    /// 父节点背景会画进中间空隙而子节点不重绘（悬停 + 定时器双脏区时侧栏项消失）。
    pub fn for_paint_clear(&self) -> DirtyRegion {
        if self.full_frame || self.rects.len() <= 1 {
            return self.clone();
        }
        let bounds = self.bounds();
        if bounds.w <= 0.0 || bounds.h <= 0.0 {
            return DirtyRegion::empty();
        }
        DirtyRegion::area(bounds)
    }

    pub fn intersects(&self, rect: Rect) -> bool {
        if self.full_frame {
            return true;
        }
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return false;
        }
        self.rects
            .iter()
            .any(|&dirty| dirty.intersect(&rect).is_some())
    }

    pub fn rects(&self) -> &[Rect] {
        &self.rects
    }
}

impl Default for DirtyRegion {
    fn default() -> Self {
        Self::empty()
    }
}

/// Rendering damage region.
#[derive(Debug, Clone, PartialEq)]
pub struct DamageRegion {
    pub full: bool,
    pub rects: Vec<Rect>,
}

impl DamageRegion {
    pub fn full() -> Self {
        Self {
            full: true,
            rects: Vec::new(),
        }
    }

    pub fn partial(rects: Vec<Rect>) -> Self {
        Self { full: false, rects }
    }

    pub fn from_rect(rect: Rect) -> Self {
        if rect.w <= 0.0 || rect.h <= 0.0 {
            Self::full()
        } else {
            Self::partial(vec![rect])
        }
    }

    pub fn bounds(&self) -> Option<Rect> {
        if self.full || self.rects.is_empty() {
            return None;
        }
        let mut bounds = self.rects[0];
        for rect in &self.rects[1..] {
            bounds = bounds.union(rect);
        }
        Some(bounds)
    }

    pub fn to_present_damage(&self) -> PresentDamage {
        if self.full || self.rects.is_empty() {
            PresentDamage::Full
        } else {
            let tuples: Vec<(i32, i32, i32, i32)> = self
                .rects
                .iter()
                .filter(|rect| rect.w > 0.0 && rect.h > 0.0)
                .map(|rect| (rect.x as i32, rect.y as i32, rect.w as i32, rect.h as i32))
                .collect();
            if tuples.is_empty() {
                PresentDamage::Full
            } else {
                PresentDamage::Partial(tuples)
            }
        }
    }
}

impl Default for DamageRegion {
    fn default() -> Self {
        Self::full()
    }
}

/// Screen damage submitted with a CPU present or GPU buffer swap.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PresentDamage {
    #[default]
    Full,
    Partial(Vec<(i32, i32, i32, i32)>),
}

impl PresentDamage {
    pub fn single(x: i32, y: i32, w: i32, h: i32) -> Self {
        if w <= 0 || h <= 0 {
            Self::Full
        } else {
            Self::Partial(vec![(x, y, w, h)])
        }
    }

    pub fn is_full(&self) -> bool {
        matches!(self, Self::Full)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn for_paint_clear_keeps_single_rect() {
        let region = DirtyRegion::area(Rect::new(10.0, 20.0, 30.0, 40.0));
        let paint = region.for_paint_clear();
        assert_eq!(paint.rects(), &[Rect::new(10.0, 20.0, 30.0, 40.0)]);
        assert!(!paint.full_frame);
    }

    #[test]
    fn for_paint_clear_unions_disjoint_rects() {
        let mut region = DirtyRegion::empty();
        region.add_rect(Rect::new(0.0, 0.0, 10.0, 10.0));
        region.add_rect(Rect::new(0.0, 90.0, 10.0, 10.0));
        let paint = region.for_paint_clear();
        assert_eq!(paint.rects().len(), 1);
        assert_eq!(paint.rects()[0], Rect::new(0.0, 0.0, 10.0, 100.0));
        // 并集覆盖中间空隙，子节点剪枝与清屏一致
        assert!(paint.intersects(Rect::new(0.0, 40.0, 10.0, 10.0)));
    }
}
