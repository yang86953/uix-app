//! 数值塔：fixnum / bignum / rational / flonum 四层表示与运算。
//!
//! `num-bigint` / `num-rational` 在本模块私有适配，不泄漏到解释器其他部分
//! 或上层合同。规范化保证：bignum 落入 i64 范围即折叠为 fixnum，有理数
//! 分母为 1 即折叠为整数——同值数值永远只有一种 exact 表示，eqv? 因此
//! 可以按 variant 直接比较。

use std::cmp::Ordering;
use std::rc::Rc;

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Num as _, Signed, ToPrimitive, Zero};

use super::error::SchemeError;
use super::value::Value;

/// 数值塔中的单个数值视图；`Value::Fixnum` / `Bignum` / `Rational` /
/// `Flonum` 的统一运算入口。
#[derive(Clone)]
pub(crate) enum Number {
    Fixnum(i64),
    Bignum(Rc<BigInt>),
    Rational(Rc<BigRational>),
    Flonum(f64),
}

#[allow(clippy::wrong_self_convention)]
impl Number {
    /// 借用视图：避免克隆 `Rc` 即可参与运算。
    pub(crate) fn view(value: &Value) -> Option<Self> {
        match value {
            Value::Fixnum(number) => Some(Self::Fixnum(*number)),
            Value::Bignum(number) => Some(Self::Bignum(number.clone())),
            Value::Rational(number) => Some(Self::Rational(number.clone())),
            Value::Flonum(number) => Some(Self::Flonum(*number)),
            _ => None,
        }
    }

    pub(crate) fn to_value(self) -> Value {
        match self {
            Self::Fixnum(number) => Value::Fixnum(number),
            Self::Bignum(number) => Value::Bignum(number),
            Self::Rational(number) => Value::Rational(number),
            Self::Flonum(number) => Value::Flonum(number),
        }
    }

    pub(crate) fn is_exact(&self) -> bool {
        !matches!(self, Self::Flonum(_))
    }

    pub(crate) fn is_inexact(&self) -> bool {
        matches!(self, Self::Flonum(_))
    }

    pub(crate) fn is_integer(&self) -> bool {
        match self {
            Self::Fixnum(_) | Self::Bignum(_) => true,
            Self::Rational(number) => number.is_integer(),
            Self::Flonum(number) => number.is_finite() && number.fract() == 0.0,
        }
    }

    pub(crate) fn is_rational(&self) -> bool {
        match self {
            Self::Flonum(number) => number.is_finite(),
            _ => true,
        }
    }

    pub(crate) fn is_nan(&self) -> bool {
        matches!(self, Self::Flonum(number) if number.is_nan())
    }

    /// 尽力转换为 i64；越界或非整数返回 `None`。
    pub(crate) fn to_i64(&self) -> Option<i64> {
        match self {
            Self::Fixnum(number) => Some(*number),
            Self::Bignum(number) => number.to_i64(),
            Self::Rational(number) => {
                if number.is_integer() {
                    number.to_integer().to_i64()
                } else {
                    None
                }
            }
            Self::Flonum(number) => {
                if number.is_finite() && number.fract() == 0.0 {
                    Some(*number as i64)
                } else {
                    None
                }
            }
        }
    }

    /// 全部数值按 f64 近似（比较与超越函数用）。
    pub(crate) fn to_f64(&self) -> f64 {
        match self {
            Self::Fixnum(number) => *number as f64,
            Self::Bignum(number) => number.to_f64().unwrap_or(f64::NAN),
            Self::Rational(number) => number.to_f64().unwrap_or(f64::NAN),
            Self::Flonum(number) => *number,
        }
    }

    pub(crate) fn sign(&self) -> Option<i32> {
        match self {
            Self::Fixnum(number) => Some(number.signum() as i32),
            Self::Bignum(number) => Some(match number.sign() {
                num_bigint::Sign::Minus => -1,
                num_bigint::Sign::NoSign => 0,
                num_bigint::Sign::Plus => 1,
            }),
            Self::Rational(number) => Some(if number.is_positive() {
                1
            } else if number.is_negative() {
                -1
            } else {
                0
            }),
            Self::Flonum(number) => {
                if number.is_nan() {
                    None
                } else if *number > 0.0 {
                    Some(1)
                } else if *number < 0.0 {
                    Some(-1)
                } else {
                    Some(0)
                }
            }
        }
    }

