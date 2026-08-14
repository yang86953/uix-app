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
    // 保存由组件展开附加到实际 View 根节点的私有状态作用域标记。
    pub(crate) component_scopes: Vec<ComponentScopeMarker>,
}

// 表示一个应在最终 ViewNode 外层应用的组件状态装饰。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ComponentScopeMarker {
    // 保存普通组件根的生命周期作用域标记。
    Scope {
        // 保存生成阶段创建的卫生作用域局部变量名称。
        scope_name: String,
        // 保存组件多根输出中的稳定根序号。
        root_ordinal: u64,
    },
    // 保存需要包裹事件 View 的动态样式状态。
    DynamicStyle(DynamicStyleBinding),
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

// 保存一个动态样式节点的组件状态与稳定身份元数据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DynamicStyleBinding {
    // 保存最近组件作用域的卫生局部变量名称。
    pub(crate) component_scope_name: String,
    // 保存节点类型与静态位置共同形成的子作用域声明标识。
    pub(crate) declaration_id: u64,
    // 保存子作用域内动态样式状态的稳定字段标识。
    pub(crate) field_id: u64,
    // 保存生成的闭合样式枚举名称。
    pub(crate) enum_name: String,
    // 保存当前节点所属最近 For 实例路径的卫生名称。
    pub(crate) instance_path_name: Option<String>,
    // 保存参与实际节点身份的可选 key。
    pub(crate) key: Option<DynamicStyleKey>,
    // 保存原始类与所有可达目标类的类型化分支。
    pub(crate) variants: Vec<DynamicStyleVariant>,
}

// 保存动态样式节点用于 reconcile 的显式 key。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DynamicStyleKey {
    // 保存字符串字面量 key。
    Literal(String),
    // 保存已经完成组件字段改写的表达式 key。
    Expression(ExpressionNode),
}

// 保存一个动态样式枚举分支及其零参数状态写入器。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DynamicStyleVariant {
    // 保存生成枚举变体的卫生名称。
    pub(crate) variant_name: String,
    // 保存每个调用点独占的卫生 setter 名称，避免事件闭包重复移动。
    pub(crate) setter_names: Vec<String>,
    // 保存类层与内联层合并后的完整 Style 字段更新。
    pub(crate) properties: Vec<StyleProperty>,
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
