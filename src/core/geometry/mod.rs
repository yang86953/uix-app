/// A 2D point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    pub const fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
    /// 两点中点。
    pub fn midpoint(a: Self, b: Self) -> Self {
        Self {
            x: (a.x + b.x) * 0.5,
            y: (a.y + b.y) * 0.5,
        }
    }
}

impl Default for Point {
    fn default() -> Self {
        Self::zero()
    }
}

/// A 2D size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub w: f32,
    pub h: f32,
}

impl Size {
    /// Common UI-sized sentinels used by placeholder components.
    #[allow(non_upper_case_globals)]
    pub const Small: Self = Self { w: 32.0, h: 32.0 };
    #[allow(non_upper_case_globals)]
    pub const Default: Self = Self { w: 40.0, h: 40.0 };
    #[allow(non_upper_case_globals)]
    pub const Medium: Self = Self::Default;
    #[allow(non_upper_case_globals)]
    pub const Large: Self = Self { w: 56.0, h: 56.0 };
    /// Create a new Size with NaN-safe clamping: NaN values become 0.0.
    /// Negative values are preserved (caller uses `.max(0.0)` if needed).
    pub fn new(w: f32, h: f32) -> Self {
        Self {
            w: if w.is_nan() { 0.0 } else { w },
            h: if h.is_nan() { 0.0 } else { h },
        }
    }
    pub const fn zero() -> Self {
        Self { w: 0.0, h: 0.0 }
    }
    pub const fn infinite() -> Self {
        Self {
            w: f32::MAX,
            h: f32::MAX,
        }
    }
}

impl Default for Size {
    fn default() -> Self {
        Self::zero()
    }
}

/// Measurement constraints for widget intrinsic sizing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Constraints {
    pub min: Size,
    pub max: Size,
    pub definite: Option<Size>,
}

impl Constraints {
    pub const fn new(min: Size, max: Size, definite: Option<Size>) -> Self {
        Self { min, max, definite }
    }

    pub const fn loose(max: Size) -> Self {
        Self {
            min: Size::zero(),
            max,
            definite: None,
        }
    }

    pub const fn unconstrained() -> Self {
        Self {
            min: Size::zero(),
            max: Size::infinite(),
            definite: None,
        }
    }

    pub fn clamp(&self, size: Size) -> Size {
        Size::new(
            size.w.max(self.min.w).min(self.max.w),
            size.h.max(self.min.h).min(self.max.h),
        )
    }
}

impl Default for Constraints {
    fn default() -> Self {
        Self::unconstrained()
    }
}

/// A rectangle with position and size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    /// Create a new Rect with NaN-safe clamping: NaN values become 0.0.
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            x: if x.is_nan() { 0.0 } else { x },
            y: if y.is_nan() { 0.0 } else { y },
            w: if w.is_nan() { 0.0 } else { w },
            h: if h.is_nan() { 0.0 } else { h },
        }
    }
    pub const fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        }
    }

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
        if w > 0.0 && h > 0.0 {
            Some(Self { x, y, w, h })
        } else {
            None
        }
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

impl Default for Rect {
    fn default() -> Self {
        Self::zero()
    }
}

/// Edge insets (padding/margin).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeInsets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl EdgeInsets {
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }
    pub const fn uniform(v: f32) -> Self {
        Self {
            left: v,
            top: v,
            right: v,
            bottom: v,
        }
    }
    pub const fn zero() -> Self {
        Self::uniform(0.0)
    }
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

impl Default for EdgeInsets {
    fn default() -> Self {
        Self::zero()
    }
}

impl From<f32> for EdgeInsets {
    fn from(v: f32) -> Self {
        Self::uniform(v)
    }
}

impl From<(f32, f32, f32, f32)> for EdgeInsets {
    fn from((top, right, bottom, left): (f32, f32, f32, f32)) -> Self {
        Self::new(left, top, right, bottom)
    }
}
