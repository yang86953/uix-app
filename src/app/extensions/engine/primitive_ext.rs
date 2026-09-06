//! 标准库补充原语：按 P0 兼容矩阵补齐 `(scheme base)` / `(scheme char)` /
//! `(scheme lazy)` / 端口（内存字符串限定）与 bytevector 过程族。
//!
//! 全部过程对参数数量与类型做显式检查；数值缺口沿用数值塔语义。

use std::cell::RefCell;
use std::rc::Rc;

use num_bigint::BigInt;
use num_integer::Integer as _;
use num_rational::BigRational;
use num_traits::{Signed, Zero};

use super::error::SchemeError;
use super::eval::ControlOp;
use super::number::Number;
use super::value::{ErrorKind, PortState, PromiseState, Value};
use super::SchemeEngine;

use super::primitive::{
    at_least, character, exact_arity, fixnum, integer_big, mismatch, number_of, string_cell,
    wrong,
};

/// 注册全部补充原语。
pub(crate) fn install(engine: &mut SchemeEngine) {
    // 数值缺口
    engine.define_primitive("exact-integer?", is_exact_integer);
    engine.define_primitive("exact-integer-sqrt", exact_integer_sqrt);
    engine.define_primitive("floor/", floor_slash);
    engine.define_primitive("floor-quotient", floor_quotient);
    engine.define_primitive("floor-remainder", floor_remainder);
    engine.define_primitive("truncate/", truncate_slash);
    engine.define_primitive("truncate-quotient", truncate_quotient);
    engine.define_primitive("truncate-remainder", truncate_remainder);
    engine.define_primitive("square", square);
    engine.define_primitive("numerator", numerator);
    engine.define_primitive("denominator", denominator);
    engine.define_primitive("rationalize", rationalize);
    engine.define_primitive("boolean=?", boolean_equal);

    // 列表 / 字符串 / 向量缺口
    engine.define_primitive("list-copy", list_copy);
    engine.define_primitive("list-set!", list_set);
    engine.define_primitive("string-map", string_map);
    engine.define_primitive("string-for-each", string_for_each);
    engine.define_primitive("string->vector", string_to_vector);
    engine.define_primitive("vector->string", vector_to_string);
    engine.define_primitive("string-copy", string_copy_range);
    engine.define_primitive("string-copy!", string_copy_bang);
    engine.define_primitive("string-fill!", string_fill);
    engine.define_primitive("vector-map", vector_map);
    engine.define_primitive("vector-for-each", vector_for_each);
    engine.define_primitive("vector-copy", vector_copy);
    engine.define_primitive("vector-copy!", vector_copy_bang);
    engine.define_primitive("vector-fill!", vector_fill_range);
    engine.define_primitive("string-ci=?", string_ci_eq);
    engine.define_primitive("string-ci<?", string_ci_lt);
    engine.define_primitive("string-ci>?", string_ci_gt);
    engine.define_primitive("string-ci<=?", string_ci_le);
    engine.define_primitive("string-ci>=?", string_ci_ge);
    engine.define_primitive("char-ci=?", char_ci_eq);
    engine.define_primitive("char-ci<?", char_ci_lt);
    engine.define_primitive("char-ci>?", char_ci_gt);
    engine.define_primitive("char-ci<=?", char_ci_le);
    engine.define_primitive("char-ci>=?", char_ci_ge);
    engine.define_primitive("digit-value", digit_value);

    // bytevector
    engine.define_primitive("bytevector", bytevector);
    engine.define_primitive("bytevector?", is_bytevector);
    engine.define_primitive("make-bytevector", make_bytevector);
    engine.define_primitive("bytevector-length", bytevector_length);
    engine.define_primitive("bytevector-u8-ref", bytevector_u8_ref);
    engine.define_primitive("bytevector-u8-set!", bytevector_u8_set);
    engine.define_primitive("bytevector-copy", bytevector_copy);
    engine.define_primitive("bytevector-copy!", bytevector_copy_bang);
    engine.define_primitive("bytevector-append", bytevector_append);
    engine.define_primitive("u8-list->bytevector", u8_list_to_bytevector);
    engine.define_primitive("bytevector->u8-list", bytevector_to_u8_list);
    engine.define_primitive("utf8->string", utf8_to_string);
    engine.define_primitive("string->utf8", string_to_utf8);

    // 多值 / 异常 / lazy / 参数
    engine.define_control("values", ControlOp::Values);
    engine.define_control("call-with-values", ControlOp::CallWithValues);
    engine.define_control("raise", ControlOp::Raise);
    engine.define_control("raise-continuable", ControlOp::RaiseContinuable);
    engine.define_control("with-exception-handler", ControlOp::WithExceptionHandler);
    engine.define_primitive("error-object?", is_error_object);
    engine.define_primitive("error-object-message", error_object_message);
    engine.define_primitive("error-object-irritants", error_object_irritants);
    engine.define_primitive("file-error?", is_file_error);
    engine.define_primitive("read-error?", is_read_error);
    engine.define_primitive("make-parameter", make_parameter);
    engine.define_primitive("force", force);
    engine.define_primitive("make-promise", make_promise);
    engine.define_primitive("promise?", is_promise);

    // 端口（内存字符串限定）
    engine.define_primitive("open-input-string", open_input_string);
    engine.define_primitive("open-output-string", open_output_string);
    engine.define_primitive("get-output-string", get_output_string);
    engine.define_primitive("close-port", close_port);
    engine.define_primitive("port?", is_port);
    engine.define_primitive("input-port?", is_input_port);
    engine.define_primitive("output-port?", is_output_port);
    engine.define_primitive("read-char", read_char);
    engine.define_primitive("peek-char", peek_char);
    engine.define_primitive("read-line", read_line);
    engine.define_primitive("read-string", read_string);
    engine.define_primitive("read-u8", read_u8);
    engine.define_primitive("peek-u8", peek_u8);
    engine.define_primitive("read", read_datum);
    engine.define_primitive("char-ready?", char_ready);
    engine.define_primitive("u8-ready?", u8_ready);
    engine.define_primitive("write-string", write_string);
    engine.define_primitive("write-char", write_char);
    engine.define_primitive("write-u8", write_u8);
    engine.define_primitive("write-bytevector", write_bytevector);
    engine.define_primitive("write", write_value);
    engine.define_primitive("display", display_value);
    engine.define_primitive("newline", newline);
    engine.define_primitive("current-output-port", current_output_port);
    engine.define_primitive("eof-object", eof_object);
    engine.define_primitive("eof-object?", is_eof);

    // 受限 eval
    engine.define_primitive("eval", eval_restricted);
    engine.define_primitive("interaction-environment", interaction_environment);

    // (uix extension) 内部支撑：parameterize 脱糖引用。
    engine.define_primitive(" param-ref", param_ref);
    engine.define_primitive(" param-set!", param_set);

    // (scheme cxr) 24 个组合访问器。
    install_cxr(engine);
}

