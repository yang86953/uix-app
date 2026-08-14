/// Manages text content and rendering properties.
#[derive(Default, Clone)]
pub struct TextManager {
    text: String,
    font_size: f32,
    font_path: String,
    color: Option<crate::draw::Color>,
    placeholder: String,
    bold: bool,
    italic: bool,
    alignment: crate::draw::HAlign,
}

impl TextManager {
    /// 创建使用所有属性默认值的文本管理器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 返回当前文本内容。
    pub fn text(&self) -> &str {
        &self.text
    }
    /// 替换当前文本内容。
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// 返回当前字体大小。
    pub fn font_size(&self) -> f32 {
        self.font_size
    }
    /// 设置字体大小。
    pub fn set_font_size(&mut self, size: f32) {
        self.font_size = size;
    }

    /// 返回当前字体资源路径。
    pub fn font_path(&self) -> &str {
        &self.font_path
    }
    /// 设置字体资源路径。
    pub fn set_font_path(&mut self, path: impl Into<String>) {
        self.font_path = path.into();
    }

    /// 返回显式文本颜色；`None` 表示由上层决定颜色。
    pub fn color(&self) -> Option<crate::draw::Color> {
        self.color
    }
    /// 设置显式文本颜色。
    pub fn set_color(&mut self, c: crate::draw::Color) {
        self.color = Some(c);
    }

    /// 返回值为空时使用的占位文本。
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }
    /// 设置值为空时使用的占位文本。
    pub fn set_placeholder(&mut self, p: impl Into<String>) {
        self.placeholder = p.into();
    }

    /// 返回当前水平文本对齐方式。
    pub fn alignment(&self) -> &crate::draw::HAlign {
        &self.alignment
    }
    /// 设置水平文本对齐方式。
    pub fn set_alignment(&mut self, a: crate::draw::HAlign) {
        self.alignment = a;
    }

    /// 设置是否使用粗体字重。
    pub fn set_bold(&mut self, v: bool) {
        self.bold = v;
    }
    /// 返回是否启用粗体字重。
    pub fn bold(&self) -> bool {
        self.bold
    }

    /// 设置是否使用斜体字形。
    pub fn set_italic(&mut self, v: bool) {
        self.italic = v;
    }
    /// 返回是否启用斜体字形。
    pub fn italic(&self) -> bool {
        self.italic
    }
}
