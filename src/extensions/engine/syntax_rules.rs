//! `syntax-rules` 卫生宏：模式匹配、模板展开与重命名卫生。
//!
//! 卫生方案是经典「重命名 + 回退」：每次展开把模板中非模式变量的标识
//! 统一重命名为 `name^<展开号>`，宏体内定义与引用一致重命名而互不与使
//! 用处捕获；环境查找失败时按 `^` 后缀回退到原始名（即宏定义环境的
//! 全局绑定）。`(quote ...)` 子模板视为数据，不参与重命名。展开即求值
//! 的下一步，天然受燃料、取消与帧数边界约束，递归宏耗尽燃料即类型化
//! 失败。椭圆绑定模型：每个模式变量绑定其匹配值序列（非椭圆变量为单
//! 元素序列，椭圆变量每迭代追加一项）。

use std::collections::BTreeMap;
use std::rc::Rc;

use super::error::SchemeError;
use super::value::Value;
use super::SchemeEngine;

/// 一条重写规则：模式与模板。
#[derive(Debug, Clone)]
pub(crate) struct SyntaxRule {
    pattern: Value,
    template: Value,
}

/// `syntax-rules` 变换器；作为 `Value::Macro` 参与环境的宏绑定。
pub struct MacroTransformer {
    literals: Vec<Rc<str>>,
    rules: Vec<SyntaxRule>,
}

impl MacroTransformer {
    /// 把规则的模式与模板（AST 值）压入标记栈。
    pub(crate) fn push_children(&self, stack: &mut Vec<Value>) {
        for rule in &self.rules {
            stack.push(rule.pattern.clone());
            stack.push(rule.template.clone());
        }
    }

    /// 估算存活字节（GC 资源计量）。
    pub(crate) fn approx_bytes(&self) -> usize {
        (self.literals.len() * 16 + self.rules.len() * 2 * std::mem::size_of::<Value>())
            .max(std::mem::size_of::<MacroTransformer>())
    }
}

/// 模式变量绑定：值序列（椭圆变量按迭代追加）。
type Bindings = BTreeMap<String, Vec<Value>>;

impl MacroTransformer {
    /// 解析 `(syntax-rules (literals...) (pattern template)...)`。
    pub fn parse(form: &[Value]) -> Result<Self, SchemeError> {
        if form.len() < 2 {
            return Err(SchemeError::InvalidSyntax {
                form: "syntax-rules",
                reason: "需要字面量列表与至少一条规则",
            });
        }
        let literal_items = match &form[0] {
            Value::Null => Vec::new(),
            list => flatten_strict(list)?,
        };
        let mut literals = Vec::with_capacity(literal_items.len());
        for literal in literal_items {
            match literal {
                Value::Symbol(name) => {
                    if !literals.iter().any(|existing| existing == &name) {
                        literals.push(name);
                    }
                }
                _ => {
                    return Err(SchemeError::InvalidSyntax {
                        form: "syntax-rules",
                        reason: "字面量必须是符号",
                    })
                }
            }
        }
        let mut rules = Vec::new();
        for rule in &form[1..] {
            let elements = flatten_strict(rule)?;
            if elements.len() != 2 {
                return Err(SchemeError::InvalidSyntax {
                    form: "syntax-rules",
                    reason: "规则是 (pattern template)",
                });
            }
            rules.push(SyntaxRule {
                pattern: elements[0].clone(),
                template: elements[1].clone(),
            });
        }
        Ok(Self { literals, rules })
    }

    /// 依次尝试规则；返回展开后的完整形式。
    pub fn expand(
        &self,
        engine: &mut super::SchemeEngine,
        keyword: &Rc<str>,
        form: &[Value],
        expansion: u64,
    ) -> Result<Value, SchemeError> {
        for rule in &self.rules {
            let mut bindings = Bindings::new();
            // 模式首元素是宏关键字（惯例 `_`）；与完整使用形式匹配。
            let mut whole_items = vec![Value::Symbol(keyword.clone())];
            whole_items.extend(form.iter().cloned());
            let whole = engine.list_from_slice(&whole_items)?;
            if match_pattern(&rule.pattern, &whole, &self.literals, &mut bindings)? {
                return expand_template(engine, &rule.template, &bindings, expansion);
            }
        }
        Err(SchemeError::InvalidSyntax {
            form: "syntax-rules",
            reason: "没有规则匹配该使用形式",
        })
    }
}