type CiOrdering = fn(std::cmp::Ordering) -> bool;

fn string_ci_eq(e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    compare_ci_strings("string-ci=?", a, ci_ordering_eq)
}
fn string_ci_lt(e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    let _ = e;
    compare_ci_strings("string-ci<?", a, ci_ordering_lt)
}
fn string_ci_gt(e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    let _ = e;
    compare_ci_strings("string-ci>?", a, ci_ordering_gt)
}
fn string_ci_le(e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    let _ = e;
    compare_ci_strings("string-ci<=?", a, ci_ordering_le)
}
fn string_ci_ge(e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    let _ = e;
    compare_ci_strings("string-ci>=?", a, ci_ordering_ge)
}
fn char_ci_eq(e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    let _ = e;
    compare_ci_chars("char-ci=?", a, ci_ordering_eq)
}
fn char_ci_lt(e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    let _ = e;
    compare_ci_chars("char-ci<?", a, ci_ordering_lt)
}
fn char_ci_gt(e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    let _ = e;
    compare_ci_chars("char-ci>?", a, ci_ordering_gt)
}
fn char_ci_le(e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    let _ = e;
    compare_ci_chars("char-ci<=?", a, ci_ordering_le)
}
fn char_ci_ge(e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    let _ = e;
    compare_ci_chars("char-ci>=?", a, ci_ordering_ge)
}

fn ci_ordering_eq(ordering: std::cmp::Ordering) -> bool {
    ordering == std::cmp::Ordering::Equal
}
fn ci_ordering_lt(ordering: std::cmp::Ordering) -> bool {
    ordering == std::cmp::Ordering::Less
}
fn ci_ordering_gt(ordering: std::cmp::Ordering) -> bool {
    ordering == std::cmp::Ordering::Greater
}
fn ci_ordering_le(ordering: std::cmp::Ordering) -> bool {
    ordering != std::cmp::Ordering::Greater
}
fn ci_ordering_ge(ordering: std::cmp::Ordering) -> bool {
    ordering != std::cmp::Ordering::Less
}

// ---------- 数值缺口 ----------

fn is_exact_integer(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("exact-integer?", a, 1)?;
    Ok(Value::Bool(matches!(a[0], Value::Fixnum(_) | Value::Bignum(_))))
}

/// `(exact-integer-sqrt k)` → 两值 `(s r)`，s²+r=k 且 0≤r<2s+1。
fn exact_integer_sqrt(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("exact-integer-sqrt", a, 1)?;
    let radicand = integer_big("exact-integer-sqrt", &a[0])?;
    if radicand.is_negative() {
        return Err(wrong("exact-integer-sqrt", "非负整数"));
    }
    let root = radicand.sqrt();
    let remainder = &radicand - &root * &root;
    let values = vec![
        Number::norm_bignum(root).to_value(),
        Number::norm_bignum(remainder).to_value(),
    ];
    engine.new_values(values)
}

fn floor_slash(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("floor/", a, 2)?;
    let left = integer_big("floor/", &a[0])?;
    let right = integer_big("floor/", &a[1])?;
    if right.is_zero() {
        return Err(SchemeError::DivisionByZero);
    }
    let quotient = left.div_floor(&right);
    let remainder = left.mod_floor(&right);
    engine.new_values(vec![
        Number::norm_bignum(quotient).to_value(),
        Number::norm_bignum(remainder).to_value(),
    ])
}

fn floor_quotient(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("floor-quotient", a, 2)?;
    let left = integer_big("floor-quotient", &a[0])?;
    let right = integer_big("floor-quotient", &a[1])?;
    if right.is_zero() {
        return Err(SchemeError::DivisionByZero);
    }
    Ok(Number::norm_bignum(left.div_floor(&right)).to_value())
}

fn floor_remainder(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("floor-remainder", a, 2)?;
    let left = integer_big("floor-remainder", &a[0])?;
    let right = integer_big("floor-remainder", &a[1])?;
    if right.is_zero() {
        return Err(SchemeError::DivisionByZero);
    }
    Ok(Number::norm_bignum(left.mod_floor(&right)).to_value())
}

fn truncate_slash(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("truncate/", a, 2)?;
    let left = integer_big("truncate/", &a[0])?;
    let right = integer_big("truncate/", &a[1])?;
    if right.is_zero() {
        return Err(SchemeError::DivisionByZero);
    }
    engine.new_values(vec![
        Number::norm_bignum(&left / &right).to_value(),
        Number::norm_bignum(&left % &right).to_value(),
    ])
}

fn truncate_quotient(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("truncate-quotient", a, 2)?;
    let left = integer_big("truncate-quotient", &a[0])?;
    let right = integer_big("truncate-quotient", &a[1])?;
    if right.is_zero() {
        return Err(SchemeError::DivisionByZero);
    }
    Ok(Number::norm_bignum(&left / &right).to_value())
}

fn truncate_remainder(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("truncate-remainder", a, 2)?;
    let left = integer_big("truncate-remainder", &a[0])?;
    let right = integer_big("truncate-remainder", &a[1])?;
    if right.is_zero() {
        return Err(SchemeError::DivisionByZero);
    }
    Ok(Number::norm_bignum(&left % &right).to_value())
}

fn square(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("square", a, 1)?;
    let base = number_of("square", &a[0])?;
    let exponent = number_of("square", &a[0])?;
    base.mul(exponent).map(|n| n.to_value())
}

fn numerator(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("numerator", a, 1)?;
    let value = number_of("numerator", &a[0])?;
    match value {
        Number::Rational(rational) => {
            Ok(Number::norm_bignum(rational.numer().clone()).to_value())
        }
        Number::Fixnum(_) | Number::Bignum(_) => Ok(a[0].clone()),
        Number::Flonum(_) => Err(wrong("numerator", "有理数")),
    }
}

fn denominator(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("denominator", a, 1)?;
    let value = number_of("denominator", &a[0])?;
    match value {
        Number::Rational(rational) => {
            Ok(Number::norm_bignum(rational.denom().clone()).to_value())
        }
        Number::Fixnum(_) | Number::Bignum(_) => Ok(Value::Fixnum(1)),
        Number::Flonum(_) => Err(wrong("denominator", "有理数")),
    }
}

