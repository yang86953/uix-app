//! 帧命令模型的几何与载荷值类型 — encoder 子模块。
//!
//! 纯值类型：整数/采样矩形、圆角、描边、透明度、字形与图片载荷。
//! 由命令契约（[`super::commands`]）与执行路径（[`super::pixels`]）共享。

use crate::core::Rect;
use crate::draw::Color;
use crate::draw::geometry::types::Radius;
use std::sync::Arc;

use super::error::FrameEncoderError;
use super::pixels::pixel_len;
/// Integer pixel rectangle used by the API-neutral frame command model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameRect {
    /// 矩形左上角的水平像素坐标。
    pub x: i32,
    /// 矩形左上角的垂直像素坐标。
    pub y: i32,
    /// 矩形的像素宽度。
    pub width: i32,
    /// 矩形的像素高度。
    pub height: i32,
}

impl FrameRect {
    /// 创建整数像素矩形。
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub(crate) fn is_empty(self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    pub(crate) fn intersection(self, other: Self) -> Option<Self> {
        let left = i64::from(self.x).max(i64::from(other.x));
        let top = i64::from(self.y).max(i64::from(other.y));
        let right = (i64::from(self.x) + i64::from(self.width))
            .min(i64::from(other.x) + i64::from(other.width));
        let bottom = (i64::from(self.y) + i64::from(self.height))
            .min(i64::from(other.y) + i64::from(other.height));
        if left >= right || top >= bottom {
            return None;
        }
        Some(Self::new(
            left as i32,
            top as i32,
            (right - left) as i32,
            (bottom - top) as i32,
        ))
    }

    pub(crate) fn translated(self, dx: i32, dy: i32) -> Option<Self> {
        Some(Self::new(
            self.x.checked_add(dx)?,
            self.y.checked_add(dy)?,
            self.width,
            self.height,
        ))
    }

    pub(crate) fn is_within(self, width: i32, height: i32) -> bool {
        if self.is_empty() || self.x < 0 || self.y < 0 {
            return false;
        }
        i64::from(self.x) + i64::from(self.width) <= i64::from(width)
            && i64::from(self.y) + i64::from(self.height) <= i64::from(height)
    }
}

/// 采样目标矩形：允许亚像素落点与非 1:1 缩放，用 `f32` bits 保持命令模型 `Eq`。
///
/// 整数源 crop 仍用 [`FrameRect`]；仅 destination 进入本类型，供 GPU 纹理四边形采样。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameSampledRect {
    x_bits: u32,
    y_bits: u32,
    width_bits: u32,
    height_bits: u32,
}

impl FrameSampledRect {
    /// 从整数像素矩形创建采样目标。
    pub fn from_integer(rect: FrameRect) -> Self {
        Self::from_parts(
            rect.x as f32,
            rect.y as f32,
            rect.width as f32,
            rect.height as f32,
        )
        .unwrap_or_else(|_| Self {
            x_bits: 0.0f32.to_bits(),
            y_bits: 0.0f32.to_bits(),
            width_bits: 0.0f32.to_bits(),
            height_bits: 0.0f32.to_bits(),
        })
    }

    /// 从有限坐标和正尺寸创建采样目标。
    pub fn from_parts(x: f32, y: f32, width: f32, height: f32) -> Result<Self, FrameEncoderError> {
        if !x.is_finite()
            || !y.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
        {
            return Err(FrameEncoderError::InvalidSampledRect);
        }
        Ok(Self {
            x_bits: x.to_bits(),
            y_bits: y.to_bits(),
            width_bits: width.to_bits(),
            height_bits: height.to_bits(),
        })
    }

    /// 返回水平坐标。
    pub const fn x(self) -> f32 {
        f32::from_bits(self.x_bits)
    }

    /// 返回垂直坐标。
    pub const fn y(self) -> f32 {
        f32::from_bits(self.y_bits)
    }

    /// 返回采样宽度。
    pub const fn width(self) -> f32 {
        f32::from_bits(self.width_bits)
    }

    /// 返回采样高度。
    pub const fn height(self) -> f32 {
        f32::from_bits(self.height_bits)
    }

