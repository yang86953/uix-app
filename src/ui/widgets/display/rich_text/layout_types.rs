//! 保存 RichText 内部共享的布局数据类型。

// 引入矩形与颜色类型。
use crate::core::Rect;
// 引入布局字形颜色。
use crate::draw::Color;

// 描述单个布局原子的绘制语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayoutGlyphKind {
    // 普通可绘制文本字形。
    Text,
    // 由一个几何盒代表的内联图片替换对象。
    InlineImage,
}

// 描述带样式与源字符索引的布局字形。
#[derive(Debug, Clone)]
pub(crate) struct LayoutGlyph {
    // 区分文本字形与原子内联图片。
    pub kind: LayoutGlyphKind,
    // 保存所属富文本 segment 索引。
    pub segment_idx: usize,
    // 保存完整逻辑源中的 Unicode 标量索引。
    pub global_char_idx: usize,
    // 保存当前视觉原子覆盖的逻辑源字符数量。
    pub source_char_len: usize,
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
}

// 描述绘制层需要区分的视觉行类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayoutLineKind {
    // 普通文本或空白行。
    Text,
    // 独立主题分隔线行。
    ThematicBreak,
}

// 描述一个完整视觉行。
#[derive(Debug, Clone)]
pub(crate) struct LayoutLine {
    // 保存视觉行顶部。
    pub y: f32,
    // 保存视觉行高度。
    pub height: f32,
    // 保存绘制层需要区分的行类型。
    pub kind: LayoutLineKind,
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
