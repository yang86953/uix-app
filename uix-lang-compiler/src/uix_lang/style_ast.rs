// 引入共享元素与源码跨度。
use super::{RecordDeclaration, SourceSpan, VisualDeclaration, WidgetDeclaration};

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
    // 保存具名关键帧动画声明。
    Keyframes(KeyframesDeclaration),
    // 保存已经验证 props、state 与视图体的顶层组件定义。
    Widget(WidgetDeclaration),
    // 保存语言面声明的类型化业务模型。
    Record(RecordDeclaration),
    // 保存语言面唯一拥有的静态视觉常量记录。
    Visual(VisualDeclaration),
}

// 表示一个文件导入及可选具名组件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportDeclaration {
    // 保存 .uix 相对或绝对路径。
    pub(crate) path: String,
    // 保存可选的单个组件名。
    pub(crate) widget: Option<String>,
    // 保存完整指令跨度。
    pub(crate) span: SourceSpan,
}

// 表示显式组件导出列表。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExportDeclaration {
    // 保存源码顺序中的组件名。
    pub(crate) widgets: Vec<String>,
    // 保存完整指令跨度。
    pub(crate) span: SourceSpan,
}

// 表示一个具名样式类。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StyleClassDeclaration {
    // 保存样式类名。
    pub(crate) name: String,
    // 保存可选状态伪类。
    pub(crate) state: Option<StylePseudoState>,
    // 保存可选继承目标。
    pub(crate) extends: Option<String>,
    // 保存源码顺序中的样式属性。
    pub(crate) properties: Vec<StyleProperty>,
    // 保存完整声明跨度。
    pub(crate) span: SourceSpan,
}

// 表示一个具名关键帧动画声明。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KeyframesDeclaration {
    // 保存 animation 属性引用的声明名。
    pub(crate) name: String,
    // 保存按规范化偏移升序排列的关键帧。
    pub(crate) frames: Vec<KeyframeDeclaration>,
    // 保存完整声明跨度。
    pub(crate) span: SourceSpan,
}

// 表示关键帧序列中的单个偏移与样式快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KeyframeDeclaration {
    // 保存零到一百万闭区间内的确定性百万分比时间偏移。
    pub(crate) offset_millionths: u32,
    // 保存当前关键帧声明的样式属性。
    pub(crate) properties: Vec<StyleProperty>,
    // 保存包含选择器与样式块的完整跨度。
    pub(crate) span: SourceSpan,
}

// 表示编译期登记的样式状态伪类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum StylePseudoState {
    // 指针位于组件命中区域内。
    Hover,
    // 组件现有 disabled 事实为真。
    Disabled,
    // 组件现有 checked 事实为真。
    Checked,
}

// 实现状态名称的规范文本映射。
impl StylePseudoState {
    // 返回 UIX 源码中的状态名称。
    pub(crate) const fn as_str(self) -> &'static str {
        // 按闭合白名单返回规范名称。
        match self {
            // 返回 hover 名称。
            Self::Hover => "hover",
            // 返回 disabled 名称。
            Self::Disabled => "disabled",
            // 返回 checked 名称。
            Self::Checked => "checked",
        }
    }
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