    /// 把大整数折叠回 i64 区间。
    pub(crate) fn norm_bignum(number: BigInt) -> Self {
        match number.to_i64() {
            Some(small) => Self::Fixnum(small),
            None => Self::Bignum(Rc::new(number)),
        }
    }

    pub(crate) fn norm_rational(number: BigRational) -> Self {
        if number.is_integer() {
            Self::norm_bignum(number.to_integer())
        } else {
            Self::Rational(Rc::new(number))
        }
    }

    pub(crate) fn big(&self) -> BigInt {
        match self {
            Self::Fixnum(number) => BigInt::from(*number),
            Self::Bignum(number) => (**number).clone(),
            Self::Rational(number) => number.to_integer(),
            Self::Flonum(number) => BigInt::from(*number as i64),
        }
    }

    pub(crate) fn rational(&self) -> BigRational {
        match self {
            Self::Fixnum(number) => BigRational::from_integer(BigInt::from(*number)),
            Self::Bignum(number) => BigRational::from_integer((**number).clone()),
            Self::Rational(number) => (**number).clone(),
            Self::Flonum(number) => BigRational::from_integer(BigInt::from(*number as i64)),
        }
    }

    pub(crate) fn add(self, other: Self) -> Result<Self, SchemeError> {
        Ok(match (self, other) {
            (Self::Fixnum(left), Self::Fixnum(right)) => match left.checked_add(right) {
                Some(sum) => Self::Fixnum(sum),
                None => Self::norm_bignum(BigInt::from(left) + BigInt::from(right)),
            },
            (Self::Flonum(left), right) => Self::Flonum(left + right.to_f64()),
            (left, Self::Flonum(right)) => Self::Flonum(left.to_f64() + right),
            (left, right) => {
                if matches!(left, Self::Rational(_)) || matches!(right, Self::Rational(_)) {
                    Self::norm_rational(left.rational() + right.rational())
                } else {
                    Self::norm_bignum(left.big() + right.big())
                }
            }
        })
    }

    pub(crate) fn sub(self, other: Self) -> Result<Self, SchemeError> {
        Ok(match (self, other) {
            (Self::Fixnum(left), Self::Fixnum(right)) => match left.checked_sub(right) {
                Some(difference) => Self::Fixnum(difference),
                None => Self::norm_bignum(BigInt::from(left) - BigInt::from(right)),
            },
            (Self::Flonum(left), right) => Self::Flonum(left - right.to_f64()),
            (left, Self::Flonum(right)) => Self::Flonum(left.to_f64() - right),
            (left, right) => {
                if matches!(left, Self::Rational(_)) || matches!(right, Self::Rational(_)) {
                    Self::norm_rational(left.rational() - right.rational())
                } else {
                    Self::norm_bignum(left.big() - right.big())
                }
            }
        })
    }

    pub(crate) fn mul(self, other: Self) -> Result<Self, SchemeError> {
        Ok(match (self, other) {
            (Self::Fixnum(left), Self::Fixnum(right)) => match left.checked_mul(right) {
                Some(product) => Self::Fixnum(product),
                None => Self::norm_bignum(BigInt::from(left) * BigInt::from(right)),
            },
            (Self::Flonum(left), right) => Self::Flonum(left * right.to_f64()),
            (left, Self::Flonum(right)) => Self::Flonum(left.to_f64() * right),
            (left, right) => {
                if matches!(left, Self::Rational(_)) || matches!(right, Self::Rational(_)) {
                    Self::norm_rational(left.rational() * right.rational())
                } else {
                    Self::norm_bignum(left.big() * right.big())
                }
            }
        })
    }

    /// 除法：exact 之间非整除产生有理数；flonum 参与按 IEEE 除法；
    /// fixnum 除 fixnum 的整除保持 fixnum。
    pub(crate) fn div(self, other: Self) -> Result<Self, SchemeError> {
        match (self, other) {
            (Self::Flonum(left), right) => Ok(Self::Flonum(left / right.to_f64())),
            (left, Self::Flonum(right)) => Ok(Self::Flonum(left.to_f64() / right)),
            (left, right) => {
                if matches!(left, Self::Fixnum(_)) && matches!(right, Self::Fixnum(_)) {
                    let (left, right) = (left.to_i64().unwrap_or(0), right.to_i64().unwrap_or(0));
                    if right == 0 {
                        return Err(SchemeError::DivisionByZero);
                    }
                    if left % right == 0 {
                        return left
                            .checked_div(right)
                            .map(Self::Fixnum)
                            .ok_or(SchemeError::ArithmeticOverflow);
                    }
                }
                let right_rational = right.rational();
                if right_rational.is_zero() {
                    return Err(SchemeError::DivisionByZero);
                }
                Ok(Self::norm_rational(left.rational() / right_rational))
            }
        }
    }

