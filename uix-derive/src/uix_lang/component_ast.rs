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

// 表示 props、回调与 record 字段白名单中的类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ComponentValueType {
    // 映射到 Rust String。
    String,
    // 映射到 Rust f64。
    Number,
    // 映射到 Rust bool。
    Bool,
    // 映射到 Rust u32（评分组件等无符号计数状态）。
    U32,
    // 映射到 Rust usize（步骤、分页等索引状态）。
    USize,
    // 映射到 Rust f32（滚动位置等单精度状态）。
    F32,
    // 映射到 Rust i32（数字输入等有符号整数状态）。
    I32,
    // 映射到公开 Date 语义类型。
    Date,
    // 映射到公开 Time 语义类型。
    Time,
    // 映射到公开 Color 语义类型。
    Color,
    // 映射到公开 Point 语义类型。
    Point,
    // 映射到公开 CascaderValue 路径类型。
    CascaderValue,
    // 映射到 HashSet<String> 集合状态。
    HashSetOfString,
    // 映射到 Vec<String> 集合状态。
    VecOfString,
    // 映射到 Option<String> 可空单选状态。
    OptionalString,
    // 映射到文档内声明的 record 类型。
    Record(String),
}

// 表示文档内声明的类型化业务模型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecordDeclaration {
    // 保存 PascalCase record 名称。
    pub(crate) name: String,
    // 保存源码顺序中的字段声明。
    pub(crate) fields: Vec<RecordField>,
    // 保存完整 Record 声明跨度。
    pub(crate) span: SourceSpan,
}

// 表示 record 的一个字段声明。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecordField {
    // 保存字段名称。
    pub(crate) name: String,
    // 保存白名单字段类型。
    pub(crate) kind: ComponentValueType,
    // 保存所属字段声明跨度。
    pub(crate) span: SourceSpan,
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

// 表示普通受限表达式、类型化表达式或 state 专用空数组初始值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ComponentStateInitial {
    // 保存基础或 Rust 可推断表达式。
    Expression(Expression),
    // 保存带显式类型注解的表达式（如 rating: u32 = 7）。
    TypedExpression(ComponentValueType, Expression),
    // 保存文档允许但普通表达式禁止的空数组。
    EmptyArray,
}