/// `(rationalize x y)`：最接近 x、误差不超过 y 的最简有理数。
fn rationalize(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("rationalize", a, 2)?;
    let target = number_of("rationalize", &a[0])?.rational();
    let tolerance = number_of("rationalize", &a[1])?.rational();
    if tolerance.is_negative() {
        return Err(wrong("rationalize", "非负容差"));
    }
    let low = &target - &tolerance;
    let high = &target + &tolerance;
    Ok(Number::norm_rational(simplest_rational(&low, &high)).to_value())
}

/// 区间 `[low, high]` 内的最简有理数（经典连分数式归约）。
fn simplest_rational(low: &BigRational, high: &BigRational) -> BigRational {
    let (low, high) = if low <= high { (low, high) } else { (high, low) };
    if low.is_negative() && high.is_positive() || *low == BigRational::zero() {
        return BigRational::zero();
    }
    if high.is_negative() {
        return -simplest_rational(&(-high), &(-low));
    }
    // 0 < low <= high。
    let floor_low = low.floor();
    let unit = BigRational::from_integer(BigInt::from(1));
    if floor_low == *low {
        // low 是整数：它是最小的候选。
        return floor_low;
    }
    if floor_low != high.floor() {
        // 区间跨过整数：floor_low + 1 在区间内且分母为 1。
        return floor_low + unit;
    }
    let offset = floor_low.clone();
    let fractional_low = low - &offset;
    let fractional_high = high - &offset;
    let inner = simplest_rational(
        &(unit.clone() / &fractional_high),
        &(unit / &fractional_low),
    );
    offset + inner
}

