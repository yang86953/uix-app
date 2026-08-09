// 引入共享元素与源码跨度。
use super::{Element, SourceSpan};

// 表示根元素之前按源码顺序出现的顶层声明。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Declaration {
    // 保存导入指令。
    Import(ImportDeclaration),
    // 保存导出指令。
    Export(ExportDeclaration),
    // 保存样式类声明。
    StyleClass(StyleClassDeclaration),
    // 保存主题声明。
    Theme(ThemeDeclaration),
    // 暂存由后续组件 Gate 继续验证的顶层组件定义。
    Component(Element),
}

// 表示一个文件导入及可选具名组件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportDeclaration {
    // 保存 .uix 相对或绝对路径。
    pub(crate) path: String,
    // 保存可选的单个组件名。
    pub(crate) component: Option<String>,
    // 保存完整指令跨度。
    pub(crate) span: SourceSpan,
}

// 表示显式组件导出列表。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExportDeclaration {
    // 保存源码顺序中的组件名。
    pub(crate) components: Vec<String>,
    // 保存完整指令跨度。
    pub(crate) span: SourceSpan,
}

// 表示一个具名样式类。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StyleClassDeclaration {
    // 保存样式类名。
    pub(crate) name: String,
    // 保存可选继承目标。
    pub(crate) extends: Option<String>,
    // 保存源码顺序中的样式属性。
    pub(crate) properties: Vec<StyleProperty>,
    // 保存完整声明跨度。
    pub(crate) span: SourceSpan,
}

// 表示一个具名主题。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ThemeDeclaration {
    // 保存主题名。
    pub(crate) name: String,
    // 保存源码顺序中的主题属性。
    pub(crate) properties: Vec<StyleProperty>,
    // 保存完整声明跨度。
    pub(crate) span: SourceSpan,
}

// 表示样式块或内联样式中的单个属性。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StyleProperty {
    // 保存属性名及可选状态后缀。
    pub(crate) name: String,
    // 保存未改写的样式值和哈希引用事实。
    pub(crate) value: StyleValue,
    // 保存完整属性跨度。
    pub(crate) span: SourceSpan,
}

// 表示一个保留 CSS 形态的样式值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StyleValue {
    // 保存去除外围空白的原始值。
    pub(crate) source: String,
    // 保存值内出现的颜色或主题引用。
    pub(crate) hashes: Vec<StyleHash>,
    // 保存规范化值跨度。
    pub(crate) span: SourceSpan,
}

// 表示样式值中以井号开头的语义片段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StyleHash {
    // 保存已经区分的哈希语义。
    pub(crate) kind: StyleHashKind,
    // 保存包含井号的源码跨度。
    pub(crate) span: SourceSpan,
}

// 区分十六进制颜色与主题属性引用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StyleHashKind {
    // 保存十六进制颜色数字。
    HexColor(String),
    // 保存主题属性名。
    ThemeReference(String),
}
