/// A 2D point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// 点的水平坐标。
    pub x: f32,
    /// 点的垂直坐标。
    pub y: f32,
}

impl Point {
    /// 使用水平和垂直坐标创建点。
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    /// 返回坐标原点。
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
    /// 尺寸的宽度。
    pub w: f32,
    /// 尺寸的高度。
    pub h: f32,
}

impl Size {
    /// Common UI-sized sentinels used by placeholder components.
    #[allow(non_upper_case_globals)]
    pub const Small: Self = Self { w: 32.0, h: 32.0 };
    /// 默认控件占位尺寸。
    #[allow(non_upper_case_globals)]
    pub const Default: Self = Self { w: 40.0, h: 40.0 };
    /// 中等控件占位尺寸。
    #[allow(non_upper_case_globals)]
    pub const Medium: Self = Self::Default;
    /// 大型控件占位尺寸。
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
    /// 返回宽高均为零的尺寸。
    pub const fn zero() -> Self {
        Self { w: 0.0, h: 0.0 }
    }
    /// 返回框架用于表示无限约束的最大尺寸。
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
    /// 允许的最小尺寸。
    pub min: Size,
    /// 允许的最大尺寸。
    pub max: Size,
    /// 可选的确定尺寸。
    pub definite: Option<Size>,
}

impl Constraints {
    /// 创建一组完整的尺寸约束。
    pub const fn new(min: Size, max: Size, definite: Option<Size>) -> Self {
        Self { min, max, definite }
    }

    /// 创建最小尺寸为零的宽松约束。
    pub const fn loose(max: Size) -> Self {
        Self {
            min: Size::zero(),
            max,
            definite: None,
        }
    }

    /// 创建不限制最大尺寸的约束。
    pub const fn unconstrained() -> Self {
        Self {
            min: Size::zero(),
            max: Size::infinite(),
            definite: None,
        }
    }

    /// 将尺寸限制在最小值和最大值之间。
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
    /// 矩形左上角的水平坐标。
    pub x: f32,
    /// 矩形左上角的垂直坐标。
    pub y: f32,
    /// 矩形宽度。
    pub w: f32,
    /// 矩形高度。
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
    /// 返回位置和尺寸均为零的矩形。
    pub const fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        }
    }

    /// 按边距向内收缩矩形，结果尺寸不会为负。
    pub fn inset(&self, p: EdgeInsets) -> Self {
        Self {
            x: self.x + p.left,
            y: self.y + p.top,
            w: (self.w - p.left - p.right).max(0.0),
            h: (self.h - p.top - p.bottom).max(0.0),
        }
    }

    /// 判断点是否位于矩形的闭合边界内。
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.x + self.w && p.y >= self.y && p.y <= self.y + self.h
    }

    /// 判断 `self` 是否完整覆盖 `other`（边界相切视为覆盖）。
    ///
    /// 这是 damage 收敛与脏区校验共用的唯一覆盖谓词；调用方不得自建副本。
    pub fn covers(&self, other: &Self) -> bool {
        self.x <= other.x
            && self.y <= other.y
            && self.x + self.w >= other.x + other.w
            && self.y + self.h >= other.y + other.h
    }

    /// 计算两个矩形的正面积交集。
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

    /// 返回能够包围两个矩形的最小矩形。
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
    /// 左边距。
    pub left: f32,
    /// 上边距。
    pub top: f32,
    /// 右边距。
    pub right: f32,
    /// 下边距。
    pub bottom: f32,
}

impl EdgeInsets {
    /// 分别指定四个方向的边距。
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }
    /// 创建四个方向数值相同的边距。
    pub const fn uniform(v: f32) -> Self {
        Self {
            left: v,
            top: v,
            right: v,
            bottom: v,
        }
    }
    /// 返回四个方向均为零的边距。
    pub const fn zero() -> Self {
        Self::uniform(0.0)
    }
    /// 返回左右边距之和。
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }
    /// 返回上下边距之和。
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