fn boolean_equal(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    at_least("boolean=?", a, 2)?;
    for pair in a.windows(2) {
        let (Value::Bool(left), Value::Bool(right)) = (&pair[0], &pair[1]) else {
            return Err(wrong("boolean=?", "布尔值"));
        };
        if left != right {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}

// ---------- 列表 / 字符串 / 向量缺口 ----------

fn list_copy(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("list-copy", a, 1)?;
    match &a[0] {
        Value::Null => Ok(Value::Null),
        _ => {
            let elements = engine.value_to_vec("list-copy", &a[0])?;
            engine.list_from_slice(&elements)
        }
    }
}

fn list_set(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("list-set!", a, 3)?;
    let index = fixnum("list-set!", &a[1])?;
    if index < 0 {
        return Err(SchemeError::IndexOutOfRange);
    }
    let mut current = a[0].clone();
    for _ in 0..index {
        current = match current {
            Value::Pair(pair) => pair.borrow().cdr.clone(),
            _ => return Err(wrong("list-set!", "严格列表")),
        };
    }
    match current {
        Value::Pair(pair) => {
            pair.borrow_mut().car = a[2].clone();
            Ok(Value::Unspecified)
        }
        _ => Err(SchemeError::IndexOutOfRange),
    }
}

fn string_map(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    at_least("string-map", a, 2)?;
    let mut iter = a.iter();
    // 契约：元素数量已在前置 arity / 长度检查中确认，解包安全。
    let procedure = iter.next().expect("已检查数量").clone();
    let mut texts = Vec::new();
    for argument in iter {
        texts.push(string_cell("string-map", argument)?.borrow().clone());
    }
    let length = texts.first().map(|text| text.chars().count()).unwrap_or(0);
    if texts.iter().any(|text| text.chars().count() != length) {
        return Err(wrong("string-map", "等长字符串"));
    }
    let mut mapped = String::new();
    for index in 0..length {
        let row: Vec<Value> = texts
            .iter()
            // 契约：元素数量已在前置 arity / 长度检查中确认，解包安全。
            .map(|text| Value::Char(text.chars().nth(index).expect("已检查长度")))
            .collect();
        let outcome = super::eval::apply_procedure(engine, &procedure, &row)?;
        mapped.push(character("string-map", &outcome)?);
    }
    engine.new_string_from(mapped)
}

fn string_for_each(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    at_least("string-for-each", a, 2)?;
    let mut iter = a.iter();
    // 契约：元素数量已在前置 arity / 长度检查中确认，解包安全。
    let procedure = iter.next().expect("已检查数量").clone();
    let mut texts = Vec::new();
    for argument in iter {
        texts.push(string_cell("string-for-each", argument)?.borrow().clone());
    }
    let length = texts.first().map(|text| text.chars().count()).unwrap_or(0);
    if texts.iter().any(|text| text.chars().count() != length) {
        return Err(wrong("string-for-each", "等长字符串"));
    }
    for index in 0..length {
        let row: Vec<Value> = texts
            .iter()
            // 契约：元素数量已在前置 arity / 长度检查中确认，解包安全。
            .map(|text| Value::Char(text.chars().nth(index).expect("已检查长度")))
            .collect();
        super::eval::apply_procedure(engine, &procedure, &row)?;
    }
    Ok(Value::Unspecified)
}

fn string_to_vector(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    let (start, end) = string_range("string->vector", a, 1)?;
    let text = string_cell("string->vector", &a[0])?.borrow().clone();
    let characters: Vec<Value> = text
        .chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .map(Value::Char)
        .collect();
    engine.new_vector_from(characters)
}

fn vector_to_string(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    let (start, end) = vector_range("vector->string", a, 1)?;
    let items = match &a[0] {
        Value::Vector(items) => items.borrow().clone(),
        _ => return Err(wrong("vector->string", "向量")),
    };
    let mut text = String::new();
    for item in items.iter().skip(start).take(end.saturating_sub(start)) {
        text.push(character("vector->string", item)?);
    }
    engine.new_string_from(text)
}

fn string_copy_range(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    let (start, end) = string_range("string-copy", a, 1)?;
    let text = string_cell("string-copy", &a[0])?.borrow().clone();
    let sliced: String = text
        .chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect();
    engine.new_string_from(sliced)
}

fn string_copy_bang(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    at_least("string-copy!", a, 3)?;
    let target_start = fixnum("string-copy!", &a[1])?;
    let (start, end) = string_range("string-copy!", &a[3..], 0)?;
    let source = string_cell("string-copy!", &a[2])?.borrow().clone();
    let segment: Vec<char> = source
        .chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect();
    let target = string_cell("string-copy!", &a[0])?;
    let mut text = target.borrow_mut();
    if target_start < 0 {
        return Err(SchemeError::IndexOutOfRange);
    }
    let mut position = target_start as usize;
    for item in segment {
        let _ = text;
        let byte = match text.char_indices().nth(position) {
            Some((byte, _)) => byte,
            None => return Err(SchemeError::IndexOutOfRange),
        };
        // 契约：位置索引已在前置边界检查中确认，解包安全。
        let old = text[position..].chars().next().expect("已定位");
        text.replace_range(byte..byte + old.len_utf8(), &item.to_string());
        position += 1;
    }
    Ok(Value::Unspecified)
}

fn string_fill(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    at_least("string-fill!", a, 2)?;
    let fill = character("string-fill!", &a[1])?;
    let (start, end) = string_range("string-fill!", &a[2..], 0)?;
    let target = string_cell("string-fill!", &a[0])?;
    let mut text = target.borrow_mut();
    let mut position = 0usize;
    let mut index = 0usize;
    let characters: Vec<(usize, char)> = text.char_indices().collect();
    for (byte, item) in characters {
        if index >= start && index < end {
            let _ = position;
            text.replace_range(byte..byte + item.len_utf8(), &fill.to_string());
        }
        index += 1;
        position += 1;
    }
    Ok(Value::Unspecified)
}

fn vector_map(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    at_least("vector-map", a, 2)?;
    let mut iter = a.iter();
    // 契约：元素数量已在前置 arity / 长度检查中确认，解包安全。
    let procedure = iter.next().expect("已检查数量").clone();
    let mut rows = Vec::new();
    for argument in iter {
        rows.push(match argument {
            Value::Vector(items) => items.borrow().clone(),
            _ => return Err(wrong("vector-map", "向量")),
        });
    }
    let length = rows.first().map(Vec::len).unwrap_or(0);
    if rows.iter().any(|row| row.len() != length) {
        return Err(wrong("vector-map", "等长向量"));
    }
    let mut mapped = Vec::with_capacity(length);
    for index in 0..length {
        let row: Vec<Value> = rows.iter().map(|row| row[index].clone()).collect();
        mapped.push(super::eval::apply_procedure(engine, &procedure, &row)?);
    }
    engine.new_vector_from(mapped)
}

fn vector_for_each(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    at_least("vector-for-each", a, 2)?;
    let mut iter = a.iter();
    // 契约：元素数量已在前置 arity / 长度检查中确认，解包安全。
    let procedure = iter.next().expect("已检查数量").clone();
    let mut rows = Vec::new();
    for argument in iter {
        rows.push(match argument {
            Value::Vector(items) => items.borrow().clone(),
            _ => return Err(wrong("vector-for-each", "向量")),
        });
    }
    let length = rows.first().map(Vec::len).unwrap_or(0);
    if rows.iter().any(|row| row.len() != length) {
        return Err(wrong("vector-for-each", "等长向量"));
    }
    for index in 0..length {
        let row: Vec<Value> = rows.iter().map(|row| row[index].clone()).collect();
        super::eval::apply_procedure(engine, &procedure, &row)?;
    }
    Ok(Value::Unspecified)
}

fn vector_copy(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    let (start, end) = vector_range("vector-copy", a, 1)?;
    let items = match &a[0] {
        Value::Vector(items) => items.borrow().clone(),
        _ => return Err(wrong("vector-copy", "向量")),
    };
    let copied: Vec<Value> = items
        .into_iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect();
    engine.new_vector_from(copied)
}

fn vector_copy_bang(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    at_least("vector-copy!", a, 3)?;
    let target_start = fixnum("vector-copy!", &a[1])?;
    let (start, end) = vector_range("vector-copy!", &a[3..], 0)?;
    let source = match &a[2] {
        Value::Vector(items) => items.borrow().clone(),
        _ => return Err(wrong("vector-copy!", "向量")),
    };
    let segment: Vec<Value> = source
        .into_iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect();
    let target = match &a[0] {
        Value::Vector(items) => items.clone(),
        _ => return Err(wrong("vector-copy!", "向量")),
    };
    let mut items = target.borrow_mut();
    if target_start < 0 {
        return Err(SchemeError::IndexOutOfRange);
    }
    for (offset, value) in segment.into_iter().enumerate() {
        let index = target_start as usize + offset;
        let Some(slot) = items.get_mut(index) else {
            return Err(SchemeError::IndexOutOfRange);
        };
        *slot = value;
    }
    Ok(Value::Unspecified)
}

fn vector_fill_range(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    at_least("vector-fill!", a, 2)?;
    let fill = a[1].clone();
    let (start, end) = vector_range("vector-fill!", &a[2..], 0)?;
    let target = match &a[0] {
        Value::Vector(items) => items.clone(),
        _ => return Err(wrong("vector-fill!", "向量")),
    };
    let mut items = target.borrow_mut();
    for index in start..end.min(items.len()) {
        items[index] = fill.clone();
    }
    Ok(Value::Unspecified)
}

/// 字符串区间参数：`[start, end)`，缺省为全串。
fn string_range(
    operation: &'static str,
    arguments: &[Value],
    base: usize,
) -> Result<(usize, usize), SchemeError> {
    let text_length = match arguments.first() {
        Some(Value::String(cell)) => cell.borrow().chars().count(),
        _ => return Err(wrong(operation, "字符串")),
    };
    range_of(operation, arguments, base, text_length)
}

/// 向量区间参数：`[start, end)`，缺省为全向量。
fn vector_range(
    operation: &'static str,
    arguments: &[Value],
    base: usize,
) -> Result<(usize, usize), SchemeError> {
    let vector_length = match arguments.first() {
        Some(Value::Vector(items)) => items.borrow().len(),
        _ => return Err(wrong(operation, "向量")),
    };
    range_of(operation, arguments, base, vector_length)
}

fn range_of(
    operation: &'static str,
    arguments: &[Value],
    base: usize,
    length: usize,
) -> Result<(usize, usize), SchemeError> {
    let start = match arguments.get(base) {
        None => 0,
        Some(value) => {
            let index = fixnum(operation, value)?;
            usize::try_from(index).map_err(|_| SchemeError::IndexOutOfRange)?
        }
    };
    let end = match arguments.get(base + 1) {
        None => length,
        Some(value) => {
            let index = fixnum(operation, value)?;
            usize::try_from(index).map_err(|_| SchemeError::IndexOutOfRange)?
        }
    };
    if start > end || end > length {
        return Err(SchemeError::IndexOutOfRange);
    }
    Ok((start, end))
}

/// 大小写折叠比较（简单 Unicode 折叠；登记于兼容矩阵）。
fn folded(text: &str) -> String {
    text.to_lowercase()
}

fn compare_ci_strings(
    operation: &'static str,
    arguments: &[Value],
    ordered: CiOrdering,
) -> Result<Value, SchemeError> {
    at_least(operation, arguments, 2)?;
    for pair in arguments.windows(2) {
        let left = folded(&string_cell(operation, &pair[0])?.borrow());
        let right = folded(&string_cell(operation, &pair[1])?.borrow());
        if !ordered(left.cmp(&right)) {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}

fn compare_ci_chars(
    operation: &'static str,
    arguments: &[Value],
    ordered: CiOrdering,
) -> Result<Value, SchemeError> {
    at_least(operation, arguments, 2)?;
    for pair in arguments.windows(2) {
        let left = character(operation, &pair[0])?.to_lowercase().next().unwrap_or(' ');
        let right = character(operation, &pair[1])?.to_lowercase().next().unwrap_or(' ');
        if !ordered(left.cmp(&right)) {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}

fn digit_value(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("digit-value", a, 1)?;
    let value = character("digit-value", &a[0])?;
    match value.to_digit(10) {
        Some(digit) => Ok(Value::Fixnum(digit as i64)),
        None => Ok(Value::Bool(false)),
    }
}

// ---------- bytevector ----------

fn bytevector(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    let mut bytes = Vec::with_capacity(a.len());
    for argument in a {
        bytes.push(byte_argument("bytevector", argument)?);
    }
    engine.new_bytevector_from(bytes)
}

fn byte_argument(operation: &'static str, value: &Value) -> Result<u8, SchemeError> {
    let byte = fixnum(operation, value)?;
    if !(0..=255).contains(&byte) {
        return Err(wrong(operation, "0..255 字节"));
    }
    Ok(byte as u8)
}

fn is_bytevector(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("bytevector?", a, 1)?;
    Ok(Value::Bool(matches!(a[0], Value::Bytevector(_))))
}

fn make_bytevector(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    if a.is_empty() || a.len() > 2 {
        return Err(mismatch("make-bytevector", 1, a.len()));
    }
    let size = fixnum("make-bytevector", &a[0])?;
    if size < 0 {
        return Err(SchemeError::IndexOutOfRange);
    }
    let fill = match a.get(1) {
        Some(value) => byte_argument("make-bytevector", value)?,
        None => 0,
    };
    engine.new_bytevector_from(vec![fill; size as usize])
}

fn bytevector_cell<'a>(
    operation: &'static str,
    value: &'a Value,
) -> Result<&'a Rc<RefCell<Vec<u8>>>, SchemeError> {
    match value {
        Value::Bytevector(cell) => Ok(cell),
        _ => Err(wrong(operation, "bytevector")),
    }
}

fn bytevector_length(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("bytevector-length", a, 1)?;
    Ok(Value::Fixnum(
        bytevector_cell("bytevector-length", &a[0])?.borrow().len() as i64,
    ))
}

fn bytevector_u8_ref(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("bytevector-u8-ref", a, 2)?;
    let index = fixnum("bytevector-u8-ref", &a[1])?;
    if index < 0 {
        return Err(SchemeError::IndexOutOfRange);
    }
    bytevector_cell("bytevector-u8-ref", &a[0])?
        .borrow()
        .get(index as usize)
        .map(|byte| Value::Fixnum(*byte as i64))
        .ok_or(SchemeError::IndexOutOfRange)
}

fn bytevector_u8_set(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("bytevector-u8-set!", a, 3)?;
    let index = fixnum("bytevector-u8-set!", &a[1])?;
    let replacement = byte_argument("bytevector-u8-set!", &a[2])?;
    if index < 0 {
        return Err(SchemeError::IndexOutOfRange);
    }
    let cell = bytevector_cell("bytevector-u8-set!", &a[0])?;
    let mut bytes = cell.borrow_mut();
    let Some(slot) = bytes.get_mut(index as usize) else {
        return Err(SchemeError::IndexOutOfRange);
    };
    *slot = replacement;
    Ok(Value::Unspecified)
}

fn bytevector_copy(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    at_least("bytevector-copy", a, 1)?;
    let length = bytevector_cell("bytevector-copy", &a[0])?.borrow().len();
    let (start, end) = bytes_range("bytevector-copy", &a[1..], length)?;
    let bytes = bytevector_cell("bytevector-copy", &a[0])?
        .borrow()
        .iter()
        .skip(start)
        .take(end - start)
        .copied()
        .collect();
    engine.new_bytevector_from(bytes)
}

fn bytes_range(
    operation: &'static str,
    arguments: &[Value],
    length: usize,
) -> Result<(usize, usize), SchemeError> {
    let start = match arguments.first() {
        None => 0,
        Some(value) => usize::try_from(fixnum(operation, value)?)
            .map_err(|_| SchemeError::IndexOutOfRange)?,
    };
    let end = match arguments.get(1) {
        None => length,
        Some(value) => usize::try_from(fixnum(operation, value)?)
            .map_err(|_| SchemeError::IndexOutOfRange)?,
    };
    if start > end || end > length {
        return Err(SchemeError::IndexOutOfRange);
    }
    Ok((start, end))
}

fn bytevector_copy_bang(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    at_least("bytevector-copy!", a, 3)?;
    let target_start = usize::try_from(fixnum("bytevector-copy!", &a[1])?)
        .map_err(|_| SchemeError::IndexOutOfRange)?;
    let source_length = bytevector_cell("bytevector-copy!", &a[2])?.borrow().len();
    let (start, end) = bytes_range("bytevector-copy!", &a[3..], source_length)?;
    let segment: Vec<u8> = bytevector_cell("bytevector-copy!", &a[2])?
        .borrow()
        .iter()
        .skip(start)
        .take(end - start)
        .copied()
        .collect();
    let target = bytevector_cell("bytevector-copy!", &a[0])?;
    let mut bytes = target.borrow_mut();
    for (offset, byte) in segment.into_iter().enumerate() {
        let index = target_start + offset;
        let Some(slot) = bytes.get_mut(index) else {
            return Err(SchemeError::IndexOutOfRange);
        };
        *slot = byte;
    }
    Ok(Value::Unspecified)
}

fn bytevector_append(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    let mut combined = Vec::new();
    for argument in a {
        combined.extend(bytevector_cell("bytevector-append", argument)?.borrow().iter().copied());
    }
    engine.new_bytevector_from(combined)
}

fn u8_list_to_bytevector(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("u8-list->bytevector", a, 1)?;
    let elements = engine.value_to_vec("u8-list->bytevector", &a[0])?;
    let mut bytes = Vec::with_capacity(elements.len());
    for element in &elements {
        bytes.push(byte_argument("u8-list->bytevector", element)?);
    }
    engine.new_bytevector_from(bytes)
}

fn bytevector_to_u8_list(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("bytevector->u8-list", a, 1)?;
    let bytes = bytevector_cell("bytevector->u8-list", &a[0])?.borrow().clone();
    let items: Vec<Value> = bytes.into_iter().map(|byte| Value::Fixnum(byte as i64)).collect();
    engine.list_from_slice(&items)
}

fn utf8_to_string(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    at_least("utf8->string", a, 1)?;
    let length = bytevector_cell("utf8->string", &a[0])?.borrow().len();
    let (start, end) = bytes_range("utf8->string", &a[1..], length)?;
    let bytes: Vec<u8> = bytevector_cell("utf8->string", &a[0])?
        .borrow()
        .iter()
        .skip(start)
        .take(end - start)
        .copied()
        .collect();
    let text = String::from_utf8(bytes)
        .map_err(|_| SchemeError::InvalidSyntax {
            form: "utf8->string",
            reason: "无效 UTF-8 序列",
        })?;
    engine.new_string_from(text)
}

fn string_to_utf8(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    at_least("string->utf8", a, 1)?;
    let text = string_cell("string->utf8", &a[0])?.borrow().clone();
    let (start, end) = string_range("string->utf8", &a[1..], 0)?;
    let segment: String = text
        .chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect();
    engine.new_bytevector_from(segment.into_bytes())
}

// ---------- 异常 / 多值 / lazy / 参数 ----------

fn is_error_object(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("error-object?", a, 1)?;
    Ok(Value::Bool(matches!(a[0], Value::ErrorObject(_))))
}

fn error_object_message(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("error-object-message", a, 1)?;
    match &a[0] {
        Value::ErrorObject(error) => engine.new_string_from(error.message().to_string()),
        _ => Err(wrong("error-object-message", "error 对象")),
    }
}

fn error_object_irritants(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("error-object-irritants", a, 1)?;
    match &a[0] {
        Value::ErrorObject(error) => engine.list_from_slice(error.irritants()),
        _ => Err(wrong("error-object-irritants", "error 对象")),
    }
}

fn is_file_error(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("file-error?", a, 1)?;
    Ok(Value::Bool(matches!(
        &a[0],
        Value::ErrorObject(error) if error.kind() == ErrorKind::File
    )))
}

fn is_read_error(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("read-error?", a, 1)?;
    Ok(Value::Bool(matches!(
        &a[0],
        Value::ErrorObject(error) if error.kind() == ErrorKind::Read
    )))
}

fn make_parameter(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    if a.is_empty() || a.len() > 2 {
        return Err(mismatch("make-parameter", 1, a.len()));
    }
    let converter = match a.get(1) {
        Some(value) => Some(value.clone()),
        None => None,
    };
    let initial = match &converter {
        Some(procedure) => super::eval::apply_procedure(engine, procedure, &[a[0].clone()])?,
        None => a[0].clone(),
    };
    engine.new_parameter(super::value::ParameterState { converter, current: initial })
}

fn force(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("force", a, 1)?;
    let cell = match &a[0] {
        Value::Promise(cell) => cell.clone(),
        other => return Ok(other.clone()),
    };
    let thunk = match &*cell.borrow() {
        PromiseState::Forced(value) => return Ok(value.clone()),
        PromiseState::Pending(thunk) => thunk.clone(),
    };
    // 先落位为 Forced 防御递归 force；完成后写回真实值。
    *cell.borrow_mut() = PromiseState::Forced(Value::Unspecified);
    let outcome = super::eval::apply_procedure(engine, &thunk, &[])?;
    *cell.borrow_mut() = PromiseState::Forced(outcome.clone());
    Ok(outcome)
}

fn make_promise(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("make-promise", a, 1)?;
    match &a[0] {
        Value::Promise(_) => Ok(a[0].clone()),
        other => engine.new_promise(PromiseState::Forced(other.clone())),
    }
}

fn is_promise(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("promise?", a, 1)?;
    Ok(Value::Bool(matches!(a[0], Value::Promise(_))))
}

fn param_ref(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity(" param-ref", a, 1)?;
    match &a[0] {
        Value::Parameter(cell) => Ok(cell.borrow().current.clone()),
        _ => Err(wrong(" param-ref", "参数对象")),
    }
}

fn param_set(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity(" param-set!", a, 2)?;
    match &a[0] {
        Value::Parameter(cell) => {
            cell.borrow_mut().current = a[1].clone();
            Ok(Value::Unspecified)
        }
        _ => Err(wrong(" param-set!", "参数对象")),
    }
}

// ---------- 端口（内存字符串限定） ----------

fn open_input_string(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("open-input-string", a, 1)?;
    let text = string_cell("open-input-string", &a[0])?.borrow().clone();
    engine.new_port(PortState::Input {
        text,
        position: 0,
        closed: false,
    })
}

fn open_output_string(engine: &mut SchemeEngine, _a: &[Value]) -> Result<Value, SchemeError> {
    engine.new_port(PortState::Output {
        text: String::new(),
        closed: false,
    })
}

fn get_output_string(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("get-output-string", a, 1)?;
    match &a[0] {
        Value::Port(cell) => match &*cell.borrow() {
            PortState::Output { text, .. } => {
                let text = text.clone();
                engine.new_string_from(text)
            }
            _ => Err(wrong("get-output-string", "输出端口")),
        },
        _ => Err(wrong("get-output-string", "输出端口")),
    }
}

fn close_port(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("close-port", a, 1)?;
    match &a[0] {
        Value::Port(cell) => {
            let mut state = cell.borrow_mut();
            match &mut *state {
                PortState::Input { closed, .. } | PortState::Output { closed, .. } => {
                    *closed = true;
                }
            }
            Ok(Value::Unspecified)
        }
        _ => Err(wrong("close-port", "端口")),
    }
}

fn is_port(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("port?", a, 1)?;
    Ok(Value::Bool(matches!(a[0], Value::Port(_))))
}

fn is_input_port(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("input-port?", a, 1)?;
    Ok(Value::Bool(matches!(
        &a[0],
        Value::Port(cell) if matches!(&*cell.borrow(), PortState::Input { .. })
    )))
}

fn is_output_port(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("output-port?", a, 1)?;
    Ok(Value::Bool(matches!(
        &a[0],
        Value::Port(cell) if matches!(&*cell.borrow(), PortState::Output { .. })
    )))
}

fn input_port<'a>(
    operation: &'static str,
    value: &'a Value,
) -> Result<Rc<RefCell<PortState>>, SchemeError> {
    match value {
        Value::Port(cell) if matches!(&*cell.borrow(), PortState::Input { .. }) => Ok(cell.clone()),
        _ => Err(wrong(operation, "输入端口")),
    }
}

fn output_port<'a>(
    operation: &'static str,
    value: &'a Value,
) -> Result<Rc<RefCell<PortState>>, SchemeError> {
    match value {
        Value::Port(cell) if matches!(&*cell.borrow(), PortState::Output { .. }) => Ok(cell.clone()),
        _ => Err(wrong(operation, "输出端口")),
    }
}

fn port_closed(cell: &RefCell<PortState>) -> bool {
    match &*cell.borrow() {
        PortState::Input { closed, .. } | PortState::Output { closed, .. } => *closed,
    }
}

fn read_char(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    let cell = input_port("read-char", port_argument("read-char", a)?)?;
    if port_closed(&cell) {
        return Err(wrong("read-char", "未关闭端口"));
    }
    let mut state = cell.borrow_mut();
    let PortState::Input { text, position, .. } = &mut *state else {
        return Err(wrong("read-char", "输入端口"));
    };
    match text[*position..].chars().next() {
        Some(next) => {
            *position += next.len_utf8();
            Ok(Value::Char(next))
        }
        None => Ok(Value::Eof),
    }
}

fn peek_char(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    let cell = input_port("peek-char", port_argument("peek-char", a)?)?;
    let state = cell.borrow();
    let PortState::Input { text, position, .. } = &*state else {
        return Err(wrong("peek-char", "输入端口"));
    };
    Ok(text[*position..].chars().next().map_or(Value::Eof, Value::Char))
}

fn read_line(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    let cell = input_port("read-line", port_argument("read-line", a)?)?;
    let mut state = cell.borrow_mut();
    let PortState::Input { text, position, .. } = &mut *state else {
        return Err(wrong("read-line", "输入端口"));
    };
    if *position >= text.len() {
        return Ok(Value::Eof);
    }
    let rest = &text[*position..];
    match rest.find('\n') {
        Some(newline) => {
            let line = rest[..newline].to_string();
            *position += newline + 1;
            engine.new_string_from(line)
        }
        None => {
            let line = rest.to_string();
            *position = text.len();
            engine.new_string_from(line)
        }
    }
}

fn read_string(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    if a.is_empty() || a.len() > 2 {
        return Err(mismatch("read-string", 2, a.len()));
    }
    let count = usize::try_from(fixnum("read-string", &a[0])?)
        .map_err(|_| SchemeError::IndexOutOfRange)?;
    let cell = input_port("read-string", port_argument("read-string", &a[1..])?)?;
    let mut state = cell.borrow_mut();
    let PortState::Input { text, position, .. } = &mut *state else {
        return Err(wrong("read-string", "输入端口"));
    };
    let available: String = text[*position..].chars().take(count).collect();
    *position += available.len();
    if available.is_empty() {
        Ok(Value::Eof)
    } else {
        engine.new_string_from(available)
    }
}

fn read_u8(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    let cell = input_port("read-u8", port_argument("read-u8", a)?)?;
    let mut state = cell.borrow_mut();
    let PortState::Input { text, position, .. } = &mut *state else {
        return Err(wrong("read-u8", "输入端口"));
    };
    match text.as_bytes().get(*position) {
        Some(byte) => {
            *position += 1;
            Ok(Value::Fixnum(*byte as i64))
        }
        None => Ok(Value::Eof),
    }
}

fn peek_u8(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    let cell = input_port("peek-u8", port_argument("peek-u8", a)?)?;
    let state = cell.borrow();
    let PortState::Input { text, position, .. } = &*state else {
        return Err(wrong("peek-u8", "输入端口"));
    };
    Ok(text
        .as_bytes()
        .get(*position)
        .map_or(Value::Eof, |byte| Value::Fixnum(*byte as i64)))
}

fn char_ready(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("char-ready?", a, 1)?;
    input_port("char-ready?", &a[0])?;
    // 内存端口非关闭即就绪。
    Ok(Value::Bool(true))
}

fn u8_ready(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("u8-ready?", a, 1)?;
    input_port("u8-ready?", &a[0])?;
    Ok(Value::Bool(true))
}

/// `read`：从输入端口解析下一个 datum；EOF 返回 eof 对象。
fn read_datum(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    let cell = input_port("read", port_argument("read", a)?)?;
    if port_closed(&cell) {
        return Err(wrong("read", "未关闭端口"));
    }
    let rest = {
        let state = cell.borrow();
        let PortState::Input { text, position, .. } = &*state else {
            return Err(wrong("read", "输入端口"));
        };
        text[*position..].to_string()
    };
    match super::reader::read_one(engine, &rest)? {
        Some((datum, consumed_bytes)) => {
            let mut state = cell.borrow_mut();
            let PortState::Input { position, .. } = &mut *state else {
                return Err(wrong("read", "输入端口"));
            };
            *position += consumed_bytes;
            Ok(datum)
        }
        None => {
            let mut state = cell.borrow_mut();
            let PortState::Input { text, position, .. } = &mut *state else {
                return Err(wrong("read", "输入端口"));
            };
            *position = text.len();
            Ok(Value::Eof)
        }
    }
}

fn port_argument<'a>(operation: &'static str, arguments: &'a [Value]) -> Result<&'a Value, SchemeError> {
    arguments.first().ok_or_else(|| wrong(operation, "端口"))
}

fn write_string(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    at_least("write-string", a, 1)?;
    let text = string_cell("write-string", &a[0])?.borrow().clone();
    write_text_to_port("write-string", a.get(1), &text)?;
    Ok(Value::Unspecified)
}

fn write_char(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    at_least("write-char", a, 1)?;
    let item = character("write-char", &a[0])?;
    write_text_to_port("write-char", a.get(1), &item.to_string())?;
    Ok(Value::Unspecified)
}

fn write_u8(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    at_least("write-u8", a, 1)?;
    let byte = byte_argument("write-u8", &a[0])?;
    let port = match a.get(1) {
        Some(value) => value,
        None => return Err(wrong("write-u8", "输出端口")),
    };
    let cell = output_port("write-u8", port)?;
    if port_closed(&cell) {
        return Err(wrong("write-u8", "未关闭端口"));
    }
    let mut state = cell.borrow_mut();
    let PortState::Output { text, .. } = &mut *state else {
        return Err(wrong("write-u8", "输出端口"));
    };
    text.push(byte as char);
    Ok(Value::Unspecified)
}

fn write_bytevector(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    at_least("write-bytevector", a, 1)?;
    let bytes = bytevector_cell("write-bytevector", &a[0])?.borrow().clone();
    let port = match a.get(1) {
        Some(value) => value,
        None => return Err(wrong("write-bytevector", "输出端口")),
    };
    let cell = output_port("write-bytevector", port)?;
    if port_closed(&cell) {
        return Err(wrong("write-bytevector", "未关闭端口"));
    }
    let mut state = cell.borrow_mut();
    let PortState::Output { text, .. } = &mut *state else {
        return Err(wrong("write-bytevector", "输出端口"));
    };
    for byte in bytes {
        text.push(byte as char);
    }
    Ok(Value::Unspecified)
}

fn write_text_to_port(
    operation: &'static str,
    port: Option<&Value>,
    text: &str,
) -> Result<(), SchemeError> {
    let Some(port) = port else {
        return Err(wrong(operation, "输出端口"));
    };
    let cell = output_port(operation, port)?;
    if port_closed(&cell) {
        return Err(wrong(operation, "未关闭端口"));
    }
    let mut state = cell.borrow_mut();
    let PortState::Output { text: sink, .. } = &mut *state else {
        return Err(wrong(operation, "输出端口"));
    };
    sink.push_str(text);
    Ok(())
}

fn write_value(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    at_least("write", a, 1)?;
    let representation = a[0].to_write_string();
    write_text_to_port("write", a.get(1), &representation)?;
    Ok(Value::Unspecified)
}

fn display_value(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    at_least("display", a, 1)?;
    let representation = match &a[0] {
        Value::String(cell) => cell.borrow().clone(),
        Value::Char(item) => item.to_string(),
        other => other.to_write_string(),
    };
    write_text_to_port("display", a.get(1), &representation)?;
    Ok(Value::Unspecified)
}

fn newline(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    write_text_to_port("newline", a.first(), "\n")?;
    Ok(Value::Unspecified)
}

/// `current-output-port`：引擎实例级内存输出端口；宿主经诊断出口读取。
fn current_output_port(engine: &mut SchemeEngine, _a: &[Value]) -> Result<Value, SchemeError> {
    engine.default_output_port()
}

fn eof_object(_e: &mut SchemeEngine, _a: &[Value]) -> Result<Value, SchemeError> {
    Ok(Value::Eof)
}

fn is_eof(_e: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("eof-object?", a, 1)?;
    Ok(Value::Bool(matches!(a[0], Value::Eof)))
}

// ---------- 受限 eval ----------

fn eval_restricted(engine: &mut SchemeEngine, a: &[Value]) -> Result<Value, SchemeError> {
    if a.is_empty() || a.len() > 2 {
        return Err(mismatch("eval", 1, a.len()));
    }
    if let Some(environment) = a.get(1)
        && !matches!(environment, Value::Bool(false) | Value::Unspecified)
    {
        return Err(wrong("eval", "(interaction-environment) 或省略"));
    }
    let globals = engine.globals_snapshot();
    super::eval::eval(engine, &a[0], &globals)
}

fn interaction_environment(_e: &mut SchemeEngine, _a: &[Value]) -> Result<Value, SchemeError> {
    // 受限宿主环境：交互环境以 #f 标记（eval 接受）。
    Ok(Value::Bool(false))
}

// ---------- (scheme cxr) ----------

macro_rules! cxr {
    ($name:ident, [$($accessor:ident),+]) => {
        fn $name(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
            exact_arity(stringify!($name), arguments, 1)?;
            let mut current = arguments[0].clone();
            $(
                current = match current {
                    Value::Pair(pair) => pair.borrow().$accessor.clone(),
                    _ => {
                        return Err(wrong(stringify!($name), "非空序对"));
                    }
                };
            )+
            Ok(current)
        }
    };
}

cxr!(caar, [car, car]);
cxr!(cadr, [car, cdr]);
cxr!(cdar, [cdr, car]);
cxr!(cddr, [cdr, cdr]);
cxr!(caaar, [car, car, car]);
cxr!(caadr, [car, car, cdr]);
cxr!(cadar, [car, cdr, car]);
cxr!(caddr, [car, cdr, cdr]);
cxr!(cdaar, [cdr, car, car]);
cxr!(cdadr, [cdr, car, cdr]);
cxr!(cddar, [cdr, cdr, car]);
cxr!(cdddr, [cdr, cdr, cdr]);
cxr!(caaaar, [car, car, car, car]);
cxr!(caaadr, [car, car, car, cdr]);
cxr!(caadar, [car, car, cdr, car]);
cxr!(caaddr, [car, car, cdr, cdr]);
cxr!(cadaar, [car, cdr, car, car]);
cxr!(cadadr, [car, cdr, car, cdr]);
cxr!(caddar, [car, cdr, cdr, car]);
cxr!(cadddr, [car, cdr, cdr, cdr]);
cxr!(cdaaar, [cdr, car, car, car]);
cxr!(cdaadr, [cdr, car, car, cdr]);
cxr!(cdadar, [cdr, car, cdr, car]);
cxr!(cdaddr, [cdr, car, cdr, cdr]);
cxr!(cddaar, [cdr, cdr, car, car]);
cxr!(cddadr, [cdr, cdr, car, cdr]);
cxr!(cdddar, [cdr, cdr, cdr, car]);
cxr!(cddddr, [cdr, cdr, cdr, cdr]);

fn install_cxr(engine: &mut SchemeEngine) {
    engine.define_primitive("caar", caar);
    engine.define_primitive("cadr", cadr);
    engine.define_primitive("cdar", cdar);
    engine.define_primitive("cddr", cddr);
    engine.define_primitive("caaar", caaar);
    engine.define_primitive("caadr", caadr);
    engine.define_primitive("cadar", cadar);
    engine.define_primitive("caddr", caddr);
    engine.define_primitive("cdaar", cdaar);
    engine.define_primitive("cdadr", cdadr);
    engine.define_primitive("cddar", cddar);
    engine.define_primitive("cdddr", cdddr);
    engine.define_primitive("caaaar", caaaar);
    engine.define_primitive("caaadr", caaadr);
    engine.define_primitive("caadar", caadar);
    engine.define_primitive("caaddr", caaddr);
    engine.define_primitive("cadaar", cadaar);
    engine.define_primitive("cadadr", cadadr);
    engine.define_primitive("caddar", caddar);
    engine.define_primitive("cadddr", cadddr);
    engine.define_primitive("cdaaar", cdaaar);
    engine.define_primitive("cdaadr", cdaadr);
    engine.define_primitive("cdadar", cdadar);
    engine.define_primitive("cdaddr", cdaddr);
    engine.define_primitive("cddaar", cddaar);
    engine.define_primitive("cddadr", cddadr);
    engine.define_primitive("cdddar", cdddar);
    engine.define_primitive("cddddr", cddddr);
}