    /// 判断采样目标是否没有任何像素贡献。
    pub const fn is_empty(self) -> bool {
        // 构造器只可能通过整数兼容入口产生规范化零尺寸。
        self.width() <= 0.0 || self.height() <= 0.0
    }

    /// 当坐标均为整数像素时返回 [`FrameRect`]，供 splice / 精确裁剪使用。
    pub fn as_integer(self) -> Option<FrameRect> {
        let x = self.x();
        let y = self.y();
        let width = self.width();
        let height = self.height();
        if x.fract() != 0.0
            || y.fract() != 0.0
            || width.fract() != 0.0
            || height.fract() != 0.0
            || width <= 0.0
            || height <= 0.0
        {
            return None;
        }
        Some(FrameRect::new(
            x as i32,
            y as i32,
            width as i32,
            height as i32,
        ))
    }

    /// 转换为逻辑浮点矩形。
    pub fn to_rect(self) -> Rect {
        Rect::new(self.x(), self.y(), self.width(), self.height())
    }
}

/// 帧命令使用的已验证圆角半径。
///
/// 构造时排除 NaN、无穷大与负值，并把 `-0.0` 规范化为 `0.0`，因此该类型
/// 可以安全保持命令模型原有的 `Eq` 契约。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameRadius {
    value: Radius,
}

impl FrameRadius {
    pub(crate) const fn zero() -> Self {
        Self {
            value: Radius {
                tl: 0.0,
                tr: 0.0,
                br: 0.0,
                bl: 0.0,
            },
        }
    }

    /// 验证并保存四个角的非负有限半径。
    pub fn new(value: Radius) -> Result<Self, FrameEncoderError> {
        for (corner, radius) in [
            ("top-left", value.tl),
            ("top-right", value.tr),
            ("bottom-right", value.br),
            ("bottom-left", value.bl),
        ] {
            if !radius.is_finite() || radius < 0.0 {
                return Err(FrameEncoderError::InvalidRadius { corner });
            }
        }
        let canonical = |radius: f32| if radius == 0.0 { 0.0 } else { radius };
        Ok(Self {
            value: Radius {
                tl: canonical(value.tl),
                tr: canonical(value.tr),
                br: canonical(value.br),
                bl: canonical(value.bl),
            },
        })
    }

    /// 返回已验证的通用圆角半径。
    pub const fn to_radius(self) -> Radius {
        self.value
    }
}

impl Eq for FrameRadius {}

/// Validated finite positive stroke width retained by frame commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameStrokeWidth(u32);

impl FrameStrokeWidth {
    /// 验证并保存有限正描边宽度。
    pub fn new(value: f32) -> Result<Self, FrameEncoderError> {
        if !value.is_finite() || value <= 0.0 {
            return Err(FrameEncoderError::InvalidStrokeWidth);
        }
        Ok(Self(value.to_bits()))
    }

    /// 返回已验证的描边宽度。
    pub const fn value(self) -> f32 {
        f32::from_bits(self.0)
    }
}

/// One validated rectangle stroke retained by a batched frame operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameStrokeRect {
    pub(crate) rect: FrameRect,
    pub(crate) color: Color,
    pub(crate) radius: FrameRadius,
    pub(crate) line_width: FrameStrokeWidth,
}

impl FrameStrokeRect {
    /// 创建一条已验证的矩形描边载荷。
    pub const fn new(
        rect: FrameRect,
        color: Color,
        radius: FrameRadius,
        line_width: FrameStrokeWidth,
    ) -> Self {
        Self {
            rect,
            color,
            radius,
            line_width,
        }
    }

    /// 返回描边矩形。
    pub const fn rect(&self) -> FrameRect {
        self.rect
    }

    /// 返回描边颜色。
    pub const fn color(&self) -> Color {
        self.color
    }

    /// 返回描边圆角半径。
    pub const fn radius(&self) -> FrameRadius {
        self.radius
    }

    /// 返回描边线宽。
    pub const fn line_width(&self) -> FrameStrokeWidth {
        self.line_width
    }
}

/// Canonical post-composition opacity for a materialized Picture command.
///
/// Storing the normalized `f32` bits preserves the CPU rasterizer's exact
/// per-channel truncation while keeping the ordered command model `Eq`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameOpacity(u32);