fn symbol_of(value: &Value) -> Option<Rc<str>> {
    match value {
        Value::Symbol(name) => Some(name.clone()),
        _ => None,
    }
}

fn is_ellipsis(value: &Value) -> bool {
    matches!(value, Value::Symbol(name) if name.as_ref() == "...")
}

/// 模式与形式匹配；成功时收集模式变量。
fn match_pattern(
    pattern: &Value,
    form: &Value,
    literals: &[Rc<str>],
    bindings: &mut Bindings,
) -> Result<bool, SchemeError> {
    match pattern {
        Value::Symbol(name) => {
            if name.as_ref() == "_" {
                return Ok(true);
            }
            if literals.iter().any(|literal| literal == name) {
                return Ok(symbol_of(form).is_some_and(|found| &found == name));
            }
            bindings.insert(name.to_string(), vec![form.clone()]);
            Ok(true)
        }
        Value::Null => Ok(matches!(form, Value::Null)),
        Value::Vector(pattern_items) => match form {
            Value::Vector(items) => {
                let pattern_items = pattern_items.borrow().clone();
                let items = items.borrow().clone();
                match_lists(&pattern_items, &items, literals, bindings)
            }
            _ => Ok(false),
        },
        Value::Pair(_) => {
            let pattern_items = flatten_relaxed(pattern)?;
            let form_items = match form {
                Value::Null => Vec::new(),
                Value::Pair(_) => flatten_relaxed(form)?,
                _ => return Ok(false),
            };
            match_lists(&pattern_items, &form_items, literals, bindings)
        }
        // 自身字面量（数值 / 字符 / 布尔 / 字符串）按相等匹配。
        literal => Ok(literal == form),
    }
}

/// 列表 / 向量模式匹配；`items` 为宽松展平（非严格尾保留为最后一项）。
fn match_lists(
    pattern: &[Value],
    form: &[Value],
    literals: &[Rc<str>],
    bindings: &mut Bindings,
) -> Result<bool, SchemeError> {
    let mut pattern_index = 0;
    let mut form_index = 0;
    while pattern_index < pattern.len() {
        if pattern_index + 1 < pattern.len() && is_ellipsis(&pattern[pattern_index + 1]) {
            // `(p ... . rest)`：p 吸收零或多个项，rest 匹配剩余（回溯）。
            let sub = pattern[pattern_index].clone();
            let after = &pattern[pattern_index + 2..];
            while form_index + after.len() <= form.len() {
                let collected = &form[form_index..form.len() - after.len()];
                let mut trial = bindings.clone();
                let mut ok = true;
                for item in collected {
                    let mut sub_bindings = Bindings::new();
                    if !match_pattern(&sub, item, literals, &mut sub_bindings)? {
                        ok = false;
                        break;
                    }
                    for (name, values) in sub_bindings {
                        trial.entry(name).or_default().extend(values);
                    }
                }
                if ok {
                    let mut rest_ok = true;
                    for (offset, rest_pattern) in after.iter().enumerate() {
                        if !match_pattern(
                            rest_pattern,
                            &form[form.len() - after.len() + offset],
                            literals,
                            &mut trial,
                        )? {
                            rest_ok = false;
                            break;
                        }
                    }
                    if rest_ok {
                        *bindings = trial;
                        return Ok(true);
                    }
                }
                form_index += 1;
            }
            return Ok(false);
        }
        if form_index >= form.len() {
            return Ok(false);
        }
        if !match_pattern(
            &pattern[pattern_index],
            &form[form_index],
            literals,
            bindings,
        )? {
            return Ok(false);
        }
        pattern_index += 1;
        form_index += 1;
    }
    Ok(form_index == form.len())
}

