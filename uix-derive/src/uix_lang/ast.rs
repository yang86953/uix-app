// 引入受限表达式与顶层声明 AST。
use super::{Declaration, Expression, StyleProperty};

// 表示源码中的半开字节区间及其一基行列位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceSpan {
    // 保存起始 UTF-8 字节偏移。
    pub(crate) start: usize,
    // 保存结束 UTF-8 字节偏移。
    pub(crate) end: usize,
    // 保存起始的一基行号。
    pub(crate) line: usize,
    // 保存起始的一基字符列号。
    pub(crate) column: usize,
}

// 表示一个已经通过唯一根约束的 UIX 文档。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Document {
    // 保存根元素之前的顶层声明顺序。
    pub(crate) declarations: Vec<Declaration>,
    // 保存文档的唯一根元素。
    pub(crate) root: Element,
}

// 表示标签元素及其属性和有序子节点。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Element {
    // 保存 PascalCase 标签名。
    pub(crate) name: String,
    // 保存声明顺序中的属性。
    pub(crate) attributes: Vec<Attribute>,
    // 保存 painter 与组合语义所需的子节点顺序。
    pub(crate) children: Vec<Node>,
    // 保存 If 或 For 元素专用的控制绑定。
    pub(crate) control: Option<ControlBinding>,
    // 保存从开始标签到结束标签的完整跨度。
    pub(crate) span: SourceSpan,
}

// 表示标签上的一个具名属性。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Attribute {
    // 保存属性名。
    pub(crate) name: String,
    // 保存字面值或表达式值。
    pub(crate) value: AttributeValue,
    // 保存完整属性跨度。
    pub(crate) span: SourceSpan,
}

// 区分双引号属性字面量与花括号表达式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AttributeValue {
    // 保存已经去除双引号的字面值。
    Literal(String),
    // 保存已经完成受限语法验证的表达式。
    Expression(ExpressionNode),
    // 保存使用共享样式语法解析的内联属性。
    InlineStyle(Vec<StyleProperty>),
}

// 表示条件渲染或循环元素的专用绑定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ControlBinding {
    // 保存 If 条件表达式。
    If(ExpressionNode),
    // 保存 For 单标识符绑定与数据源。
    For {
        // 保存循环项绑定名称。
        binding: String,
        // 保存绑定声明跨度。
        binding_span: SourceSpan,
        // 保存可选索引绑定名称。
        index_binding: Option<String>,
        // 保存可选索引绑定声明跨度。
        index_span: Option<SourceSpan>,
        // 保存数据源表达式。
        iterable: ExpressionNode,
        // 保存可选稳定行身份表达式。
        key: Option<ExpressionNode>,
    },
}

// 表示元素内部的有序节点联合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Node {
    // 保存嵌套元素。
    Element(Element),
    // 保存普通文本。
    Text(TextNode),
    // 保存文本插值表达式。
    Interpolation(ExpressionNode),
}

// 表示已经处理转义花括号的文本节点。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextNode {
    // 保存用户可见文本。
    pub(crate) value: String,
    // 保存原始文本跨度。
    pub(crate) span: SourceSpan,
}

// 表示保留原始内容的表达式节点。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExpressionNode {
    // 保存去除外层花括号后的源码。
    pub(crate) source: String,
    // 保存确定性表达式 AST。
    pub(crate) expression: Expression,
    // 保存包含花括号的完整跨度。
    pub(crate) span: SourceSpan,
}