impl FrameOpacity {
    /// 返回完全不透明的规范值。
    pub const fn opaque() -> Self {
        Self(1.0f32.to_bits())
    }

    /// 从画布透明度创建零到一之间的规范值。
    pub fn from_canvas(opacity: f32) -> Self {
        let opacity = if opacity.is_nan() {
            0.0
        } else {
            opacity.clamp(0.0, 1.0)
        };
        Self(if opacity == 0.0 {
            0.0f32.to_bits()
        } else {
            opacity.to_bits()
        })
    }

    /// 返回规范化透明度。
    pub const fn value(self) -> f32 {
        f32::from_bits(self.0)
    }

    /// 判断透明度是否可视为完全不透明。
    pub fn is_opaque(self) -> bool {
        self.value() >= 1.0 - 1e-6
    }

    /// 判断透明度是否完全透明。
    pub const fn is_transparent(self) -> bool {
        self.0 == 0.0f32.to_bits()
    }
}

/// One validated integer-positioned glyph coverage blit.
///
/// The coverage allocation is retained by the command stream, so a recorded
/// frame stays valid even when the font cache evicts the glyph before submit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameGlyphBlit {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) color: Color,
    pub(crate) coverage: Arc<[u8]>,
}

impl FrameGlyphBlit {
    /// 验证覆盖率缓冲与尺寸后创建字形复制载荷。
    pub fn new(
        x: i32,
        y: i32,
        coverage: Arc<[u8]>,
        width: usize,
        height: usize,
        color: Color,
    ) -> Result<Self, FrameEncoderError> {
        let width_u32 =
            u32::try_from(width).map_err(|_| FrameEncoderError::InvalidGlyphCoverage {
                width,
                height,
                actual: coverage.len(),
            })?;
        let height_u32 =
            u32::try_from(height).map_err(|_| FrameEncoderError::InvalidGlyphCoverage {
                width,
                height,
                actual: coverage.len(),
            })?;
        let required = width.checked_mul(height).filter(|required| *required > 0);
        if required.is_none_or(|required| coverage.len() < required) {
            return Err(FrameEncoderError::InvalidGlyphCoverage {
                width,
                height,
                actual: coverage.len(),
            });
        }
        Ok(Self {
            x,
            y,
            width: width_u32,
            height: height_u32,
            color,
            coverage,
        })
    }

    /// 返回字形左上角的水平像素坐标。
    pub const fn x(&self) -> i32 {
        self.x
    }

    /// 返回字形左上角的垂直像素坐标。
    pub const fn y(&self) -> i32 {
        self.y
    }

    /// 返回字形覆盖率宽度。
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// 返回字形覆盖率高度。
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// 返回字形颜色。
    pub const fn color(&self) -> Color {
        self.color
    }

    /// 借用共享覆盖率缓冲。
    pub fn coverage(&self) -> &Arc<[u8]> {
        &self.coverage
    }
}

/// 一条保留字体轮廓边列表的整数定位字形命令。
#[derive(Debug, Clone)]
pub struct FrameGlyphOutline {
    // 保存 surface 空间水平位置。
    pub(crate) x: i32,
    // 保存 surface 空间垂直位置。
    pub(crate) y: i32,
    // 保存目标字形宽度。
    pub(crate) width: u32,
    // 保存目标字形高度。
    pub(crate) height: u32,
    // 保存直通颜色。
    pub(crate) color: Color,
    // 保留字体轮廓边列表供 GPU MSDF 生成。
    pub(crate) edges: Arc<[f32]>,
}

// 按浮点位模式比较轮廓，保持 FrameEncoder 命令模型的确定性 Eq 契约。
impl PartialEq for FrameGlyphOutline {
    // 比较几何、颜色和每个轮廓浮点的原始位模式。
    fn eq(&self, other: &Self) -> bool {
        // 先比较固定大小字段，再比较轮廓长度和位模式。
        self.x == other.x
            && self.y == other.y
            && self.width == other.width
            && self.height == other.height
            && self.color == other.color
            && self.edges.len() == other.edges.len()
            && self
                .edges
                .iter()
                .zip(other.edges.iter())
                .all(|(left, right)| left.to_bits() == right.to_bits())
    }
}

