//! 有界多色标线性渐变的共享值与局部采样契约。
use super::color::Color;

/// 一条线性渐变最多十六个色标，CPU 与全部原生后端采用相同上限。
pub const MAX_GRADIENT_STOPS: usize = 16;

/// 已定位色标；位置使用 0..=1 的渐变轴比例。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientStop {
    /// 色标位置；相同位置产生硬切换，后项在该位置生效。
    pub offset: f32,
    /// 非预乘颜色，按既有 Color 通道空间插值。
    pub color: Color,
}

impl GradientStop {
    /// 创建待验证色标；完整列表由 LinearGradient::new 验证。
    pub const fn new(offset: f32, color: Color) -> Self {
        Self { offset, color }
    }
}

/// 已验证、可复制的线性渐变；0° 向上、90° 向右、180° 向下。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearGradient {
    angle_degrees: f32,
    count: u8,
    stops: [GradientStop; MAX_GRADIENT_STOPS],
}

impl LinearGradient {
    /// 拒绝非有限角度、非 2..=16 色标、越界或倒序位置；不排序或截断输入。
    pub fn new(angle_degrees: f32, stops: &[GradientStop]) -> Option<Self> {
        if !angle_degrees.is_finite()
            || !(2..=MAX_GRADIENT_STOPS).contains(&stops.len())
            || stops
                .iter()
                .any(|s| !s.offset.is_finite() || !(0.0..=1.0).contains(&s.offset))
            || stops.windows(2).any(|w| w[0].offset > w[1].offset)
        {
            return None;
        }
        let mut data = [GradientStop::new(0.0, Color::TRANSPARENT); MAX_GRADIENT_STOPS];
        data[..stops.len()].copy_from_slice(stops);
        Some(Self {
            angle_degrees: angle_degrees.rem_euclid(360.0),
            count: stops.len() as u8,
            stops: data,
        })
    }

    /// 返回规范化为 [0,360) 的角度。
    pub fn angle_degrees(&self) -> f32 {
        self.angle_degrees
    }
    /// 返回声明顺序色标；未用容量不可见。
    pub fn stops(&self) -> &[GradientStop] {
        &self.stops[..usize::from(self.count)]
    }

    /// 在归一化轴上采样，端点外延伸端点色，相同位置由后项覆盖。
    pub fn sample(&self, t: f32) -> Color {
        let stops = self.stops();
        let mut left = stops[0];
        for right in &stops[1..] {
            if t < right.offset {
                return left.color.mix(
                    &right.color,
                    ((t - left.offset) / (right.offset - left.offset)).clamp(0.0, 1.0),
                );
            }
            left = *right;
        }
        left.color
    }

    // 局部单位矩形中的投影系数；CPU 与 GPU lowering 共用这一角度定义。
    pub(crate) fn axis(&self, width: f32, height: f32) -> Option<[f32; 2]> {
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return None;
        }
        // f64 中求投影避免巨大但有限逻辑尺寸相加溢出。
        let angle = f64::from(self.angle_degrees).to_radians();
        let x = f64::from(width) * angle.sin();
        let y = -f64::from(height) * angle.cos();
        let length = x.abs() + y.abs();
        Some([(x / length) as f32, (y / length) as f32])
    }
}
