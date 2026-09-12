//! Module 与组件共用的普通数据方法执行内核。
use super::value::{quota_error, type_error};
use super::{Limits, RuntimeResult, Value};

fn allocation(limits: &Limits, bytes: usize) -> RuntimeResult<()> {
    if bytes > limits.value_bytes {
        Err(quota_error())
    } else {
        Ok(())
    }
}

pub(crate) fn method_data(
    value: Value,
    name: &str,
    args: Vec<Value>,
    limits: &Limits,
) -> RuntimeResult<Value> {
    let argument = |index: usize| args.get(index).ok_or_else(|| type_error("方法参数缺失"));
    let expected = match name {
        "replace" => 2,
        "split" | "join" | "contains" | "push" | "get" | "unwrapOr" => 1,
        _ => 0,
    };
    if args.len() != expected {
        return Err(type_error("方法参数数量不符"));
    }
    let result = match (value, name) {
        (Value::String(v), "len") => Value::Int(v.chars().count() as i64),
        (Value::Array(v), "len") => Value::Int(v.len() as i64),
        (Value::Bytes(v), "len") => Value::Int(v.len() as i64),
        (Value::String(v), "trim") => Value::String(v.trim().to_string()),
        (Value::String(v), "toUpperCase") => {
            allocation(limits, v.len().saturating_mul(3))?;
            Value::String(v.to_uppercase())
        }
        (Value::String(v), "toLowerCase") => {
            allocation(limits, v.len().saturating_mul(3))?;
            Value::String(v.to_lowercase())
        }
        (Value::String(v), "chars") => {
            allocation(
                limits,
                v.chars()
                    .count()
                    .saturating_mul(std::mem::size_of::<Value>() + 4),
            )?;
            Value::Array(v.chars().map(|c| Value::String(c.to_string())).collect())
        }
        (Value::String(v), "words") => {
            allocation(
                limits,
                v.split_whitespace()
                    .count()
                    .saturating_mul(std::mem::size_of::<Value>())
                    .saturating_add(v.len()),
            )?;
            Value::Array(
                v.split_whitespace()
                    .map(|v| Value::String(v.to_string()))
                    .collect(),
            )
        }
        (Value::String(v), "lines") => {
            allocation(
                limits,
                v.lines()
                    .count()
                    .saturating_mul(std::mem::size_of::<Value>())
                    .saturating_add(v.len()),
            )?;
            Value::Array(v.lines().map(|v| Value::String(v.to_string())).collect())
        }
        (Value::String(v), "split") => {
            let delimiter = argument(0)?.as_str()?;
            allocation(
                limits,
                v.split(delimiter)
                    .count()
                    .saturating_mul(std::mem::size_of::<Value>())
                    .saturating_add(v.len()),
            )?;
            Value::Array(
                v.split(delimiter)
                    .map(|v| Value::String(v.to_string()))
                    .collect(),
            )
        }
        (Value::String(v), "replace") => {
            let from = argument(0)?.as_str()?;
            let to = argument(1)?.as_str()?;
            allocation(
                limits,
                v.len()
                    .saturating_add(v.matches(from).count().saturating_mul(to.len())),
            )?;
            Value::String(v.replace(from, to))
        }
        (Value::String(v), "contains") => Value::Bool(v.contains(argument(0)?.as_str()?)),
        (Value::Array(v), "contains") => Value::Bool(v.contains(argument(0)?)),
        (Value::Array(mut v), "push") => {
            v.push(argument(0)?.clone());
            Value::Array(v)
        }
        (Value::Array(v), "join") => {
            let separator = argument(0)?.as_str()?;
            let strings = v
                .iter()
                .map(Value::as_str)
                .collect::<RuntimeResult<Vec<_>>>()?;
            let bytes = strings
                .iter()
                .fold(0usize, |n, s| n.saturating_add(s.len()))
                .saturating_add(
                    separator
                        .len()
                        .saturating_mul(strings.len().saturating_sub(1)),
                );
            allocation(limits, bytes)?;
            Value::String(strings.join(separator))
        }
        (Value::Array(v), "get") => {
            let index = usize::try_from(argument(0)?.as_int()?).ok();
            Value::Optional(index.and_then(|i| v.get(i)).cloned().map(Box::new))
        }
        (Value::Optional(v), "isSome") => Value::Bool(v.is_some()),
        (Value::Result(v), "isOk") => Value::Bool(v.is_ok()),
        (Value::Optional(v), "unwrapOr") => v.map(|v| *v).unwrap_or(argument(0)?.clone()),
        (Value::Result(v), "unwrapOr") => v.map(|v| *v).unwrap_or(argument(0)?.clone()),
        (v, "toString") => Value::String(v.display_text()?),
        _ => return Err(type_error("类型不支持该方法")),
    };
    result.validate_budget(limits.value_bytes, limits.value_items)?;
    Ok(result)
}