// 位模式相等满足自反、对称和传递关系。
impl Eq for FrameGlyphOutline {}

// 提供经过验证的字形轮廓值构造与只读访问。
impl FrameGlyphOutline {
    // 验证尺寸与边列表后构造保留命令。
    pub(crate) fn new(
        // 接收 surface 水平位置。
        x: i32,
        // 接收 surface 垂直位置。
        y: i32,
        // 接收共享轮廓边列表。
        edges: Arc<[f32]>,
        // 接收目标宽度。
        width: usize,
        // 接收目标高度。
        height: usize,
        // 接收直通颜色。
        color: Color,
    ) -> Option<Self> {
        // 轮廓必须满足字体模块登记的固定边 ABI。
        if !crate::draw::resources::font::glyph_outline::is_outline_edges(edges.as_ref()) {
            // 拒绝异常轮廓载荷。
            return None;
        }
        // 把尺寸收窄到 RHI 字形 ABI。
        let (width, height) = (u32::try_from(width).ok()?, u32::try_from(height).ok()?);
        // 零尺寸字形不生成命令。
        if width == 0 || height == 0 {
            // 拒绝空目标。
            return None;
        }
        // 返回完整保留轮廓命令。
        Some(Self {
            // 保存水平位置。
            x,
            // 保存垂直位置。
            y,
            // 保存目标宽度。
            width,
            // 保存目标高度。
            height,
            // 保存直通颜色。
            color,
            // 保留共享轮廓分配。
            edges,
        })
    }

    /// 返回 surface 水平位置。
    pub const fn x(&self) -> i32 {
        self.x
    }
    /// 返回 surface 垂直位置。
    pub const fn y(&self) -> i32 {
        self.y
    }
    /// 返回目标字形宽度。
    pub const fn width(&self) -> u32 {
        self.width
    }
    /// 返回目标字形高度。
    pub const fn height(&self) -> u32 {
        self.height
    }
    /// 返回直通颜色。
    pub const fn color(&self) -> Color {
        self.color
    }
    /// 返回共享轮廓边列表。
    pub fn edges(&self) -> &Arc<[f32]> {
        &self.edges
    }
}

/// A self-contained premultiplied-AARRGGBB CPU image used to model a
/// Picture/offscreen result. This matches the software rasterizer's pixel
/// representation, so reference execution can be copied into a CPU Picture
/// without a lossy color conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameImage {
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) pixels: Arc<Vec<u32>>,
}

impl FrameImage {
    /// 验证像素数量后创建自包含帧图像。
    pub fn new(width: i32, height: i32, pixels: Vec<u32>) -> Result<Self, FrameEncoderError> {
        let expected = pixel_len(width, height)?;
        if pixels.len() != expected {
            return Err(FrameEncoderError::PixelCountMismatch {
                width,
                height,
                actual: pixels.len(),
            });
        }
        Ok(Self {
            width,
            height,
            pixels: Arc::new(pixels),
        })
    }

    /// 验证像素数量后接管共享不可变帧图像，不复制像素载荷。
    pub(crate) fn from_shared(
        width: i32,
        height: i32,
        pixels: Arc<Vec<u32>>,
    ) -> Result<Self, FrameEncoderError> {
        let expected = pixel_len(width, height)?;
        if pixels.len() != expected {
            return Err(FrameEncoderError::PixelCountMismatch {
                width,
                height,
                actual: pixels.len(),
            });
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// 创建由单一颜色填充的帧图像。
    pub fn solid(width: i32, height: i32, color: Color) -> Result<Self, FrameEncoderError> {
        Ok(Self {
            width,
            height,
            pixels: Arc::new(vec![color.premultiplied(); pixel_len(width, height)?]),
        })
    }

    /// 返回图像宽度。
    pub const fn width(&self) -> i32 {
        self.width
    }

    /// 返回图像高度。
    pub const fn height(&self) -> i32 {
        self.height
    }

    /// 借用预乘 AARRGGBB 像素。
    pub fn pixels(&self) -> &[u32] {
        self.pixels.as_slice()
    }
}