/// 模板展开：模式变量替换、椭圆展开与非引用标识的重命名卫生。
fn expand_template(
    engine: &mut super::SchemeEngine,
    template: &Value,
    bindings: &Bindings,
    expansion: u64,
) -> Result<Value, SchemeError> {
    expand_node(engine, template, bindings, expansion, false)
}

fn expand_node(
    engine: &mut super::SchemeEngine,
    template: &Value,
    bindings: &Bindings,
    expansion: u64,
    quoted: bool,
) -> Result<Value, SchemeError> {
    match template {
        Value::Symbol(name) => {
            if quoted {
                return expand_quoted(engine, template, bindings);
            }
            if let Some(values) = bindings.get(name.as_ref()) {
                // 裸模式变量取首个匹配值；椭圆迭代经由列表展开路径。
                return Ok(values.first().cloned().unwrap_or(Value::Unspecified));
            }
            if name.as_ref() == "_" {
                return Ok(template.clone());
            }
            if is_syntactic_keyword(name) {
                // 语法关键字不是变量，重命名会遮蔽 special form 分发。
                return Ok(template.clone());
            }
            // 宏引入标识：统一重命名（卫生）。
            Ok(Value::Symbol(Rc::from(format!("{name}^{expansion}"))))
        }
        Value::Pair(_) => {
            let items = flatten_relaxed(template)?;
            if !quoted
                && let Some(Value::Symbol(head)) = items.first()
                && head.as_ref() == "quote"
            {
                // quote 子模板是数据：模式变量替换为输入，其余字面。
                let mut expanded = Vec::with_capacity(items.len());
                expanded.push(items[0].clone());
                for item in &items[1..] {
                    expanded.push(expand_quoted(engine, item, bindings)?);
                }
                return engine.list_from_slice(&expanded);
            }
            if !quoted {
                return expand_list(engine, &items, bindings, expansion);
            }
            let mut expanded = Vec::with_capacity(items.len());
            for item in items {
                expanded.push(expand_quoted(engine, &item, bindings)?);
            }
            quoted_list(engine, expanded)
        }
        Value::Vector(items) => {
            let items = items.borrow().clone();
            let mut expanded = Vec::new();
            let mut index = 0;
            while index < items.len() {
                if index + 1 < items.len() && is_ellipsis(&items[index + 1]) {
                    expanded
                        .extend(ellipsis_values(engine, &items[index], bindings, expansion)?);
                    index += 2;
                } else {
                    expanded.push(expand_node(
                        engine,
                        &items[index],
                        bindings,
                        expansion,
                        quoted,
                    )?);
                    index += 1;
                }
            }
            engine.new_vector_from(expanded)
        }
        other => Ok(other.clone()),
    }
}

/// 列表模板展开：`(子模板 ...)` 椭圆拼接与普通元素。
fn expand_list(
    engine: &mut super::SchemeEngine,
    items: &[Value],
    bindings: &Bindings,
    expansion: u64,
) -> Result<Value, SchemeError> {
    let mut flat: Vec<Value> = Vec::with_capacity(items.len());
    let mut index = 0;
    while index < items.len() {
        if index + 1 < items.len() && is_ellipsis(&items[index + 1]) {
            flat.extend(ellipsis_values(engine, &items[index], bindings, expansion)?);
            index += 2;
            continue;
        }
        if is_ellipsis(&items[index]) {
            return Err(SchemeError::InvalidSyntax {
                form: "syntax-rules",
                reason: "椭圆前缺少子模板",
            });
        }
        flat.push(expand_node(engine, &items[index], bindings, expansion, false)?);
        index += 1;
    }
    engine.list_from_slice(&flat)
}

