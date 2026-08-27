// 区分无单位字号倍率与固定逻辑像素。
#[derive(Debug, Clone, Copy, PartialEq)]
enum LineHeightUnit {
    // 值乘以最终字体尺寸。
    Factor,
    // 值直接表示逻辑像素。
    Pixels,
}

// 表示已经验证为正有限数值的文本行高。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineHeight {
    // 保存倍率或像素数值。
    value: f32,
    // 保存数值解释单位。
    unit: LineHeightUnit,
}

// 提供受控构造与最终字号解析。
impl LineHeight {
    // 创建无单位字号倍率。
    pub fn factor(value: f32) -> Option<Self> {
        // 委托统一正有限值验证。
        Self::new(value, LineHeightUnit::Factor)
    }

    // 创建固定逻辑像素行高。
    pub fn pixels(value: f32) -> Option<Self> {
        // 委托统一正有限值验证。
        Self::new(value, LineHeightUnit::Pixels)
    }

    // 只允许正有限值进入公开样式契约。
    fn new(value: f32, unit: LineHeightUnit) -> Option<Self> {
        // 拒绝零、负数、无穷与 NaN。
        if !value.is_finite() || value <= 0.0 {
            // 用空值表达无效构造，不偷偷回退默认行高。
            return None;
        }
        // 保存完成验证的值和单位。
        Some(Self { value, unit })
    }

    // 按最终字体尺寸解析逻辑像素行高。
    pub fn resolve(self, font_size: f32) -> f32 {
        // 字体尺寸异常时采用稳定正值避免传播非有限几何。
        let font_size = if font_size.is_finite() && font_size > 0.0 {
            // 保留合法最终字号。
            font_size
        } else {
            // 与文本组件默认字号量级一致。
            12.0
        };
        // 按单位解释已验证值。
        let resolved = match self.unit {
            // 倍率依赖最终字号。
            LineHeightUnit::Factor => self.value * font_size,
            // 像素值与字号无关。
            LineHeightUnit::Pixels => self.value,
        };
        // 极端有限输入相乘仍可能溢出，统一保护绘制几何。
        if resolved.is_finite() && resolved > 0.0 {
            // 返回有效逻辑像素。
            resolved
        } else {
            // 溢出时回到稳定 normal 行高。
            font_size * 1.5
        }
    }
}
