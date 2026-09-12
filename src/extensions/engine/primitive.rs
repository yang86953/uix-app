//! M1 原语集：数值（fixnum / flonum 子集）、序对与列表、字符串、字符、
//! 向量、谓词与控制过程。
//!
//! 数值塔（bigint / rational、精确跨类型比较）按合同在 M2 交付；跨精确
//! 类型比较统一提升为 f64 是有意为之的临时语义。所有原语对参数数量与
//! 类型做显式检查，任何调用都以类型化错误失败，绝不 panic。

use std::cell::RefCell;
use std::rc::Rc;

use num_bigint::BigInt;
use num_integer::Integer as _;
use num_traits::Zero as _;
use num_rational::BigRational;

use super::error::SchemeError;
use super::eval::ControlOp;
use super::value::{ErrorKind, ErrorObject, Value};

/// equal? 系 Rust 递归的栈安全深度上限；与机器帧数上限解耦。
const MAX_EQUAL_DEPTH: u32 = 256;
use super::number::{self, Number};
use super::SchemeEngine;

/// 把全部 M1 原语注册进全局环境。
pub(crate) fn install(engine: &mut SchemeEngine) {
    // 数值
    engine.define_primitive("+", add);
    engine.define_primitive("-", subtract);
    engine.define_primitive("*", multiply);
    engine.define_primitive("/", divide);
    engine.define_primitive("=", numeric_compare_equal);
    engine.define_primitive("<", numeric_compare_less);
    engine.define_primitive("<=", numeric_compare_less_equal);
    engine.define_primitive(">", numeric_compare_greater);
    engine.define_primitive(">=", numeric_compare_greater_equal);
    engine.define_primitive("quotient", quotient);
    engine.define_primitive("remainder", remainder);
    engine.define_primitive("modulo", modulo);
    engine.define_primitive("abs", absolute);
    engine.define_primitive("min", minimum);
    engine.define_primitive("max", maximum);
    engine.define_primitive("floor", floor_value);
    engine.define_primitive("ceiling", ceiling_value);
    engine.define_primitive("truncate", truncate_value);
    engine.define_primitive("round", round_value);
    engine.define_primitive("sqrt", sqrt_value);
    engine.define_primitive("expt", expt);
    engine.define_primitive("exp", exp_value);
    engine.define_primitive("log", log_value);
    engine.define_primitive("sin", sin_value);
    engine.define_primitive("cos", cos_value);
    engine.define_primitive("tan", tan_value);
    engine.define_primitive("atan", atan_value);
    engine.define_primitive("gcd", gcd);
    engine.define_primitive("lcm", lcm);
    engine.define_primitive("zero?", is_zero);
    engine.define_primitive("positive?", is_positive);
    engine.define_primitive("negative?", is_negative);
    engine.define_primitive("even?", is_even);
    engine.define_primitive("odd?", is_odd);
    engine.define_primitive("nan?", is_nan);
    engine.define_primitive("number?", is_number);
    engine.define_primitive("integer?", is_integer);
    engine.define_primitive("rational?", is_rational);
    engine.define_primitive("real?", is_real);
    engine.define_primitive("exact?", is_exact);
    engine.define_primitive("inexact?", is_inexact);
    engine.define_primitive("exact->inexact", exact_to_inexact);
    engine.define_primitive("inexact->exact", inexact_to_exact);
    engine.define_primitive("exact", inexact_to_exact);
    engine.define_primitive("inexact", exact_to_inexact);
    engine.define_primitive("number->string", number_to_string);
    engine.define_primitive("string->number", string_to_number);

    // 序对与列表
    engine.define_primitive("cons", cons);
    engine.define_primitive("car", car);
    engine.define_primitive("cdr", cdr);
    engine.define_primitive("set-car!", set_car);
    engine.define_primitive("set-cdr!", set_cdr);
    engine.define_primitive("null?", is_null);
    engine.define_primitive("pair?", is_pair);
    engine.define_primitive("list?", is_list_value);
    engine.define_primitive("list", list);
    engine.define_primitive("length", length);
    engine.define_primitive("append", append);
    engine.define_primitive("reverse", reverse);
    engine.define_primitive("list-tail", list_tail);
    engine.define_primitive("list-ref", list_ref);
    engine.define_primitive("memq", memq);
    engine.define_primitive("memv", memq);
    engine.define_primitive("member", member);
    engine.define_primitive("assq", assq);
    engine.define_primitive("assv", assq);
    engine.define_primitive("assoc", assoc);

    // 谓词与符号
    engine.define_primitive("eq?", is_eq);
    engine.define_primitive("eqv?", is_eq);
    engine.define_primitive("equal?", is_equal);
    engine.define_primitive("not", not);
    engine.define_primitive("boolean?", is_boolean);
    engine.define_primitive("symbol?", is_symbol);
    engine.define_primitive("procedure?", is_procedure);
    engine.define_primitive("symbol->string", symbol_to_string);
    engine.define_primitive("string->symbol", string_to_symbol);

    // 字符串
    engine.define_primitive("string?", is_string);
    engine.define_primitive("make-string", make_string);
    engine.define_primitive("string", string_constructor);
    engine.define_primitive("string-length", string_length);
    engine.define_primitive("string-ref", string_ref);
    engine.define_primitive("string-set!", string_set);
    engine.define_primitive("string=?", string_equal);
    engine.define_primitive("string<?", string_less);
    engine.define_primitive("string>?", string_greater);
    engine.define_primitive("string<=?", string_less_equal);
    engine.define_primitive("string>=?", string_greater_equal);
    engine.define_primitive("substring", substring);
    engine.define_primitive("string-append", string_append);
    engine.define_primitive("string->list", string_to_list);
    engine.define_primitive("list->string", list_to_string);
    engine.define_primitive("string-copy", string_copy);
    engine.define_primitive("string-upcase", string_upcase);
    engine.define_primitive("string-downcase", string_downcase);

    // 字符
    engine.define_primitive("char?", is_char);
    engine.define_primitive("char->integer", char_to_integer);
    engine.define_primitive("integer->char", integer_to_char);
    engine.define_primitive("char=?", char_equal);
    engine.define_primitive("char<?", char_less);
    engine.define_primitive("char>?", char_greater);
    engine.define_primitive("char<=?", char_less_equal);
    engine.define_primitive("char>=?", char_greater_equal);
    engine.define_primitive("char-upcase", char_upcase);
    engine.define_primitive("char-downcase", char_downcase);
    engine.define_primitive("char-alphabetic?", char_alphabetic);
    engine.define_primitive("char-numeric?", char_numeric);
    engine.define_primitive("char-whitespace?", char_whitespace);

    // 向量
    engine.define_primitive("vector?", is_vector);
    engine.define_primitive("make-vector", make_vector);
    engine.define_primitive("vector", vector);
    engine.define_primitive("vector-length", vector_length);
    engine.define_primitive("vector-ref", vector_ref);
    engine.define_primitive("vector-set!", vector_set);
    engine.define_primitive("vector->list", vector_to_list);
    engine.define_primitive("list->vector", list_to_vector);
    engine.define_primitive("vector-fill!", vector_fill);

    // 控制原语：机器内建（捕获帧栈 / wind 链），作为一等值传递。
    engine.define_control("call/cc", ControlOp::CallCc);
    engine.define_control("call-with-current-continuation", ControlOp::CallCc);
    engine.define_control("dynamic-wind", ControlOp::DynamicWind);

    // define-record-type 的内部支撑原语；名称带前导空格，reader 无法产生，
    // 用户代码不能直接引用（R7RS record 无字段级隐藏，无权限破坏）。
    engine.define_primitive(" make-record", record_make);
    engine.define_primitive(" record-ref", record_ref);
    engine.define_primitive(" record-set!", record_set);
    engine.define_primitive(" record-pred", record_pred);

    // 控制与失败（map / for-each / apply 为机器控制原语：回调内 continuation
    // 作用于调用方机器，逃逸语义正确）
    engine.define_control("map", ControlOp::Map);
    engine.define_control("for-each", ControlOp::ForEach);
    engine.define_control("apply", ControlOp::Apply);
    engine.define_primitive("error", error);

    // (uix extension) 宿主库：扩展身份与命令登记。
    engine.define_primitive("extension-id", extension_identity_id);
    engine.define_primitive("extension-version", extension_identity_version);
    engine.define_primitive("register-command!", register_command);
    engine.define_primitive("register-handler!", register_handler);
    engine.define_primitive("register-state-export!", register_state_export);
    engine.define_primitive("register-state-import!", register_state_import);

    // 标准库补充（数值缺口、bytevector、端口、lazy、参数、多值、异常、cxr）。
    super::primitive_ext::install(engine);
}

