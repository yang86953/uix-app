//! 语法树仅描述作者所写结构；名字解析、类型及效果由后续语义阶段决定。

/// 原源码字节半开区间。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    pub text: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    pub names: Vec<ImportName>,
    pub source: String,
    pub source_span: Span,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportName {
    pub imported: Name,
    pub local: Name,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Declaration {
    pub exported: bool,
    pub name: Name,
    pub kind: DeclarationKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeclarationKind {
    Component {
        parameters: Vec<Parameter>,
        body: Vec<ComponentMember>,
    },
    Function(Function),
    Type(TypeNode),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: Name,
    /// 仅 lambda 参数可以缺省类型，由调用点推导。
    pub ty: Option<TypeNode>,
    pub default: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub parameters: Vec<Parameter>,
    pub returns: TypeNode,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComponentMember {
    State {
        name: Name,
        ty: TypeNode,
        initial: Expr,
        span: Span,
    },
    Function {
        name: Name,
        function: Function,
        span: Span,
    },
    Statement(Statement),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeNode {
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    Named {
        path: Vec<Name>,
        arguments: Vec<TypeNode>,
    },
    Array(Box<TypeNode>),
    Record(Vec<(Name, TypeNode)>),
    Function {
        parameters: Vec<(Name, TypeNode)>,
        returns: Box<TypeNode>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StatementKind {
    Let {
        name: Name,
        ty: Option<TypeNode>,
        value: Expr,
    },
    Assign {
        target: Expr,
        value: Expr,
    },
    Evaluate(Expr),
    If {
        condition: Expr,
        then_block: Block,
        else_block: Option<Block>,
    },
    Return(Option<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Unit,
    Bool(bool),
    /// 保留正数字面量的幅度，允许语义阶段处理 `-9223372036854775808`。
    /// 语法阶段只接收至 i64::MAX + 1；正数是否溢出由共同类型检查决定。
    Integer(u64),
    Float(f64),
    String(String),
    Name(Name),
    Array(Vec<Expr>),
    Record(Vec<(Name, Expr)>),
    Unary {
        op: UnaryOp,
        value: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Conditional {
        condition: Box<Expr>,
        yes: Box<Expr>,
        no: Box<Expr>,
    },
    Member {
        value: Box<Expr>,
        name: Name,
    },
    Index {
        value: Box<Expr>,
        index: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
    },
    Lambda {
        parameters: Vec<Parameter>,
        body: LambdaBody,
    },
    Element(Element),
    Fragment(Vec<ViewChild>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Negate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Or,
    And,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LambdaBody {
    Expression(Box<Expr>),
    Block(Block),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub name: Vec<Name>,
    pub attributes: Vec<Attribute>,
    pub children: Vec<ViewChild>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub name: Name,
    /// 无值属性为真正的 Bool(true)，不是字符串。
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViewChild {
    /// 保留原始文本及空白。布局文本规范化属于共享语义阶段。
    Text {
        value: String,
        span: Span,
    },
    Expression(Expr),
}