    /// 数值相等 `=`：跨精确层按数值比较，inexact 侧转 f64。
    pub(crate) fn numeric_equal(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Flonum(_), _) | (_, Self::Flonum(_)) => {
                if self.is_nan() || other.is_nan() {
                    false
                } else {
                    self.to_f64() == other.to_f64()
                }
            }
            (left, right) => left.rational() == right.rational(),
        }
    }

    /// 全序比较：exact 之间精确；flonum 参与转 f64 近似。
    pub(crate) fn compare(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::Fixnum(left), Self::Fixnum(right)) => Some(left.cmp(right)),
            (Self::Flonum(_), _) | (_, Self::Flonum(_)) => {
                self.to_f64().partial_cmp(&other.to_f64())
            }
            (left, right) => Some(left.rational().cmp(&right.rational())),
        }
    }

    pub(crate) fn negate(self) -> Result<Self, SchemeError> {
        match self {
            Self::Fixnum(number) => number
                .checked_neg()
                .map(Self::Fixnum)
                .ok_or(SchemeError::ArithmeticOverflow),
            Self::Bignum(number) => Ok(Self::norm_bignum(-&*number)),
            Self::Rational(number) => Ok(Self::norm_rational(-&*number)),
            Self::Flonum(number) => Ok(Self::Flonum(-number)),
        }
    }

    pub(crate) fn abs(self) -> Result<Self, SchemeError> {
        match self {
            Self::Fixnum(number) => number
                .checked_abs()
                .map(Self::Fixnum)
                .ok_or(SchemeError::ArithmeticOverflow),
            Self::Bignum(number) => Ok(Self::norm_bignum(number.abs())),
            Self::Rational(number) => Ok(Self::norm_rational(number.abs())),
            Self::Flonum(number) => Ok(Self::Flonum(number.abs())),
        }
    }

    /// 整数幂：exact 基数与 exact 非负整数指数保持精确，否则转 flonum。
    pub(crate) fn pow(self, exponent: &Self) -> Result<Self, SchemeError> {
        match (&self, exponent) {
            (Self::Flonum(_), _) => {
                Ok(Self::Flonum(self.to_f64().powf(exponent.to_f64())))
            }
            (_, Self::Flonum(power)) => Ok(Self::Flonum(self.to_f64().powf(*power))),
            _ => {
                let base = self.big();
                let Some(power) = exponent.to_i64() else {
                    return Ok(Self::Flonum(base.to_f64().unwrap_or_default().powf(
                        exponent.to_f64(),
                    )));
                };
                // 指数上限是资源边界：越过即降级为 inexact 近似，避免分配爆炸。
                const MAX_EXACT_EXPONENT: i64 = 100_000;
                if power >= 0 {
                    if power > MAX_EXACT_EXPONENT {
                        return Ok(Self::Flonum(self.to_f64().powf(power as f64)));
                    }
                    let result = base.pow(power as u32);
                    Ok(Self::norm_bignum(result))
                } else {
                    // 负指数：基数为 0 报错，其余转有理数 1/base^|n|。
                    if base.is_zero() {
                        return Err(SchemeError::DivisionByZero);
                    }
                    if power.unsigned_abs() > MAX_EXACT_EXPONENT as u64 {
                        return Ok(Self::Flonum(self.to_f64().powf(power as f64)));
                    }
                    let denominator = base.pow(power.unsigned_abs() as u32);
                    Ok(Self::norm_rational(BigRational::new(
                        BigInt::from(1),
                        denominator,
                    )))
                }
            }
        }
    }

    /// 平方根：exact 完全平方返回 exact，否则 flonum。
    pub(crate) fn sqrt(&self) -> Result<Self, SchemeError> {
        match self {
            Self::Flonum(number) => Ok(Self::Flonum(number.sqrt())),
            _ => match self.sign() {
                Some(-1) => Ok(Self::Flonum(f64::NAN)),
                _ => {
                    if let Some(exact) = self.rational().sqrt() {
                        return Ok(Self::norm_rational(exact));
                    }
                    Ok(Self::Flonum(self.to_f64().sqrt()))
                }
            },
        }
    }

    /// exact 化（inexact->exact）：有限 flonum 按值转有理数。
    pub(crate) fn to_exact(self) -> Result<Self, SchemeError> {
        match self {
            Self::Flonum(number) => {
                if !number.is_finite() {
                    return Err(SchemeError::InvalidNumber {
                        text: number.to_string(),
                    });
                }
                // f64 恒可精确表示为有限二进制有理数。
                let rational = BigRational::from_float(number).ok_or(SchemeError::InvalidNumber {
                    text: number.to_string(),
                })?;
                Ok(Self::norm_rational(rational))
            }
            exact => Ok(exact),
        }
    }

    pub(crate) fn to_inexact(self) -> Result<Self, SchemeError> {
        Ok(Self::Flonum(self.to_f64()))
    }

    /// 十进制写形式；flonum 整数值保持 `x.0` 记法。
    pub(crate) fn to_text(&self) -> String {
        match self {
            Self::Fixnum(number) => number.to_string(),
            Self::Bignum(number) => number.to_string(),
            Self::Rational(number) => format!("{}/{}", number.numer(), number.denom()),
            Self::Flonum(number) => {
                if number.is_nan() {
                    "+nan.0".to_string()
                } else if number.is_infinite() {
                    if *number > 0.0 { "+inf.0" } else { "-inf.0" }.to_string()
                } else if *number == number.trunc() {
                    format!("{number}.0")
                } else {
                    format!("{number}")
                }
            }
        }
    }
}