// ---------- 参数解构与 arity 守卫 ----------

pub(crate) fn wrong(operation: &'static str, expected: &'static str) -> SchemeError {
    SchemeError::WrongType { operation, expected }
}

pub(crate) fn mismatch(operation: &'static str, expected: usize, actual: usize) -> SchemeError {
    SchemeError::ArityMismatch {
        procedure: operation.to_string(),
        expected: expected.to_string(),
        actual,
    }
}

pub(crate) fn exact_arity(
    operation: &'static str,
    arguments: &[Value],
    expected: usize,
) -> Result<(), SchemeError> {
    if arguments.len() != expected {
        return Err(mismatch(operation, expected, arguments.len()));
    }
    Ok(())
}

pub(crate) fn at_least(
    operation: &'static str,
    arguments: &[Value],
    minimum: usize,
) -> Result<(), SchemeError> {
    if arguments.len() < minimum {
        return Err(SchemeError::ArityMismatch {
            procedure: operation.to_string(),
            expected: format!("至少 {minimum}"),
            actual: arguments.len(),
        });
    }
    Ok(())
}

pub(crate) fn fixnum(operation: &'static str, value: &Value) -> Result<i64, SchemeError> {
    Number::view(value)
        .and_then(|number| number.to_i64())
        .ok_or_else(|| wrong(operation, "整数"))
}

pub(crate) fn string_cell<'a>(
    operation: &'static str,
    value: &'a Value,
) -> Result<&'a Rc<RefCell<String>>, SchemeError> {
    match value {
        Value::String(cell) => Ok(cell),
        _ => Err(wrong(operation, "字符串")),
    }
}

pub(crate) fn character(operation: &'static str, value: &Value) -> Result<char, SchemeError> {
    match value {
        Value::Char(character) => Ok(*character),
        _ => Err(wrong(operation, "字符")),
    }
}

fn symbol_name(operation: &'static str, value: &Value) -> Result<Rc<str>, SchemeError> {
    match value {
        Value::Symbol(name) => Ok(name.clone()),
        _ => Err(wrong(operation, "符号")),
    }
}

/// 把字符索引 `[start, end)` 转换为字节边界。
fn char_bounds(text: &str, start: usize, end: usize) -> Result<(usize, usize), SchemeError> {
    let length = text.chars().count();
    if start > end || end > length {
        return Err(SchemeError::IndexOutOfRange);
    }
    let start_byte = text
        .char_indices()
        .nth(start)
        .map(|(byte, _)| byte)
        .unwrap_or(text.len());
    let end_byte = text
        .char_indices()
        .nth(end)
        .map(|(byte, _)| byte)
        .unwrap_or(text.len());
    Ok((start_byte, end_byte))
}

// ---------- 数值 ----------

/// 数值视图解构；非数值按类型错误拒绝。
pub(crate) fn number_of(operation: &'static str, value: &Value) -> Result<Number, SchemeError> {
    Number::view(value).ok_or_else(|| wrong(operation, "数值"))
}

/// 整数 BigInt 视图；非整数按类型错误拒绝。
pub(crate) fn integer_big(operation: &'static str, value: &Value) -> Result<BigInt, SchemeError> {
    let number = number_of(operation, value)?;
    if !number.is_integer() {
        return Err(wrong(operation, "整数"));
    }
    Ok(number.big())
}

fn fold(
    operation: &'static str,
    arguments: &[Value],
    unit: Number,
    step: fn(Number, Number) -> Result<Number, SchemeError>,
) -> Result<Value, SchemeError> {
    let mut accumulator = unit;
    for argument in arguments {
        accumulator = step(accumulator, number_of(operation, argument)?)?;
    }
    Ok(accumulator.to_value())
}

fn add(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    fold("+", arguments, Number::Fixnum(0), Number::add)
}

fn multiply(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    fold("*", arguments, Number::Fixnum(1), Number::mul)
}

fn subtract(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    at_least("-", arguments, 1)?;
    if arguments.len() == 1 {
        return number_of("-", &arguments[0])?.negate().map(|n| n.to_value());
    }
    let mut accumulator = number_of("-", &arguments[0])?;
    for argument in &arguments[1..] {
        accumulator = accumulator.sub(number_of("-", argument)?)?;
    }
    Ok(accumulator.to_value())
}

fn divide(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    at_least("/", arguments, 1)?;
    if arguments.len() == 1 {
        return Number::Fixnum(1)
            .div(number_of("/", &arguments[0])?)
            .map(|n| n.to_value());
    }
    let mut accumulator = number_of("/", &arguments[0])?;
    for argument in &arguments[1..] {
        accumulator = accumulator.div(number_of("/", argument)?)?;
    }
    Ok(accumulator.to_value())
}

