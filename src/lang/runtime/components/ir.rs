//! 检查器降低的执行 IR；仅构建器与显式动态能力需要。AOT 目标不编入这些节点。
use super::*;
use crate::lang::runtime::Binary;

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub location: Location,
}
#[derive(Debug, Clone)]
pub enum ExprKind {
    Constant(DataValue),
    Get(BindingId),
    Closure(FunctionId),
    NativeFunction(ExportKey),
    Unary {
        negate: bool,
        value: Box<Expr>,
    },
    Binary {
        op: Binary,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Conditional {
        condition: Box<Expr>,
        yes: Box<Expr>,
        no: Box<Expr>,
    },
    Array {
        values: Vec<Expr>,
        rich: bool,
    },
    Record(Vec<(String, Expr)>),
    Member {
        value: Box<Expr>,
        name: String,
    },
    Index {
        value: Box<Expr>,
        index: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
    },
    Method {
        value: Box<Expr>,
        name: String,
        arguments: Vec<Expr>,
    },
    Map {
        value: Box<Expr>,
        callback: Box<Expr>,
        filter: bool,
    },
    Element {
        site: usize,
        target: Target,
        attributes: Vec<Attribute>,
    },
    Fragment(Vec<Expr>),
    Concat(Vec<Expr>),
}
#[derive(Debug, Clone)]
pub enum Attribute {
    Key(Expr),
    Property(String, Expr),
}

#[derive(Debug, Clone)]
pub struct Statement {
    pub kind: StatementKind,
    pub location: Location,
}
#[derive(Debug, Clone)]
pub enum StatementKind {
    Let(BindingId, Expr),
    Set(BindingId, Expr),
    Evaluate(Expr),
    If(Expr, Vec<Statement>, Vec<Statement>),
    Return(Option<Expr>),
}