/// 解析数值字面量：十进制 / radix 前缀、有理数 `n/d` 与特殊 flonum 记法。
pub(crate) fn parse_literal(token: &str, radix: u32, force_exact: bool, force_inexact: bool) -> Option<Number> {
    let token = token.replace('_', "");
    if token.is_empty() {
        return None;
    }
    // 特殊 flonum 记法在任意前缀下都按 inexact 解析。
    match token.as_str() {
        "+inf.0" => return Some(Number::Flonum(f64::INFINITY)),
        "-inf.0" => return Some(Number::Flonum(f64::NEG_INFINITY)),
        "+nan.0" | "-nan.0" => return Some(Number::Flonum(f64::NAN)),
        _ => {}
    }
    if let Some(slash) = token.find('/') {
        let (numerator, denominator) = token.split_at(slash);
        let denominator = &denominator[1..];
        let numerator = BigInt::from_str_radix(numerator, radix).ok()?;
        let denominator = BigInt::from_str_radix(denominator, radix).ok()?;
        if denominator.is_zero() {
            return None;
        }
        let rational = BigRational::new(numerator, denominator);
        let number = Number::norm_rational(rational);
        return finish_precision(number, force_exact, force_inexact);
    }
    let is_float_notation = token.contains('.') || (radix == 10 && token.contains(['e', 'E']));
    if is_float_notation {
        if radix != 10 {
            return None;
        }
        let value: f64 = token.parse().ok()?;
        return finish_precision(Number::Flonum(value), force_exact, force_inexact);
    }
    let integer = BigInt::from_str_radix(&token, radix).ok()?;
    let number = Number::norm_bignum(integer);
    finish_precision(number, force_exact, force_inexact)
}

fn finish_precision(
    number: Number,
    force_exact: bool,
    force_inexact: bool,
) -> Option<Number> {
    if force_exact {
        Some(number.to_exact().ok()?)
    } else if force_inexact {
        number.to_inexact().ok()
    } else {
        Some(number)
    }
}

// BigRational 无内建开方；分子分母分别整开方验证完全平方。
trait RationalSqrt {
    fn sqrt(&self) -> Option<BigRational>;
}

impl RationalSqrt for BigRational {
    fn sqrt(&self) -> Option<BigRational> {
        let exact_root = |value: &BigInt| -> Option<BigInt> {
            if value.is_negative() {
                return None;
            }
            let root = value.sqrt();
            (&root * &root == *value).then_some(root)
        };
        let numerator = exact_root(self.numer())?;
        let denominator = exact_root(self.denom())?;
        Some(BigRational::new(numerator, denominator))
    }
}