fn compare_chain(
    operation: &'static str,
    arguments: &[Value],
    ordered: fn(std::cmp::Ordering) -> bool,
) -> Result<Value, SchemeError> {
    at_least(operation, arguments, 2)?;
    for pair in arguments.windows(2) {
        let left = number_of(operation, &pair[0])?;
        let right = number_of(operation, &pair[1])?;
        let ordering = left.compare(&right).ok_or_else(|| SchemeError::InvalidNumber {
            text: left.to_text(),
        })?;
        if !ordered(ordering) {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}

fn numeric_compare_equal(
    _engine: &mut SchemeEngine,
    arguments: &[Value],
) -> Result<Value, SchemeError> {
    at_least("=", arguments, 2)?;
    for pair in arguments.windows(2) {
        let left = number_of("=", &pair[0])?;
        let right = number_of("=", &pair[1])?;
        if !left.numeric_equal(&right) {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}

fn numeric_compare_less(
    _engine: &mut SchemeEngine,
    arguments: &[Value],
) -> Result<Value, SchemeError> {
    compare_chain("<", arguments, |ordering| {
        ordering == std::cmp::Ordering::Less
    })
}

fn numeric_compare_less_equal(
    _engine: &mut SchemeEngine,
    arguments: &[Value],
) -> Result<Value, SchemeError> {
    compare_chain("<=", arguments, |ordering| {
        ordering != std::cmp::Ordering::Greater
    })
}

fn numeric_compare_greater(
    _engine: &mut SchemeEngine,
    arguments: &[Value],
) -> Result<Value, SchemeError> {
    compare_chain(">", arguments, |ordering| {
        ordering == std::cmp::Ordering::Greater
    })
}

fn numeric_compare_greater_equal(
    _engine: &mut SchemeEngine,
    arguments: &[Value],
) -> Result<Value, SchemeError> {
    compare_chain(">=", arguments, |ordering| {
        ordering != std::cmp::Ordering::Less
    })
}

fn quotient(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("quotient", arguments, 2)?;
    let left = integer_big("quotient", &arguments[0])?;
    let right = integer_big("quotient", &arguments[1])?;
    if right.is_zero() {
        return Err(SchemeError::DivisionByZero);
    }
    Ok(Number::norm_bignum(left / right).to_value())
}

fn remainder(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("remainder", arguments, 2)?;
    let left = integer_big("remainder", &arguments[0])?;
    let right = integer_big("remainder", &arguments[1])?;
    if right.is_zero() {
        return Err(SchemeError::DivisionByZero);
    }
    // 截断语义（符号跟被除数），与 Rust 的 BigInt `%` 一致。
    Ok(Number::norm_bignum(left % right).to_value())
}

fn modulo(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("modulo", arguments, 2)?;
    let left = integer_big("modulo", &arguments[0])?;
    let right = integer_big("modulo", &arguments[1])?;
    if right.is_zero() {
        return Err(SchemeError::DivisionByZero);
    }
    // 数学 modulo：符号跟随除数。
    Ok(Number::norm_bignum(left.mod_floor(&right)).to_value())
}

fn absolute(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("abs", arguments, 1)?;
    number_of("abs", &arguments[0])?.abs().map(|n| n.to_value())
}

fn pick_extreme(
    operation: &'static str,
    arguments: &[Value],
    keep: fn(std::cmp::Ordering) -> bool,
) -> Result<Value, SchemeError> {
    at_least(operation, arguments, 1)?;
    let mut best = number_of(operation, &arguments[0])?;
    for candidate in &arguments[1..] {
        let candidate = number_of(operation, candidate)?;
        match best.compare(&candidate) {
            Some(ordering) if keep(ordering) => best = candidate,
            Some(_) => {}
            None => {
                return Err(SchemeError::InvalidNumber {
                    text: candidate.to_text(),
                })
            }
        }
    }
    Ok(best.to_value())
}

fn minimum(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    pick_extreme("min", arguments, |ordering| {
        ordering == std::cmp::Ordering::Greater
    })
}

fn maximum(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    pick_extreme("max", arguments, |ordering| ordering == std::cmp::Ordering::Less)
}

/// 有理数取整到偶（R7RS round 语义）。
fn round_rational_to_even(value: &BigRational) -> BigRational {
    let floor = value.floor();
    let floor_integer = floor.to_integer();
    let difference = value - &floor;
    let half = BigRational::new(BigInt::from(1), BigInt::from(2));
    match difference.cmp(&half) {
        std::cmp::Ordering::Less => floor,
        std::cmp::Ordering::Equal => {
            if floor_integer.clone() % BigInt::from(2) == BigInt::zero() {
                floor
            } else {
                BigRational::from_integer(floor_integer + 1)
            }
        }
        std::cmp::Ordering::Greater => BigRational::from_integer(floor_integer + 1),
    }
}

fn rounding(
    operation: &'static str,
    arguments: &[Value],
    exact_step: fn(&BigRational) -> BigRational,
    float_step: fn(f64) -> f64,
) -> Result<Value, SchemeError> {
    exact_arity(operation, arguments, 1)?;
    let number = number_of(operation, &arguments[0])?;
    Ok(match number {
        Number::Flonum(value) => Number::Flonum(float_step(value)).to_value(),
        exact => {
            let rational = exact.rational();
            if rational.is_integer() {
                exact.to_value()
            } else {
                Number::norm_rational(exact_step(&rational)).to_value()
            }
        }
    })
}

fn floor_value(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    rounding("floor", arguments, |value| value.floor(), f64::floor)
}

fn ceiling_value(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    rounding("ceiling", arguments, |value| value.ceil(), f64::ceil)
}

fn truncate_value(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    rounding("truncate", arguments, |value| value.trunc(), f64::trunc)
}

fn round_value(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    rounding("round", arguments, round_rational_to_even, f64::round_ties_even)
}

fn sqrt_value(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("sqrt", arguments, 1)?;
    number_of("sqrt", &arguments[0])?.sqrt().map(|n| n.to_value())
}

fn expt(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("expt", arguments, 2)?;
    let base = number_of("expt", &arguments[0])?;
    let power = number_of("expt", &arguments[1])?;
    base.pow(&power).map(|n| n.to_value())
}

fn unary_float(
    operation: &'static str,
    arguments: &[Value],
    apply: fn(f64) -> f64,
) -> Result<Value, SchemeError> {
    exact_arity(operation, arguments, 1)?;
    Ok(Value::Flonum(apply(
        number_of(operation, &arguments[0])?.to_f64(),
    )))
}

fn exp_value(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    unary_float("exp", arguments, f64::exp)
}

fn log_value(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    if arguments.len() == 2 {
        let value = number_of("log", &arguments[0])?.to_f64();
        let base = number_of("log", &arguments[1])?.to_f64();
        return Ok(Value::Flonum(value.ln() / base.ln()));
    }
    unary_float("log", arguments, f64::ln)
}

fn sin_value(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    unary_float("sin", arguments, f64::sin)
}

fn cos_value(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    unary_float("cos", arguments, f64::cos)
}

fn tan_value(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    unary_float("tan", arguments, f64::tan)
}

fn atan_value(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    if arguments.len() == 2 {
        let y = number_of("atan", &arguments[0])?.to_f64();
        let x = number_of("atan", &arguments[1])?.to_f64();
        return Ok(Value::Flonum(y.atan2(x)));
    }
    unary_float("atan", arguments, f64::atan)
}

fn gcd(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    let mut accumulator = BigInt::zero();
    for argument in arguments {
        let value = integer_big("gcd", argument)?;
        accumulator = num_integer::gcd(accumulator, value);
    }
    Ok(Number::norm_bignum(accumulator).to_value())
}

fn lcm(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    let mut accumulator = BigInt::from(1);
    for argument in arguments {
        let value = integer_big("lcm", argument)?;
        if value.is_zero() {
            return Ok(Value::Fixnum(0));
        }
        accumulator = num_integer::lcm(accumulator, value);
    }
    Ok(Number::norm_bignum(accumulator).to_value())
}

fn unary_predicate(
    operation: &'static str,
    arguments: &[Value],
    predicate: impl Fn(&Value) -> Result<bool, SchemeError>,
) -> Result<Value, SchemeError> {
    exact_arity(operation, arguments, 1)?;
    Ok(Value::Bool(predicate(&arguments[0])?))
}

fn is_zero(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    unary_predicate("zero?", arguments, |value| {
        Ok(number_of("zero?", value)?.sign() == Some(0))
    })
}

fn is_positive(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    unary_predicate("positive?", arguments, |value| {
        Ok(number_of("positive?", value)?.sign() == Some(1))
    })
}

fn is_negative(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    unary_predicate("negative?", arguments, |value| {
        Ok(number_of("negative?", value)?.sign() == Some(-1))
    })
}

fn is_even(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    unary_predicate("even?", arguments, |value| {
        Ok(integer_big("even?", value)? % BigInt::from(2) == BigInt::zero())
    })
}

fn is_odd(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    unary_predicate("odd?", arguments, |value| {
        Ok(integer_big("odd?", value)? % BigInt::from(2) != BigInt::zero())
    })
}

fn is_nan(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    unary_predicate("nan?", arguments, |value| {
        Ok(number_of("nan?", value)?.is_nan())
    })
}

fn is_number(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    unary_predicate("number?", arguments, |value| Ok(value.is_number()))
}

fn is_integer(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    unary_predicate("integer?", arguments, |value| {
        Ok(Number::view(value).is_some_and(|n| n.is_integer()))
    })
}

fn is_rational(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    unary_predicate("rational?", arguments, |value| {
        Ok(Number::view(value).is_some_and(|n| n.is_rational()))
    })
}

fn is_real(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    unary_predicate("real?", arguments, |value| Ok(value.is_number()))
}

fn is_exact(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    unary_predicate("exact?", arguments, |value| {
        Ok(number_of("exact?", value)?.is_exact())
    })
}

fn is_inexact(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    unary_predicate("inexact?", arguments, |value| {
        Ok(number_of("inexact?", value)?.is_inexact())
    })
}

fn exact_to_inexact(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("exact->inexact", arguments, 1)?;
    number_of("exact->inexact", &arguments[0])?
        .to_inexact()
        .map(|n| n.to_value())
}

fn inexact_to_exact(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("inexact->exact", arguments, 1)?;
    number_of("inexact->exact", &arguments[0])?
        .to_exact()
        .map(|n| n.to_value())
}

fn number_to_string(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    if arguments.is_empty() || arguments.len() > 2 {
        return Err(mismatch("number->string", 1, arguments.len()));
    }
    let radix = match arguments.get(1) {
        None => 10,
        Some(value) => number_of("number->string", value)?
            .to_i64()
            .filter(|radix| matches!(radix, 2 | 8 | 10 | 16))
            .ok_or_else(|| wrong("number->string", "进制 2 / 8 / 10 / 16"))?,
    };
    let number = number_of("number->string", &arguments[0])?;
    let text = match &number {
        Number::Fixnum(value) if radix == 10 => value.to_string(),
        Number::Flonum(value) if radix == 10 => Number::Flonum(*value).to_text(),
        Number::Rational(value) if radix == 10 => {
            format!("{}/{}", value.numer(), value.denom())
        }
        exact => exact.big().to_str_radix(u32::try_from(radix).map_err(|_| wrong("number->string", "进制"))?),
    };
    engine.new_string_from(text)
}

fn string_to_number(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    if arguments.is_empty() || arguments.len() > 2 {
        return Err(mismatch("string->number", 1, arguments.len()));
    }
    let text = string_cell("string->number", &arguments[0])?.borrow().clone();
    let radix = match arguments.get(1) {
        None => 10,
        Some(value) => number_of("string->number", value)?
            .to_i64()
            .and_then(|radix| u32::try_from(radix).ok())
            .ok_or_else(|| wrong("string->number", "进制"))?,
    };
    match number::parse_literal(&text, radix, false, false) {
        Some(number) => Ok(number.to_value()),
        None => Ok(Value::Bool(false)),
    }
}

// ---------- 序对与列表 ----------

fn cons(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("cons", arguments, 2)?;
    engine.new_pair(arguments[0].clone(), arguments[1].clone())
}

fn car(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("car", arguments, 1)?;
    match &arguments[0] {
        Value::Pair(pair) => Ok(pair.borrow().car.clone()),
        _ => Err(wrong("car", "非空序对")),
    }
}

fn cdr(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("cdr", arguments, 1)?;
    match &arguments[0] {
        Value::Pair(pair) => Ok(pair.borrow().cdr.clone()),
        _ => Err(wrong("cdr", "非空序对")),
    }
}

fn set_car(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("set-car!", arguments, 2)?;
    match &arguments[0] {
        Value::Pair(pair) => {
            pair.borrow_mut().car = arguments[1].clone();
            Ok(Value::Unspecified)
        }
        _ => Err(wrong("set-car!", "非空序对")),
    }
}

fn set_cdr(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("set-cdr!", arguments, 2)?;
    match &arguments[0] {
        Value::Pair(pair) => {
            pair.borrow_mut().cdr = arguments[1].clone();
            Ok(Value::Unspecified)
        }
        _ => Err(wrong("set-cdr!", "非空序对")),
    }
}

fn is_null(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("null?", arguments, 1)?;
    Ok(Value::Bool(matches!(arguments[0], Value::Null)))
}

fn is_pair(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("pair?", arguments, 1)?;
    Ok(Value::Bool(matches!(arguments[0], Value::Pair(_))))
}

fn is_list_value(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("list?", arguments, 1)?;
    Ok(Value::Bool(arguments[0].is_list()))
}

fn list(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    engine.list_from_slice(arguments)
}

fn length(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("length", arguments, 1)?;
    let mut count = 0_i64;
    let mut current = arguments[0].clone();
    loop {
        match current {
            Value::Null => return Ok(Value::Fixnum(count)),
            Value::Pair(pair) => {
                count += 1;
                current = pair.borrow().cdr.clone();
            }
            _ => return Err(wrong("length", "严格列表")),
        }
    }
}

fn append(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    let Some((last, heads)) = arguments.split_last() else {
        return Ok(Value::Null);
    };
    let mut result = last.clone();
    for head in heads.iter().rev() {
        let elements = engine.value_to_vec("append", head)?;
        for element in elements.into_iter().rev() {
            result = engine.new_pair(element, result)?;
        }
    }
    Ok(result)
}

fn reverse(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("reverse", arguments, 1)?;
    let elements = engine.value_to_vec("reverse", &arguments[0])?;
    let mut result = Value::Null;
    for element in elements {
        result = engine.new_pair(element, result)?;
    }
    Ok(result)
}

fn walk_list_index(
    operation: &'static str,
    list: &Value,
    index: i64,
) -> Result<Value, SchemeError> {
    if index < 0 {
        return Err(SchemeError::IndexOutOfRange);
    }
    let mut current = list.clone();
    for _ in 0..index {
        current = match current {
            Value::Pair(pair) => pair.borrow().cdr.clone(),
            _ => return Err(wrong(operation, "严格列表")),
        };
    }
    Ok(current)
}

fn list_tail(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("list-tail", arguments, 2)?;
    let index = fixnum("list-tail", &arguments[1])?;
    walk_list_index("list-tail", &arguments[0], index)
}

fn list_ref(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("list-ref", arguments, 2)?;
    let index = fixnum("list-ref", &arguments[1])?;
    match walk_list_index("list-ref", &arguments[0], index)? {
        Value::Pair(pair) => Ok(pair.borrow().car.clone()),
        _ => Err(SchemeError::IndexOutOfRange),
    }
}

/// member 系；`deep` 为 true 时用递归 equal?。
fn member_like(
    operation: &'static str,
    engine: &mut SchemeEngine,
    arguments: &[Value],
    deep: bool,
) -> Result<Value, SchemeError> {
    let _ = engine;
    exact_arity(operation, arguments, 2)?;
    let maximum = MAX_EQUAL_DEPTH;
    let mut current = arguments[1].clone();
    loop {
        match current {
            Value::Null => return Ok(Value::Bool(false)),
            Value::Pair(pair) => {
                let borrowed = pair.borrow();
                let matched = if deep {
                    equal_values(&arguments[0], &borrowed.car, maximum, 0)?
                } else {
                    arguments[0] == borrowed.car
                };
                if matched {
                    return Ok(Value::Pair(pair.clone()));
                }
                current = borrowed.cdr.clone();
            }
            _ => return Err(wrong(operation, "严格列表")),
        }
    }
}

fn memq(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    member_like("memq", engine, arguments, false)
}

fn member(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    member_like("member", engine, arguments, true)
}

/// assoc 系；`deep` 为 true 时用递归 equal?。
fn assoc_like(
    operation: &'static str,
    engine: &mut SchemeEngine,
    arguments: &[Value],
    deep: bool,
) -> Result<Value, SchemeError> {
    let _ = engine;
    exact_arity(operation, arguments, 2)?;
    let maximum = MAX_EQUAL_DEPTH;
    let mut current = arguments[1].clone();
    loop {
        match current {
            Value::Null => return Ok(Value::Bool(false)),
            Value::Pair(pair) => {
                let borrowed = pair.borrow();
                let entry = match &borrowed.car {
                    Value::Pair(entry) => entry.borrow().car.clone(),
                    _ => return Err(wrong(operation, "元素必须是序对")),
                };
                let matched = if deep {
                    equal_values(&arguments[0], &entry, maximum, 0)?
                } else {
                    arguments[0] == entry
                };
                if matched {
                    return Ok(borrowed.car.clone());
                }
                current = borrowed.cdr.clone();
            }
            _ => return Err(wrong(operation, "严格列表")),
        }
    }
}

fn assq(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    assoc_like("assq", engine, arguments, false)
}

fn assoc(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    assoc_like("assoc", engine, arguments, true)
}

/// 递归 equal?；带深度上限，环形结构以类型化错误终止。
pub(crate) fn equal_values(
    left: &Value,
    right: &Value,
    maximum: u32,
    depth: u32,
) -> Result<bool, SchemeError> {
    if depth > maximum {
        return Err(SchemeError::DepthLimitExceeded { maximum });
    }
    match (left, right) {
        (Value::String(left), Value::String(right)) => Ok(*left.borrow() == *right.borrow()),
        (Value::Pair(left), Value::Pair(right)) => {
            let (left_car, left_cdr) = {
                let borrowed = left.borrow();
                (borrowed.car.clone(), borrowed.cdr.clone())
            };
            let (right_car, right_cdr) = {
                let borrowed = right.borrow();
                (borrowed.car.clone(), borrowed.cdr.clone())
            };
            Ok(equal_values(&left_car, &right_car, maximum, depth + 1)?
                && equal_values(&left_cdr, &right_cdr, maximum, depth + 1)?)
        }
        (Value::Vector(left), Value::Vector(right)) => {
            let left_items = left.borrow().clone();
            let right_items = right.borrow().clone();
            if left_items.len() != right_items.len() {
                return Ok(false);
            }
            for (left_item, right_item) in left_items.iter().zip(right_items.iter()) {
                if !equal_values(left_item, right_item, maximum, depth + 1)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (Value::Record(left), Value::Record(right)) => {
            let left_record = left.borrow();
            let right_record = right.borrow();
            if left_record.type_name() != right_record.type_name()
                || left_record.fields().len() != right_record.fields().len()
            {
                return Ok(false);
            }
            for (left_item, right_item) in left_record
                .fields()
                .iter()
                .zip(right_record.fields().iter())
            {
                if !equal_values(left_item, right_item, maximum, depth + 1)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(left == right),
    }
}

// ---------- 谓词与符号 ----------

fn binary_predicate(
    operation: &'static str,
    arguments: &[Value],
    predicate: impl Fn(&Value, &Value) -> Result<bool, SchemeError>,
) -> Result<Value, SchemeError> {
    exact_arity(operation, arguments, 2)?;
    Ok(Value::Bool(predicate(&arguments[0], &arguments[1])?))
}

fn is_eq(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    binary_predicate("eq?", arguments, |left, right| Ok(left == right))
}

fn is_equal(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    let maximum = MAX_EQUAL_DEPTH;
    let _ = engine;
    binary_predicate("equal?", arguments, |left, right| {
        equal_values(left, right, maximum, 0)
    })
}

fn not(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("not", arguments, 1)?;
    Ok(Value::Bool(arguments[0].is_false()))
}

fn is_boolean(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("boolean?", arguments, 1)?;
    Ok(Value::Bool(matches!(arguments[0], Value::Bool(_))))
}

fn is_symbol(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("symbol?", arguments, 1)?;
    Ok(Value::Bool(matches!(arguments[0], Value::Symbol(_))))
}

fn is_procedure(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("procedure?", arguments, 1)?;
    Ok(Value::Bool(matches!(
        arguments[0],
        Value::Closure(_)
            | Value::Primitive(_)
            | Value::Control(_)
            | Value::Continuation(_)
            | Value::Host(_)
    )))
}

fn symbol_to_string(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("symbol->string", arguments, 1)?;
    let name = symbol_name("symbol->string", &arguments[0])?;
    engine.new_string_from(name.to_string())
}

fn string_to_symbol(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("string->symbol", arguments, 1)?;
    let text = string_cell("string->symbol", &arguments[0])?.borrow().clone();
    Ok(Value::Symbol(text.as_str().into()))
}

// ---------- 字符串 ----------

fn is_string(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("string?", arguments, 1)?;
    Ok(Value::Bool(matches!(arguments[0], Value::String(_))))
}

fn make_string(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    if arguments.is_empty() || arguments.len() > 2 {
        return Err(mismatch("make-string", 1, arguments.len()));
    }
    let size = fixnum("make-string", &arguments[0])?;
    if size < 0 {
        return Err(SchemeError::IndexOutOfRange);
    }
    let fill = match arguments.get(1) {
        Some(value) => character("make-string", value)?,
        None => ' ',
    };
    engine.new_string_with_fill(size as usize, fill)
}

fn string_constructor(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    let mut text = String::new();
    for argument in arguments {
        text.push(character("string", argument)?);
    }
    engine.new_string_from(text)
}

fn string_length(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("string-length", arguments, 1)?;
    let text = string_cell("string-length", &arguments[0])?.borrow();
    Ok(Value::Fixnum(text.chars().count() as i64))
}

fn string_ref(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("string-ref", arguments, 2)?;
    let index = fixnum("string-ref", &arguments[1])?;
    if index < 0 {
        return Err(SchemeError::IndexOutOfRange);
    }
    let cell = string_cell("string-ref", &arguments[0])?;
    let text = cell.borrow();
    match text.chars().nth(index as usize) {
        Some(character) => Ok(Value::Char(character)),
        None => Err(SchemeError::IndexOutOfRange),
    }
}

fn string_set(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("string-set!", arguments, 3)?;
    let index = fixnum("string-set!", &arguments[1])?;
    if index < 0 {
        return Err(SchemeError::IndexOutOfRange);
    }
    let replacement = character("string-set!", &arguments[2])?;
    let cell = string_cell("string-set!", &arguments[0])?;
    let mut text = cell.borrow_mut();
    match text.char_indices().nth(index as usize) {
        Some((byte, old)) => {
            text.replace_range(byte..byte + old.len_utf8(), &replacement.to_string());
            Ok(Value::Unspecified)
        }
        None => Err(SchemeError::IndexOutOfRange),
    }
}

fn compare_strings(
    operation: &'static str,
    arguments: &[Value],
    ordered: fn(std::cmp::Ordering) -> bool,
) -> Result<Value, SchemeError> {
    at_least(operation, arguments, 2)?;
    for pair in arguments.windows(2) {
        let left = string_cell(operation, &pair[0])?.borrow().clone();
        let right = string_cell(operation, &pair[1])?.borrow().clone();
        if !ordered(left.cmp(&right)) {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}

fn string_equal(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    compare_strings("string=?", arguments, |ordering| {
        ordering == std::cmp::Ordering::Equal
    })
}

fn string_less(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    compare_strings("string<?", arguments, |ordering| {
        ordering == std::cmp::Ordering::Less
    })
}

fn string_greater(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    compare_strings("string>?", arguments, |ordering| {
        ordering == std::cmp::Ordering::Greater
    })
}

fn string_less_equal(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    compare_strings("string<=?", arguments, |ordering| {
        ordering != std::cmp::Ordering::Greater
    })
}

fn string_greater_equal(
    _engine: &mut SchemeEngine,
    arguments: &[Value],
) -> Result<Value, SchemeError> {
    compare_strings("string>=?", arguments, |ordering| {
        ordering != std::cmp::Ordering::Less
    })
}

fn substring(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("substring", arguments, 3)?;
    let start = fixnum("substring", &arguments[1])?;
    let end = fixnum("substring", &arguments[2])?;
    if start < 0 || end < start {
        return Err(SchemeError::IndexOutOfRange);
    }
    let text = string_cell("substring", &arguments[0])?.borrow().clone();
    let (start_byte, end_byte) = char_bounds(&text, start as usize, end as usize)?;
    engine.new_string_from(text[start_byte..end_byte].to_string())
}

fn string_append(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    let mut combined = String::new();
    for argument in arguments {
        combined.push_str(&string_cell("string-append", argument)?.borrow());
    }
    engine.new_string_from(combined)
}

fn string_to_list(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("string->list", arguments, 1)?;
    let text = string_cell("string->list", &arguments[0])?.borrow().clone();
    let characters: Vec<Value> = text.chars().map(Value::Char).collect();
    engine.list_from_slice(&characters)
}

fn list_to_string(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("list->string", arguments, 1)?;
    let elements = engine.value_to_vec("list->string", &arguments[0])?;
    let mut text = String::new();
    for element in elements {
        text.push(character("list->string", &element)?);
    }
    engine.new_string_from(text)
}

fn string_copy(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("string-copy", arguments, 1)?;
    let text = string_cell("string-copy", &arguments[0])?.borrow().clone();
    engine.new_string_from(text)
}

/// 完整 Unicode 案例映射（Rust 标准库按 Unicode Standard 实现）。
fn unicode_case(text: &str, upper: bool) -> String {
    if upper {
        text.to_uppercase()
    } else {
        text.to_lowercase()
    }
}

fn string_upcase(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("string-upcase", arguments, 1)?;
    let text = string_cell("string-upcase", &arguments[0])?.borrow().clone();
    engine.new_string_from(unicode_case(&text, true))
}

fn string_downcase(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("string-downcase", arguments, 1)?;
    let text = string_cell("string-downcase", &arguments[0])?.borrow().clone();
    engine.new_string_from(unicode_case(&text, false))
}

// ---------- 字符 ----------

fn is_char(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("char?", arguments, 1)?;
    Ok(Value::Bool(matches!(arguments[0], Value::Char(_))))
}

fn char_to_integer(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("char->integer", arguments, 1)?;
    Ok(Value::Fixnum(character("char->integer", &arguments[0])? as i64))
}

fn integer_to_char(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("integer->char", arguments, 1)?;
    let code = fixnum("integer->char", &arguments[0])?;
    u32::try_from(code)
        .ok()
        .and_then(char::from_u32)
        .map(Value::Char)
        .ok_or(SchemeError::IndexOutOfRange)
}

fn compare_chars(
    operation: &'static str,
    arguments: &[Value],
    ordered: fn(std::cmp::Ordering) -> bool,
) -> Result<Value, SchemeError> {
    at_least(operation, arguments, 2)?;
    for pair in arguments.windows(2) {
        let left = character(operation, &pair[0])?;
        let right = character(operation, &pair[1])?;
        if !ordered(left.cmp(&right)) {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}

fn char_equal(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    compare_chars("char=?", arguments, |ordering| {
        ordering == std::cmp::Ordering::Equal
    })
}

fn char_less(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    compare_chars("char<?", arguments, |ordering| {
        ordering == std::cmp::Ordering::Less
    })
}

fn char_greater(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    compare_chars("char>?", arguments, |ordering| {
        ordering == std::cmp::Ordering::Greater
    })
}

fn char_less_equal(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    compare_chars("char<=?", arguments, |ordering| {
        ordering != std::cmp::Ordering::Greater
    })
}

fn char_greater_equal(
    _engine: &mut SchemeEngine,
    arguments: &[Value],
) -> Result<Value, SchemeError> {
    compare_chars("char>=?", arguments, |ordering| {
        ordering != std::cmp::Ordering::Less
    })
}

/// 单字符 Unicode 案例映射；一对多映射的字符按 R7RS 保持原样。
fn unicode_char_case(value: char, upper: bool) -> char {
    let mapped: Vec<char> = if upper {
        value.to_uppercase().collect()
    } else {
        value.to_lowercase().collect()
    };
    if mapped.len() == 1 {
        mapped[0]
    } else {
        value
    }
}

fn char_upcase(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("char-upcase", arguments, 1)?;
    Ok(Value::Char(unicode_char_case(
        character("char-upcase", &arguments[0])?,
        true,
    )))
}

fn char_downcase(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("char-downcase", arguments, 1)?;
    Ok(Value::Char(unicode_char_case(
        character("char-downcase", &arguments[0])?,
        false,
    )))
}

fn char_alphabetic(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("char-alphabetic?", arguments, 1)?;
    Ok(Value::Bool(character("char-alphabetic?", &arguments[0])?.is_alphabetic()))
}

fn char_numeric(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("char-numeric?", arguments, 1)?;
    Ok(Value::Bool(character("char-numeric?", &arguments[0])?.is_numeric()))
}

fn char_whitespace(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("char-whitespace?", arguments, 1)?;
    Ok(Value::Bool(
        character("char-whitespace?", &arguments[0])?.is_whitespace(),
    ))
}

// ---------- 向量 ----------

fn is_vector(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("vector?", arguments, 1)?;
    Ok(Value::Bool(matches!(arguments[0], Value::Vector(_))))
}

fn make_vector(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    if arguments.is_empty() || arguments.len() > 2 {
        return Err(mismatch("make-vector", 1, arguments.len()));
    }
    let size = fixnum("make-vector", &arguments[0])?;
    if size < 0 {
        return Err(SchemeError::IndexOutOfRange);
    }
    let fill = arguments.get(1).cloned().unwrap_or(Value::Unspecified);
    engine.new_vector_with_fill(size as usize, fill)
}

fn vector(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    engine.new_vector_from(arguments.to_vec())
}

fn vector_length(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("vector-length", arguments, 1)?;
    match &arguments[0] {
        Value::Vector(items) => Ok(Value::Fixnum(items.borrow().len() as i64)),
        _ => Err(wrong("vector-length", "向量")),
    }
}

fn vector_ref(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("vector-ref", arguments, 2)?;
    let index = fixnum("vector-ref", &arguments[1])?;
    if index < 0 {
        return Err(SchemeError::IndexOutOfRange);
    }
    match &arguments[0] {
        Value::Vector(items) => items
            .borrow()
            .get(index as usize)
            .cloned()
            .ok_or(SchemeError::IndexOutOfRange),
        _ => Err(wrong("vector-ref", "向量")),
    }
}

fn vector_set(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("vector-set!", arguments, 3)?;
    let index = fixnum("vector-set!", &arguments[1])?;
    if index < 0 {
        return Err(SchemeError::IndexOutOfRange);
    }
    match &arguments[0] {
        Value::Vector(items) => {
            let mut items = items.borrow_mut();
            let slot = items
                .get_mut(index as usize)
                .ok_or(SchemeError::IndexOutOfRange)?;
            *slot = arguments[2].clone();
            Ok(Value::Unspecified)
        }
        _ => Err(wrong("vector-set!", "向量")),
    }
}

fn vector_to_list(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("vector->list", arguments, 1)?;
    match &arguments[0] {
        Value::Vector(items) => {
            let items = items.borrow().clone();
            engine.list_from_slice(&items)
        }
        _ => Err(wrong("vector->list", "向量")),
    }
}

fn list_to_vector(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("list->vector", arguments, 1)?;
    let elements = engine.value_to_vec("list->vector", &arguments[0])?;
    engine.new_vector_from(elements)
}

fn vector_fill(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("vector-fill!", arguments, 2)?;
    match &arguments[0] {
        Value::Vector(items) => {
            let mut items = items.borrow_mut();
            for slot in items.iter_mut() {
                *slot = arguments[1].clone();
            }
            Ok(Value::Unspecified)
        }
        _ => Err(wrong("vector-fill!", "向量")),
    }
}

// ---------- 控制与失败 ----------

fn error(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    at_least("error", arguments, 1)?;
    let message = string_cell("error", &arguments[0])?.borrow().clone();
    let irritants = arguments[1..].to_vec();
    let object = engine.new_error_object(ErrorObject::new(
        message,
        irritants,
        ErrorKind::User,
    ))?;
    Err(SchemeError::Raised(object))
}

// ---------- (uix extension) 宿主库 ----------

/// 命令名字符集：小写字母、数字、连字符、下划线；长度 1..=64。
fn valid_command_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

fn extension_identity_id(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("extension-id", arguments, 0)?;
    match engine.extension_identity() {
        Some((id, _)) => engine.new_string_from(id.to_string()),
        None => Ok(Value::Bool(false)),
    }
}

fn extension_identity_version(
    engine: &mut SchemeEngine,
    arguments: &[Value],
) -> Result<Value, SchemeError> {
    exact_arity("extension-version", arguments, 0)?;
    match engine.extension_identity() {
        Some((_, version)) => engine.new_string_from(version.to_string()),
        None => Ok(Value::Bool(false)),
    }
}

/// `(register-handler! "name" procedure)`：把 UI 事件处理过程登记进
/// ui-event 命名空间；面板事件的代际校验后经引擎回投。
fn register_handler(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    register_namespaced(engine, arguments, "ui-event")
}

/// `(register-state-export! proc)`：热替换静止点导出扩展私有状态
/// （无参过程，返回跨边界 owned 兼容值）。
fn register_state_export(
    engine: &mut SchemeEngine,
    arguments: &[Value],
) -> Result<Value, SchemeError> {
    register_single_procedure(engine, arguments, "state-export", "register-state-export!")
}

/// `(register-state-import! proc)`：候选代接收旧代状态快照
/// （单参过程；失败使替换回退）。
fn register_state_import(
    engine: &mut SchemeEngine,
    arguments: &[Value],
) -> Result<Value, SchemeError> {
    register_single_procedure(engine, arguments, "state-import", "register-state-import!")
}

/// 状态过程登记：每命名空间最多一个。
fn register_single_procedure(
    engine: &mut SchemeEngine,
    arguments: &[Value],
    namespace: &str,
    form: &'static str,
) -> Result<Value, SchemeError> {
    exact_arity(form, arguments, 1)?;
    match &arguments[0] {
        Value::Closure(_) | Value::Primitive(_) | Value::Control(_) | Value::Continuation(_)
        | Value::Host(_) => {}
        _ => return Err(wrong(form, "可调用过程")),
    }
    if engine
        .host_registrations()
        .iter()
        .any(|registration| registration.namespace == namespace)
    {
        return Err(SchemeError::InvalidSyntax {
            form,
            reason: "状态过程已登记",
        });
    }
    engine.register_host_value(namespace, "state", arguments[0].clone())?;
    Ok(Value::Unspecified)
}

/// `(register-command! "name" procedure)`：把命令过程登记进宿主命令表。
/// 过程留在引擎内，宿主调用经引擎回投，闭包不跨边界。
fn register_command(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity("register-command!", arguments, 2)?;
    let name = string_cell("register-command!", &arguments[0])?.borrow().clone();
    if !valid_command_name(&name) {
        return Err(wrong("register-command!", "小写字母/数字/连字符/下划线的命令名"));
    }
    match &arguments[1] {
        Value::Closure(_) | Value::Primitive(_) | Value::Control(_) | Value::Continuation(_)
        | Value::Host(_) => {}
        other => {
            let _ = other;
            return Err(wrong("register-command!", "可调用过程"));
        }
    }
    engine.register_host_value("command", &name, arguments[1].clone())?;
    Ok(Value::Unspecified)
}

/// 命名空间化登记的共享实现。
fn register_namespaced(
    engine: &mut SchemeEngine,
    arguments: &[Value],
    namespace: &str,
) -> Result<Value, SchemeError> {
    exact_arity("register-handler!", arguments, 2)?;
    let name = string_cell("register-handler!", &arguments[0])?.borrow().clone();
    if !valid_command_name(&name) {
        return Err(wrong("register-handler!", "小写字母/数字/连字符/下划线的名称"));
    }
    match &arguments[1] {
        Value::Closure(_) | Value::Primitive(_) | Value::Control(_) | Value::Continuation(_)
        | Value::Host(_) => {}
        _ => return Err(wrong("register-handler!", "可调用过程")),
    }
    engine.register_host_value(namespace, &name, arguments[1].clone())?;
    Ok(Value::Unspecified)
}


// ---------- record 内部支撑 ----------

fn record_symbol(operation: &'static str, value: &Value) -> Result<Rc<str>, SchemeError> {
    symbol_name(operation, value)
}

fn record_ref(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity(" record-ref", arguments, 3)?;
    let type_name = record_symbol(" record-ref", &arguments[1])?;
    let index = fixnum(" record-ref", &arguments[2])?;
    match &arguments[0] {
        Value::Record(record) => {
            let record = record.borrow();
            if record.type_name() != &type_name {
                return Err(wrong(" record-ref", "匹配的 record 类型"));
            }
            if index < 0 {
                return Err(SchemeError::IndexOutOfRange);
            }
            record
                .fields()
                .get(index as usize)
                .cloned()
                .ok_or(SchemeError::IndexOutOfRange)
        }
        _ => Err(wrong(" record-ref", "record 实例")),
    }
}

fn record_set(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity(" record-set!", arguments, 4)?;
    let type_name = record_symbol(" record-set!", &arguments[1])?;
    let index = fixnum(" record-set!", &arguments[2])?;
    match &arguments[0] {
        Value::Record(record) => {
            let mut record = record.borrow_mut();
            if record.type_name() != &type_name {
                return Err(wrong(" record-set!", "匹配的 record 类型"));
            }
            if index < 0 {
                return Err(SchemeError::IndexOutOfRange);
            }
            if record.set_field(index as usize, arguments[3].clone()) {
                Ok(Value::Unspecified)
            } else {
                Err(SchemeError::IndexOutOfRange)
            }
        }
        _ => Err(wrong(" record-set!", "record 实例")),
    }
}

fn record_pred(_engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    exact_arity(" record-pred", arguments, 2)?;
    let type_name = record_symbol(" record-pred", &arguments[1])?;
    Ok(Value::Bool(match &arguments[0] {
        Value::Record(record) => record.borrow().type_name() == &type_name,
        _ => false,
    }))
}

/// `( make-record 'type ctor-value... indices-vector)`：按构造参数顺序与
/// 声明索引映射创建 record；未被构造覆盖的字段为未指定值。
fn record_make(engine: &mut SchemeEngine, arguments: &[Value]) -> Result<Value, SchemeError> {
    if arguments.len() < 2 {
        return Err(mismatch(" make-record", 2, arguments.len()));
    }
    let type_name = record_symbol(" make-record", &arguments[0])?;
    let indices = match &arguments[1] {
        Value::Vector(indices) => indices.borrow().clone(),
        _ => return Err(wrong(" make-record", "索引向量")),
    };
    let mut slots: Vec<Value> = vec![Value::Unspecified; indices.len()];
    for (position, index) in indices.iter().enumerate() {
        let declared = fixnum(" make-record", index)? as usize;
        let value = arguments
            .get(2 + position)
            .cloned()
            .ok_or_else(|| mismatch(" make-record", 3 + indices.len(), arguments.len()))?;
        if declared >= slots.len() {
            return Err(SchemeError::IndexOutOfRange);
        }
        slots[declared] = value;
    }
    engine.new_record(type_name, slots)
}
