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