/// 椭圆子模板展开：按迭代索引逐次实例化子模板。
fn ellipsis_values(
    engine: &mut super::SchemeEngine,
    sub_template: &Value,
    bindings: &Bindings,
    expansion: u64,
) -> Result<Vec<Value>, SchemeError> {
    let iterating: Vec<&String> = bindings
        .keys()
        .filter(|name| template_uses(sub_template, name))
        .collect();
    if iterating.is_empty() {
        // 零匹配（如 `(x ...)` 匹配空输入）展开为零项。
        return Ok(Vec::new());
    }
    let length = iterating
        .iter()
        .map(|name| bindings[*name].len())
        .max()
        .unwrap_or(0);
    let mut results = Vec::with_capacity(length);
    for iteration in 0..length {
        let mut single = Bindings::new();
        for name in &iterating {
            let value = bindings[*name]
                .get(iteration)
                .cloned()
                .unwrap_or(Value::Unspecified);
            single.insert((*name).clone(), vec![value]);
        }
        results.push(expand_node(engine, sub_template, &single, expansion, false)?);
    }
    Ok(results)
}

fn template_uses(template: &Value, name: &str) -> bool {
    match template {
        Value::Symbol(found) => found.as_ref() == name,
        Value::Pair(_) => flatten_relaxed(template)
            .unwrap_or_default()
            .iter()
            .any(|item| template_uses(item, name)),
        Value::Vector(items) => items
            .borrow()
            .iter()
            .any(|item| template_uses(item, name)),
        _ => false,
    }
}

/// quote 子模板：模式变量替换为输入数据，其余保持字面。
fn expand_quoted(
    engine: &mut SchemeEngine,
    template: &Value,
    bindings: &Bindings,
) -> Result<Value, SchemeError> {
    match template {
        Value::Symbol(name) => {
            if let Some(values) = bindings.get(name.as_ref()) {
                return Ok(values.first().cloned().unwrap_or(Value::Unspecified));
            }
            Ok(template.clone())
        }
        Value::Pair(_) => {
            let items = flatten_relaxed(template)?;
            let mut expanded = Vec::with_capacity(items.len());
            for item in items {
                expanded.push(expand_quoted(engine, &item, bindings)?);
            }
            quoted_list(engine, expanded)
        }
        Value::Vector(items) => {
            let items = items.borrow().clone();
            let mut expanded = Vec::with_capacity(items.len());
            for item in items {
                expanded.push(expand_quoted(engine, &item, bindings)?);
            }
            engine.new_vector_from(expanded)
        }
        other => Ok(other.clone()),
    }
}

/// quote 数据列表拼装（构造走引擎登记与配额）。
fn quoted_list(engine: &mut SchemeEngine, items: Vec<Value>) -> Result<Value, SchemeError> {
    let mut list = Value::Null;
    for item in items.into_iter().rev() {
        list = engine.new_pair(item, list)?;
    }
    Ok(list)
}

fn flatten_strict(value: &Value) -> Result<Vec<Value>, SchemeError> {
    super::eval::flatten_public(value)
}

fn flatten_relaxed(value: &Value) -> Result<Vec<Value>, SchemeError> {
    super::eval::flatten_public_relaxed(value)
}

/// 语法关键字清单：special form 与控制原语名不参与重命名。
fn is_syntactic_keyword(name: &str) -> bool {
    matches!(
        name,
        "quote"
            | "if"
            | "define"
            | "set!"
            | "lambda"
            | "begin"
            | "let"
            | "let*"
            | "letrec"
            | "letrec*"
            | "cond"
            | "case"
            | "and"
            | "or"
            | "when"
            | "unless"
            | "do"
            | "define-record-type"
            | "syntax-rules"
            | "define-syntax"
            | "let-syntax"
            | "letrec-syntax"
            | "define-library"
            | "import"
            | "quasiquote"
            | "unquote"
            | "unquote-splicing"
            | "let-values"
            | "let*-values"
            | "define-values"
            | "guard"
            | "raise"
            | "raise-continuable"
            | "with-exception-handler"
            | "parameterize"
            | "delay"
            | "delay-force"
            | "make-promise"
            | "case-lambda"
            | "assert"
            | "include"
            | "include-ci"
            | "cond-expand"
            | "call/cc"
            | "call-with-current-continuation"
            | "dynamic-wind"
            | "else"
            | "..."
            | "=>"
            | "unquote-splicing"
    )
}
