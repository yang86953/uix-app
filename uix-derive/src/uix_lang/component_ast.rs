// 引入共享表达式、节点与源码跨度。
use super::{Expression, Node, SourceSpan};

// 表示完成声明级验证的自定义组件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComponentDeclaration {
    // 保存 PascalCase 组件名称。
    pub(crate) name: String,
    // 保存源码顺序中的 props。
    pub(crate) props: Vec<ComponentProp>,
    // 保存源码顺序中的私有 state。
    pub(crate) states: Vec<ComponentState>,
    // 保存组件视图体的有序节点。
    pub(crate) children: Vec<Node>,
    // 保存完整 Component 声明跨度。
    pub(crate) span: SourceSpan,
}

// 表示一个具名组件输入参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComponentProp {
    // 保存参数名称。
    pub(crate) name: String,
    // 保存经过白名单验证的参数类型。
    pub(crate) kind: ComponentPropType,
    // 保存所属 props 属性跨度。
    pub(crate) span: SourceSpan,
}

// 表示文档允许的四类 props 结构。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ComponentPropType {
    // 保存基础值参数。
    Value(ComponentValueType),
    // 保存共享响应式状态参数。
    State(ComponentValueType),
    // 保存回调参数列表与可选返回类型。
    Callback {
        // 保存源码顺序中的回调参数类型。
        parameters: Vec<ComponentValueType>,
        // 保存可选返回基础类型。
        returns: Option<ComponentValueType>,
    },
}

// 表示 props 与回调白名单中的基础类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ComponentValueType {
    // 映射到 Rust String。
    String,
    // 映射到 Rust f64。
    Number,
    // 映射到 Rust bool。
    Bool,
}

// 表示一个组件私有状态槽。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComponentState {
    // 保存状态名称。
    pub(crate) name: String,
    // 保存确定性初始值。
    pub(crate) initial: ComponentStateInitial,
    // 保存所属 state 属性跨度。
    pub(crate) span: SourceSpan,
}

// 表示普通受限表达式或 state 专用空数组初始值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ComponentStateInitial {
    // 保存基础或 Rust 可推断表达式。
    Expression(Expression),
    // 保存文档允许但普通表达式禁止的空数组。
    EmptyArray,
}
