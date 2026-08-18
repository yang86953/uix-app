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

// 验证行高构造、解析与样式合并语义。
#[cfg(test)]
mod tests {
    // 引入当前模块公开值和 Style。
    use super::{super::Style, LineHeight};

    // 验证倍率与像素值按各自单位解析。
    #[test]
    fn line_height_resolves_factor_and_pixels() {
        // 创建一倍半行高。
        let factor = LineHeight::factor(1.5).expect("正倍率必须有效");
        // 创建固定二十四像素行高。
        let pixels = LineHeight::pixels(24.0).expect("正像素必须有效");
        // 倍率必须乘以最终字号。
        assert_eq!(factor.resolve(16.0), 24.0);
        // 固定像素不随字号变化。
        assert_eq!(pixels.resolve(16.0), 24.0);
    }

    // 验证非法值拒绝且显式行高覆盖继承值。
    #[test]
    fn line_height_rejects_invalid_and_overrides_inherited_value() {
        // 零值不能进入行高契约。
        assert!(LineHeight::factor(0.0).is_none());
        // 非有限像素不能进入行高契约。
        assert!(LineHeight::pixels(f32::INFINITY).is_none());
        // 基础样式使用倍率。
        let base = Style::default()
            // 设置一倍半继承值。
            .with_line_height(LineHeight::factor(1.5).expect("倍率有效"));
        // 差异样式使用固定像素。
        let overlay = Style::default()
            // 设置二十像素覆盖值。
            .with_line_height(LineHeight::pixels(20.0).expect("像素有效"));
        // 显式差异值必须覆盖继承值。
        assert_eq!(base.apply(overlay).resolve_line_height(10.0), Some(20.0));
    }
}
