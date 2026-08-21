// 引入共享源码跨度。
use super::{LoweredActionBlock, SourceSpan};

// 表示一个已经通过受限语法验证的表达式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Expression {
    // 保存表达式的确定性结构。
    pub(crate) kind: ExpressionKind,
    // 保存表达式覆盖的源码跨度。
    pub(crate) span: SourceSpan,
}

// 枚举规范允许的全部表达式结构。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExpressionKind {
    // 保存 action 内联阶段的独立标签块桥接；表达式解析器不接受该形状。
    LoweredAction(Box<LoweredActionBlock>),
    // 保存标识符引用。
    Identifier(String),
    // 保存未改写的数字字面量。
    Number(String),
    // 保存已经解码的单引号字符串。
    String(String),
    // 保存布尔字面量。
    Boolean(bool),
    // 保存仅供已登记结构属性消费的对象字面量。
    Object(Vec<ObjectField>),
    // 保存数据绑定与不可变更新使用的数组字面量。
    Array(Vec<Expression>),
    // 保存只允许作为数组操作参数的单参数闭包。
    Closure {
        // 保存闭包局部参数名称。
        parameter: String,
        // 保存唯一表达式闭包体。
        body: Box<Expression>,
    },
    // 保存一元运算。
    Unary {
        // 保存一元运算符。
        operator: UnaryOperator,
        // 保存一元操作数。
        operand: Box<Expression>,
    },
    // 保存二元运算。
    Binary {
        // 保存左操作数。
        left: Box<Expression>,
        // 保存二元运算符。
        operator: BinaryOperator,
        // 保存右操作数。
        right: Box<Expression>,
    },
    // 保存三元条件表达式。
    Ternary {
        // 保存条件表达式。
        condition: Box<Expression>,
        // 保存条件为真时的表达式。
        then_branch: Box<Expression>,
        // 保存条件为假时的表达式。
        else_branch: Box<Expression>,
    },
    // 保存点号成员访问。
    Member {
        // 保存被访问对象。
        object: Box<Expression>,
        // 保存成员名称。
        member: String,
    },
    // 保存下标访问。
    Index {
        // 保存被索引对象。
        object: Box<Expression>,
        // 保存下标表达式。
        index: Box<Expression>,
    },
    // 保存经过调用规则验证的调用。
    Call {
        // 保存调用目标。
        callee: Box<Expression>,
        // 保存源码顺序中的调用参数。
        arguments: Vec<CallArgument>,
    },
}

// 表示对象字面量中一个按源码顺序保存的字段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObjectField {
    // 保存经过词法验证的标识符键。
    pub(crate) name: String,
    // 保存字段值的受限表达式。
    pub(crate) value: Expression,
    // 保存从键到字段值末尾的完整跨度。
    pub(crate) span: SourceSpan,
}

// 表示位置参数或 setState 命名参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CallArgument {
    // 保存可选命名参数名称。
    pub(crate) name: Option<String>,
    // 保存参数值表达式。
    pub(crate) value: Expression,
    // 保存完整参数跨度。
    pub(crate) span: SourceSpan,
}

// 枚举规范允许的一元运算符。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnaryOperator {
    // 表示逻辑非。
    Not,
    // 表示数值取负。
    Negate,
}

// 枚举规范允许的二元运算符。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BinaryOperator {
    // 表示加法。
    Add,
    // 表示减法。
    Subtract,
    // 表示乘法。
    Multiply,
    // 表示除法。
    Divide,
    // 表示取余。
    Remainder,
    // 表示相等比较。
    Equal,
    // 表示不等比较。
    NotEqual,
    // 表示小于比较。
    Less,
    // 表示小于等于比较。
    LessEqual,
    // 表示大于比较。
    Greater,
    // 表示大于等于比较。
    GreaterEqual,
    // 表示逻辑与。
    And,
    // 表示逻辑或。
    Or,
}
