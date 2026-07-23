use crate::core::{Point, Rect};

/// Corner radii.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Radius {
    pub tl: f32,
    pub tr: f32,
    pub br: f32,
    pub bl: f32,
}

impl Radius {
    pub const fn uniform(r: f32) -> Self {
        Self {
            tl: r,
            tr: r,
            br: r,
            bl: r,
        }
    }
    pub const fn zero() -> Self {
        Self::uniform(0.0)
    }
}

impl Default for Radius {
    fn default() -> Self {
        Self::zero()
    }
}

/// Blend modes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    #[default]
    Alpha,
    SrcOver,
    Additive,
}

/// Gradient direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientDirection {
    Horizontal,
    Vertical,
    DiagonalTLBR,
    DiagonalBLTR,
}

/// Text horizontal alignment.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum HAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Text vertical alignment.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum VAlign {
    #[default]
    Top,
    Middle,
    Bottom,
    Baseline,
}

/// 2D affine transform (3x2 matrix).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub m: [f32; 6],
}

impl Transform {
    pub const fn identity() -> Self {
        Self {
            m: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        }
    }

    pub fn translate(tx: f32, ty: f32) -> Self {
        Self {
            m: [1.0, 0.0, tx, 0.0, 1.0, ty],
        }
    }

    pub fn scale(sx: f32, sy: f32) -> Self {
        Self {
            m: [sx, 0.0, 0.0, 0.0, sy, 0.0],
        }
    }

    /// Matrix product `self * rhs`: `rhs` is applied first, then `self`.
    pub fn concat(self, rhs: Self) -> Self {
        let [a, b, tx, c, d, ty] = self.m;
        let [ra, rb, rtx, rc, rd, rty] = rhs.m;
        Self {
            m: [
                a * ra + b * rc,
                a * rb + b * rd,
                a * rtx + b * rty + tx,
                c * ra + d * rc,
                c * rb + d * rd,
                c * rtx + d * rty + ty,
            ],
        }
    }

    pub fn is_identity(self) -> bool {
        self == Self::identity()
    }

    pub fn transform_point(self, point: Point) -> Point {
        let [a, b, tx, c, d, ty] = self.m;
        Point::new(
            a * point.x + b * point.y + tx,
            c * point.x + d * point.y + ty,
        )
    }

    pub fn transform_rect(self, rect: Rect) -> Rect {
        let [a, b, tx, c, d, ty] = self.m;
        if b == 0.0 && c == 0.0 {
            let x = if a >= 0.0 {
                a * rect.x + tx
            } else {
                a * (rect.x + rect.w) + tx
            };
            let y = if d >= 0.0 {
                d * rect.y + ty
            } else {
                d * (rect.y + rect.h) + ty
            };
            return Rect::new(x, y, a.abs() * rect.w, d.abs() * rect.h);
        }
        let top_left = self.transform_point(Point::new(rect.x, rect.y));
        let top_right = self.transform_point(Point::new(rect.x + rect.w, rect.y));
        let bottom_left = self.transform_point(Point::new(rect.x, rect.y + rect.h));
        let bottom_right = self.transform_point(Point::new(rect.x + rect.w, rect.y + rect.h));
        let min_x = top_left
            .x
            .min(top_right.x)
            .min(bottom_left.x)
            .min(bottom_right.x);
        let min_y = top_left
            .y
            .min(top_right.y)
            .min(bottom_left.y)
            .min(bottom_right.y);
        let max_x = top_left
            .x
            .max(top_right.x)
            .max(bottom_left.x)
            .max(bottom_right.x);
        let max_y = top_left
            .y
            .max(top_right.y)
            .max(bottom_left.y)
            .max(bottom_right.y);
        Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    pub fn inverse(self) -> Option<Self> {
        let [a, b, tx, c, d, ty] = self.m;
        let determinant = a * d - b * c;
        if !determinant.is_finite() || determinant.abs() <= f32::EPSILON {
            return None;
        }
        let inverse = 1.0 / determinant;
        Some(Self {
            m: [
                d * inverse,
                -b * inverse,
                (b * ty - d * tx) * inverse,
                -c * inverse,
                a * inverse,
                (c * tx - a * ty) * inverse,
            ],
        })
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::identity()
    }
}

/// Text layout options.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextLayoutOptions {
    pub max_width: f32,
    pub max_height: f32,
    pub line_height: f32,
    pub word_wrap: bool,
    pub h_align: HAlign,
    pub v_align: VAlign,
    pub font_size: f32,
}

impl Default for TextLayoutOptions {
    fn default() -> Self {
        Self {
            max_width: f32::MAX,
            max_height: 0.0,
            line_height: 0.0,
            word_wrap: true,
            h_align: HAlign::Left,
            v_align: VAlign::Top,
            font_size: 14.0,
        }
    }
}

/// Opaque handle for backend-managed resources (offscreen buffers, etc.).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ImageHandle(pub u32);

/// Opaque handle for font references.
/// Contains a font index for engine lookup (replacing pointer-identity matching).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FontHandle(pub u32);

impl FontHandle {
    /// Create a new font handle with the given index.
    pub const fn new(idx: u32) -> Self {
        Self(idx)
    }
}
