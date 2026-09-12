//! Module 和组件编译器共用的数据运算类型合同；执行仍使用同一拥有型 Value。

use super::{Binary, Type};

pub fn binary_signature(op: Binary, left: &Type, right: &Type) -> Option<Type> {
    if left != right {
        return None;
    }
    use Binary::*;
    Some(match op {
        And | Or if *left == Type::Bool => Type::Bool,
        Equal | NotEqual => Type::Bool,
        Less | LessEqual | Greater | GreaterEqual
            if matches!(left, Type::Int | Type::Float | Type::String) =>
        {
            Type::Bool
        }
        Add if *left == Type::String => Type::String,
        Add | Subtract | Multiply | Divide | Remainder
            if matches!(left, Type::Int | Type::Float) =>
        {
            left.clone()
        }
        _ => return None,
    })
}

/// 不包含回调执行的普通数据方法；map/filter 由调用方检查其函数值及效果。
pub fn method_signature(value: &Type, name: &str) -> Option<(Vec<Type>, Type)> {
    Some(match (value, name) {
        (Type::String | Type::Array(_) | Type::Bytes, "len") => (vec![], Type::Int),
        (Type::String, "trim" | "toUpperCase" | "toLowerCase") => (vec![], Type::String),
        (Type::String, "chars" | "words" | "lines") => {
            (vec![], Type::Array(Box::new(Type::String)))
        }
        (Type::String, "split") => (vec![Type::String], Type::Array(Box::new(Type::String))),
        (Type::String, "replace") => (vec![Type::String, Type::String], Type::String),
        (Type::String, "contains") => (vec![Type::String], Type::Bool),
        (Type::Array(t), "contains") => (vec![*t.clone()], Type::Bool),
        (Type::Array(t), "push") => (vec![*t.clone()], value.clone()),
        (Type::Array(t), "get") => (vec![Type::Int], Type::Optional(t.clone())),
        (Type::Array(t), "join") if **t == Type::String => (vec![Type::String], Type::String),
        (Type::Optional(_), "isSome") | (Type::Result(_, _), "isOk") => (vec![], Type::Bool),
        (Type::Optional(t) | Type::Result(t, _), "unwrapOr") => (vec![*t.clone()], *t.clone()),
        (Type::String | Type::Int | Type::Float | Type::Bool, "toString") => (vec![], Type::String),
        _ => return None,
    })
}

/// 工具的候选名集合仍由同一签名函数过滤，不另写一份接收者类型规则。
pub fn method_names(value: &Type) -> impl Iterator<Item = &'static str> + '_ {
    [
        "len",
        "trim",
        "toUpperCase",
        "toLowerCase",
        "chars",
        "words",
        "lines",
        "split",
        "replace",
        "contains",
        "push",
        "get",
        "join",
        "isSome",
        "isOk",
        "unwrapOr",
        "toString",
    ]
    .into_iter()
    .filter(|name| method_signature(value, name).is_some())
}
