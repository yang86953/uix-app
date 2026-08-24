// 引入共享表达式、节点与源码跨度。
use super::{ActionBody, Expression, Node, SourceSpan};

// 表示完成声明级验证的自定义组件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WidgetDeclaration {
    // 保存 PascalCase 组件名称。
    pub(crate) name: String,
    // 保存源码顺序中的 props。
    pub(crate) props: Vec<WidgetProp>,
    // 保存源码顺序中的私有 state。
    pub(crate) states: Vec<WidgetState>,
    // 保存源码顺序中的派生值。
    pub(crate) computed: Vec<WidgetComputed>,
    // 保存源码顺序中的同步业务 action。
    pub(crate) actions: Vec<WidgetAction>,
    // 保存组件模板中已经完成唯一性验证的插槽声明。
    pub(crate) slots: Vec<WidgetSlot>,
    // 保存组件体获准引用的 Rust 外部符号。
    pub(crate) external: Vec<String>,
    // 保存组件视图体的有序节点。
    pub(crate) children: Vec<Node>,
    // 保存完整 Widget 声明跨度。
    pub(crate) span: SourceSpan,
}

// 表示一个无参数且在事件位置静态展开的同步 action。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WidgetAction {
    // 保存组件内唯一的 action 名称。
    pub(crate) name: String,
    // 保存兼容单表达式或独立 do 语句块主体。
    pub(crate) body: ActionBody,
    // 保存所属 actions 属性跨度。
    pub(crate) span: SourceSpan,
}

// 表示组件模板中的一个默认或具名插槽占位。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WidgetSlot {
    // 保存具名插槽名称；None 表示默认插槽。
    pub(crate) name: Option<String>,
    // 保存完整 Slot 元素跨度供调用诊断使用。
    pub(crate) span: SourceSpan,
}

// 表示一个按组件展开时机重新求值的有序派生值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WidgetComputed {
    // 保存组件体可引用的派生名称。
    pub(crate) name: String,
    // 保存已经通过受限语法验证的派生表达式。
    pub(crate) expression: Expression,
    // 保存所属 computed 属性跨度。
    pub(crate) span: SourceSpan,
}

// 表示一个具名组件输入参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WidgetProp {
    // 保存参数名称。
    pub(crate) name: String,
    // 保存经过白名单验证的参数类型。
    pub(crate) kind: WidgetPropType,
    // 保存可选的声明期默认表达式；None 表示调用必填。
    pub(crate) default: Option<Expression>,
    // 保存所属 props 属性跨度。
    pub(crate) span: SourceSpan,
}

// 表示文档允许的四类 props 结构。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WidgetPropType {
    // 保存基础值参数。
    Value(WidgetValueType),
    // 保存共享响应式状态参数。
    State(WidgetValueType),
    // 保存回调参数列表与可选返回类型。
    Callback {
        // 保存源码顺序中的回调参数类型。
        parameters: Vec<WidgetValueType>,
        // 保存可选返回基础类型。
        returns: Option<WidgetValueType>,
    },
}

// 表示 props、回调与 record 字段白名单中的类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WidgetValueType {
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
    // 映射到 Vec<f64> 数值集合状态。
    VecOfNumber,
    // 映射到文档内 record 元素向量。
    VecOfRecord(String),
    // 映射到 Vec<UploadFile> 受控上传队列状态。
    VecOfUploadFile,
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
    pub(crate) kind: WidgetValueType,
    // 保存所属字段声明跨度。
    pub(crate) span: SourceSpan,
}

// 表示由 UIX 唯一拥有并生成为 Rust 模块级常量的静态视觉记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VisualDeclaration {
    // 保存 SCREAMING_SNAKE_CASE 常量名称。
    pub(crate) name: String,
    // 保存同一 Rust 模块内的视觉结构类型名称。
    pub(crate) rust_type: String,
    // 保存源码顺序中的具名视觉字段。
    pub(crate) fields: Vec<VisualField>,
    // 保存完整 Visual 声明跨度。
    pub(crate) span: SourceSpan,
}

// 表示一个已映射到 Rust snake_case 字段的静态视觉值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VisualField {
    // 保存 UIX 属性名称，供诊断与查询使用。
    pub(crate) source_name: String,
    // 保存最终 Rust 结构体字段名称。
    pub(crate) rust_name: String,
    // 保存字符串字面量或受限表达式值。
    pub(crate) value: VisualValue,
    // 保存完整字段属性跨度。
    pub(crate) span: SourceSpan,
}

// 区分 Visual 字段的静态字符串与通用常量表达式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VisualValue {
    // 双引号 UIX 属性直接生成静态 Rust 字符串字面量。
    Literal(String),
    // 花括号值使用现有受限表达式 AOT 生成器。
    Expression(Expression),
}

// 表示一个组件私有状态槽。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WidgetState {
    // 保存状态名称。
    pub(crate) name: String,
    // 保存确定性初始值。
    pub(crate) initial: WidgetStateInitial,
    // 保存所属 state 属性跨度。
    pub(crate) span: SourceSpan,
}

// 表示普通受限表达式、类型化表达式或 state 专用空数组初始值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WidgetStateInitial {
    // 保存基础或 Rust 可推断表达式。
    Expression(Expression),
    // 保存带显式类型注解的表达式（如 rating: u32 = 7）。
    TypedExpression(WidgetValueType, Expression),
    // 保存文档允许但普通表达式禁止的空数组。
    EmptyArray,
}
