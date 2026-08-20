// 引入独立 action 语句使用的受限表达式与源码跨度。
use super::{Expression, SourceSpan};

// 表示兼容的单表达式主体或结构化同步语句块。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ActionBody {
    // 保存旧语法的单一受限表达式。
    Expression(Expression),
    // 保存由 do 引入的独立语句 AST。
    Block(ActionBlock),
}

// 表示建立独立词法作用域的有序 action 语句块。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActionBlock {
    // 保存源码顺序中的语句。
    pub(crate) statements: Vec<ActionStatement>,
    // 保存完整花括号块跨度。
    pub(crate) span: SourceSpan,
}

// 表示多语句同步 action 当前登记的全部语句形状。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ActionStatement {
    // 声明从下一条语句开始可见的词法局部。
    Let {
        // 保存局部名称。
        name: String,
        // 保存初始化表达式。
        initializer: Expression,
        // 语义阶段标记该绑定是否实际被赋值。
        mutable: bool,
        // 保存语句跨度。
        span: SourceSpan,
    },
    // 赋值给最近的 action 局部。
    Assign {
        // 保存局部名称。
        name: String,
        // 保存右值表达式。
        value: Expression,
        // 保存语句跨度。
        span: SourceSpan,
    },
    // 按顺序执行并丢弃结果的表达式语句。
    Expression {
        // 保存表达式。
        expression: Expression,
        // 保存语句跨度。
        span: SourceSpan,
    },
    // 保存条件与两个独立词法分支。
    If {
        // 保存条件表达式。
        condition: Expression,
        // 保存真分支。
        then_block: ActionBlock,
        // 保存可选假分支。
        else_block: Option<ActionBlock>,
        // 保存完整条件语句跨度。
        span: SourceSpan,
    },
    // 退出当前 action，并可携带返回表达式。
    Return {
        // None 表示返回单元值。
        value: Option<Expression>,
        // 保存语句跨度。
        span: SourceSpan,
    },
}

// 表示仅由 action 静态内联阶段构造的表达式位置 Rust 标签块。
// 普通表达式解析器永远不会产生该结构，语句文法仍由独立 AST 拥有。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoweredActionBlock {
    // 保存每个内联调用独占的 Rust 标签名称。
    pub(crate) label: String,
    // 保存已完成组件字段与嵌套 action 降低的语句块。
    pub(crate) block: ActionBlock,
}
