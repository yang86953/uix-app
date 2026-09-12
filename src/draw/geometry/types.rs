use crate::core::{Point, Rect};

/// Corner radii.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Radius {
    /// 左上角半径。
    pub tl: f32,
    /// 右上角半径。
    pub tr: f32,
    /// 右下角半径。
    pub br: f32,
    /// 左下角半径。
    pub bl: f32,
}

impl Radius {
    /// 创建四角半径相同的圆角。
    pub const fn uniform(r: f32) -> Self {
        Self {
            tl: r,
            tr: r,
            br: r,
            bl: r,
        }
    }
    /// 创建不带圆角的半径值。
    pub const fn zero() -> Self {
        Self::uniform(0.0)
    }
    /// 按 CSS 式相邻和规则归一化：任一相邻半径之和超出对应边长时，
    /// 全部半径按同一比例缩小，保持圆角轮廓不相交。
    ///
    /// 这是 CPU 与 GPU 各绘制入口共用的唯一归一化规则；上层在生成
    /// 自行采样几何（如圆角周长折线）前应先调用本方法。
    pub fn normalized(self, width: f32, height: f32) -> Self {
        crate::draw::raster::rasterizer::core::normalize_corner_radius(width, height, self)
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
    /// 使用标准源透明度混合。
    Alpha,
    /// 将源颜色覆盖到目标颜色之上。
    SrcOver,
    /// 将源颜色与目标颜色相加。
    Additive,
}

/// Gradient direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientDirection {
    /// 从左向右渐变。
    Horizontal,
    /// 从上向下渐变。
    Vertical,
    /// 从左上向右下渐变。
    DiagonalTLBR,
    /// 从左下向右上渐变。
    DiagonalBLTR,
}

/// Text horizontal alignment.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum HAlign {
    #[default]
    /// 左对齐文本。
    Left,
    /// 水平居中文本。
    Center,
    /// 右对齐文本。
    Right,
    /// 扩展段落非末行的可折叠空白以实现两端对齐。
    Justify,
}

/// Text vertical alignment.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum VAlign {
    #[default]
    /// 顶部对齐文本。
    Top,
    /// 垂直居中文本。
    Middle,
    /// 底部对齐文本。
    Bottom,
    /// 按文本基线对齐。
    Baseline,
}

/// 2D affine transform (3x2 matrix).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// 按 `[a, b, tx, c, d, ty]` 存储的仿射矩阵。
    pub m: [f32; 6],
}

impl Transform {
    /// 返回单位变换。
    pub const fn identity() -> Self {
        Self {
            m: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        }
    }

    /// 创建二维平移变换。
    pub fn translate(tx: f32, ty: f32) -> Self {
        Self {
            m: [1.0, 0.0, tx, 0.0, 1.0, ty],
        }
    }

    /// 创建二维缩放变换。
    pub fn scale(sx: f32, sy: f32) -> Self {
        Self {
            m: [sx, 0.0, 0.0, 0.0, sy, 0.0],
        }
    }

    /// 创建二维旋转变换；正角度在屏幕坐标系中表现为顺时针旋转。
    pub fn rotate(angle_radians: f32) -> Self {
        // 同时计算正弦与余弦以保持同一参数的数值一致性。
        let (sin, cos) = angle_radians.sin_cos();
        // 按当前 3x2 行主序契约写入旋转基向量。
        Self {
            // 屏幕 Y 轴向下，因此标准正角度矩阵呈现顺时针视觉结果。
            m: [cos, -sin, 0.0, sin, cos, 0.0],
        }
    }

    /// 创建二维倾斜变换，两个参数分别控制 X 轴与 Y 轴倾斜角。
    pub fn skew(x_angle_radians: f32, y_angle_radians: f32) -> Self {
        // 把角度转换为仿射矩阵需要的切线系数。
        let x_tangent = x_angle_radians.tan();
        // 独立计算 Y 轴倾斜系数，允许只倾斜单轴。
        let y_tangent = y_angle_radians.tan();
        // 写入同时支持 skewX 与 skewY 的二维线性部分。
        Self {
            // X 输出读取 Y 值，Y 输出读取 X 值。
            m: [1.0, x_tangent, 0.0, y_tangent, 1.0, 0.0],
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

    /// 判断变换是否为单位变换。
    pub fn is_identity(self) -> bool {
        self == Self::identity()
    }

    /// 将变换应用到一个点。
    pub fn transform_point(self, point: Point) -> Point {
        let [a, b, tx, c, d, ty] = self.m;
        Point::new(
            a * point.x + b * point.y + tx,
            c * point.x + d * point.y + ty,
        )
    }

    /// 返回变换后四个角的轴对齐包围矩形。
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

    /// 计算逆变换，不可逆或非有限矩阵返回空值。
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
    /// 文本布局允许的最大宽度。
    pub max_width: f32,
    /// 文本布局允许的最大高度。
    pub max_height: f32,
    /// 文本行高，零值表示使用字体默认行高。
    pub line_height: f32,
    /// 是否允许自动换行。
    pub word_wrap: bool,
    /// 水平对齐方式。
    pub h_align: HAlign,
    /// 垂直对齐方式。
    pub v_align: VAlign,
    /// 字体大小。
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
