// 定义 UI System 拥有的闭合文本水平对齐值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    // 文本沿内容框左缘对齐，并可显式覆盖继承值。
    #[default]
    Left,
    // 文本沿内容框右缘对齐。
    Right,
    // 文本在内容框内水平居中。
    Center,
    // 段落非末行通过扩展词间空白占满内容框。
    Justify,
}

// 提供到 draw System 中性排版值的单向适配。
impl TextAlign {
    // 将 UI 语义转换为不引用 UI 类型的 draw 对齐值。
    pub fn to_draw(self) -> crate::draw::HAlign {
        // 闭合映射全部公开 UI 变体。
        match self {
            // 左对齐映射到 draw 左对齐。
            Self::Left => crate::draw::HAlign::Left,
            // 右对齐映射到 draw 右对齐。
            Self::Right => crate::draw::HAlign::Right,
            // 居中映射到 draw 居中。
            Self::Center => crate::draw::HAlign::Center,
            // 两端对齐映射到 draw 中性空白扩展算法。
            Self::Justify => crate::draw::HAlign::Justify,
        }
    }
}
