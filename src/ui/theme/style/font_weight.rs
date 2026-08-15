// 定义 UI System 拥有并保留精确数值的字体粗细契约。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FontWeight {
    // 保存文档允许的一百到九百整数值。
    value: u16,
}

// 提供受控构造、常用常量与字体面选择语义。
impl FontWeight {
    // 最细字重。
    pub const THIN: Self = Self { value: 100 };
    // 常规字重，同时对应 UIX normal。
    pub const NORMAL: Self = Self { value: 400 };
    // 中等字重。
    pub const MEDIUM: Self = Self { value: 500 };
    // 半粗字重，也是当前合成粗体面的选择边界。
    pub const SEMIBOLD: Self = Self { value: 600 };
    // 粗体字重，同时对应 UIX bold。
    pub const BOLD: Self = Self { value: 700 };
    // 最粗字重。
    pub const BLACK: Self = Self { value: 900 };

    // 从文档数值构造字体粗细，不对非法输入做隐式夹取。
    pub const fn from_numeric(value: u16) -> Option<Self> {
        // 只允许文档登记的闭区间。
        if value >= 100 && value <= 900 {
            // 保留精确整数值供样式合并与快照使用。
            Some(Self { value })
        } else {
            // 越界值由调用方显式处理。
            None
        }
    }

    // 返回未经量化的公开数值。
    pub const fn value(self) -> u16 {
        // 返回构造时已经验证的值。
        self.value
    }

    // 选择当前字体后端可提供的最接近字体面。
    pub(crate) const fn uses_bold_face(self) -> bool {
        // 六百及以上使用现有合成粗体面，其余使用常规面。
        self.value >= Self::SEMIBOLD.value
    }
}

// 未显式声明时采用文档默认 normal。
impl Default for FontWeight {
    // 返回常规字重。
    fn default() -> Self {
        // 复用公开 normal 常量。
        Self::NORMAL
    }
}

// 验证数值边界和显式默认值覆盖语义。
#[cfg(test)]
mod tests {
    // 引入字体粗细与统一样式契约。
    use super::{super::Style, FontWeight};

    // 文档闭区间必须完整保留且拒绝越界值。
    #[test]
    fn font_weight_preserves_supported_numeric_range() {
        // 最小值必须有效。
        assert_eq!(FontWeight::from_numeric(100), Some(FontWeight::THIN));
        // 区间内非百位步进值也必须精确保留。
        assert_eq!(FontWeight::from_numeric(550).map(FontWeight::value), Some(550));
        // 最大值必须有效。
        assert_eq!(FontWeight::from_numeric(900), Some(FontWeight::BLACK));
        // 下界外数值必须拒绝。
        assert!(FontWeight::from_numeric(99).is_none());
        // 上界外数值必须拒绝。
        assert!(FontWeight::from_numeric(901).is_none());
    }

    // 显式 normal 必须覆盖继承的 bold。
    #[test]
    fn font_weight_explicit_normal_overrides_inherited_bold() {
        // 构造粗体基础样式。
        let base = Style::default().with_font_weight(FontWeight::BOLD);
        // 构造显式常规覆盖样式。
        let overlay = Style::default().with_font_weight(FontWeight::NORMAL);
        // 合并后必须保留显式默认值。
        let merged = base.apply(overlay);
        // 有效字重必须变为 normal。
        assert_eq!(merged.effective_font_weight(), FontWeight::NORMAL);
    }
}
