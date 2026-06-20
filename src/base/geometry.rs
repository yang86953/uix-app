/// A 2D point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self { Self { x, y } }
    pub const fn zero() -> Self { Self { x: 0.0, y: 0.0 } }
    /// 两点中点。
    pub fn midpoint(a: Self, b: Self) -> Self {
        Self { x: (a.x + b.x) * 0.5, y: (a.y + b.y) * 0.5 }
    }
}

impl Default for Point { fn default() -> Self { Self::zero() } }

/// A 2D size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub w: f32,
    pub h: f32,
}

impl Size {
    pub const fn new(w: f32, h: f32) -> Self { Self { w, h } }
    pub const fn zero() -> Self { Self { w: 0.0, h: 0.0 } }
    pub const fn infinite() -> Self { Self { w: f32::MAX, h: f32::MAX } }
}

impl Default for Size { fn default() -> Self { Self::zero() } }

/// A rectangle with position and size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self { Self { x, y, w, h } }
    pub const fn zero() -> Self { Self { x: 0.0, y: 0.0, w: 0.0, h: 0.0 } }

    pub fn inset(&self, p: EdgeInsets) -> Self {
        Self {
            x: self.x + p.left,
            y: self.y + p.top,
            w: (self.w - p.left - p.right).max(0.0),
            h: (self.h - p.top - p.bottom).max(0.0),
        }
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.x + self.w && p.y >= self.y && p.y <= self.y + self.h
    }

    pub fn intersect(&self, other: &Self) -> Option<Self> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let w = (self.x + self.w).min(other.x + other.w) - x;
        let h = (self.y + self.h).min(other.y + other.h) - y;
        if w > 0.0 && h > 0.0 { Some(Self { x, y, w, h }) } else { None }
    }

    pub fn union(&self, other: &Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Self {
            x,
            y,
            w: (self.x + self.w).max(other.x + other.w) - x,
            h: (self.y + self.h).max(other.y + other.h) - y,
        }
    }
}

impl Default for Rect { fn default() -> Self { Self::zero() } }

/// Edge insets (padding/margin).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeInsets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl EdgeInsets {
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self { Self { left, top, right, bottom } }
    pub const fn uniform(v: f32) -> Self { Self { left: v, top: v, right: v, bottom: v } }
    pub const fn zero() -> Self { Self::uniform(0.0) }
    pub fn horizontal(&self) -> f32 { self.left + self.right }
    pub fn vertical(&self) -> f32 { self.top + self.bottom }
}

impl Default for EdgeInsets { fn default() -> Self { Self::zero() } }

#[cfg(test)]
mod tests {
    use super::*;

    // ── Rect ──

    #[test]
    fn rect_contains_point_inside() {
        let r = Rect::new(10.0, 10.0, 100.0, 50.0);
        assert!(r.contains(Point::new(10.0, 10.0)));
        assert!(r.contains(Point::new(110.0, 60.0)));
        assert!(r.contains(Point::new(50.0, 30.0)));
    }

    #[test]
    fn rect_contains_point_outside() {
        let r = Rect::new(10.0, 10.0, 100.0, 50.0);
        assert!(!r.contains(Point::new(9.0, 10.0)));
        assert!(!r.contains(Point::new(10.0, 9.0)));
        assert!(!r.contains(Point::new(111.0, 30.0)));
        assert!(!r.contains(Point::new(50.0, 61.0)));
    }

    #[test]
    fn rect_intersect_overlapping() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(50.0, 50.0, 100.0, 100.0);
        let c = a.intersect(&b).unwrap();
        assert_eq!(c, Rect::new(50.0, 50.0, 50.0, 50.0));
    }

    #[test]
    fn rect_intersect_non_overlapping() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(200.0, 200.0, 100.0, 100.0);
        assert!(a.intersect(&b).is_none());
    }

    #[test]
    fn rect_intersect_contained() {
        let outer = Rect::new(0.0, 0.0, 100.0, 100.0);
        let inner = Rect::new(10.0, 10.0, 80.0, 80.0);
        assert_eq!(outer.intersect(&inner), Some(inner));
    }

    #[test]
    fn rect_union_separate() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(200.0, 50.0, 100.0, 100.0);
        let u = a.union(&b);
        assert_eq!(u, Rect::new(0.0, 0.0, 300.0, 150.0));
    }

    #[test]
    fn rect_union_overlapping() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(50.0, 50.0, 100.0, 100.0);
        let u = a.union(&b);
        assert_eq!(u, Rect::new(0.0, 0.0, 150.0, 150.0));
    }

    // ── Point & Size ──

    #[test]
    fn point_zero() {
        assert_eq!(Point::zero(), Point::new(0.0, 0.0));
    }

    #[test]
    fn size_infinite() {
        let s = Size::infinite();
        assert_eq!(s.w, f32::MAX);
        assert_eq!(s.h, f32::MAX);
    }
}
