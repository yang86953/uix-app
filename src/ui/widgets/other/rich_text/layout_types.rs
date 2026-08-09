//! 保存 RichText 内部共享的布局数据类型。

// 引入矩形与颜色类型。
use crate::core::Rect;
// 引入布局字形颜色。
use crate::draw::Color;

// 描述带样式与源字符索引的布局字形。
#[derive(Debug, Clone)]
pub(crate) struct LayoutGlyph {
    // 保存所属富文本 segment 索引。
    pub segment_idx: usize,
    // 保存完整逻辑源中的 Unicode 标量索引。
    pub global_char_idx: usize,
    // 保存当前视觉行应用 UAX #9 L1 后的嵌入级别。
    pub bidi_level: u8,
    // 保存源字符。
    pub ch: char,
    // 保存行内水平位置。
    pub x: f32,
    // 保存当前字符消费的 advance。
    pub width: f32,
    // 保存当前字符字号。
    pub font_size: f32,
    // 保存当前字符前景色。
    pub color: Color,
    // 保存可选背景色。
    pub bg_color: Option<Color>,
    // 标记当前字符是否属于链接。
    pub is_link: bool,
    // 使用共享字符串保存链接 URL，避免逐字形分配。
    pub link_url: Option<std::sync::Arc<str>>,
}

// 描述一个完整视觉行。
#[derive(Debug, Clone)]
pub(crate) struct LayoutLine {
    // 保存视觉行顶部。
    pub y: f32,
    // 保存视觉行高度。
    pub height: f32,
    // 保存行内全部布局字形。
    pub glyphs: Vec<LayoutGlyph>,
}

// 描述代码块复制按钮命中区域。
#[derive(Debug, Clone)]
pub(crate) struct CodeCopyRegion {
    // 保存复制按钮矩形。
    pub rect: Rect,
    // 保存关联代码 segment 索引。
    pub segment_idx: usize,
}
