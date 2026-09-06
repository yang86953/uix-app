//! 求值核心：显式栈机器、proper tail calls、一等 continuation、
//! dynamic-wind、异常（raise / guard / with-exception-handler）与多值。
//!
//! 求值不消耗宿主栈：全部「求值后继续」以显式帧压入机器栈（堆分配），
//! 非尾深度因此是纯粹的帧数资源边界。尾位置不压新帧。`call/cc` 捕获
//! 当前帧栈与 dynamic-wind 链的不可变快照，调用快照即控制转移。
//! `let` / `let*` / `letrec` / named let / `do` / `case-lambda` /
//! `parameterize` 在求值期脱糖为 lambda 应用；`quasiquote` 求值期展开。
//! 异常以 `SchemeError::Raised(对象)` 在机器内传播，`guard` 帧快照恢复，
//! 配额 / 取消 / 墙钟 / 内部错误不经脚本异常通道。
//! 机器循环在安全点检查回收阈值，以帧栈、wind 链与 handler 栈为额外根。

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use super::error::SchemeError;
use super::syntax_rules::MacroTransformer;
use super::value::{single_value, Closure, Env, EnvNode, Pair, Value};
use super::SchemeEngine;

/// let 系绑定列表：`(name, initializer)` 对。
type Bindings = Vec<(Rc<str>, Value)>;

/// 形式参数：固定参数与可选 rest 参数。
type Formals = (Vec<Rc<str>>, Option<Rc<str>>);

/// dynamic-wind 的 `(before, after)` 过程对。
pub type WindPair = (Value, Value);

/// 一等 continuation 的不可变快照；捕获帧栈与 wind 链。
pub struct ContinuationSnapshot {
    frames: Vec<Frame>,
    winds: Vec<WindPair>,
}

impl ContinuationSnapshot {
    /// 快照持有的环境（帧中的 env 引用）。
    pub(crate) fn environments(&self) -> Vec<Env> {
        let mut envs = Vec::new();
        for frame in &self.frames {
            frame.collect_envs(&mut envs);
        }
        envs
    }

    /// 快照持有的值根。
    pub(crate) fn child_values(&self) -> Vec<Value> {
        let mut values = Vec::new();
        for frame in &self.frames {
            frame.collect_values(&mut values);
        }
        for (before, after) in &self.winds {
            values.push(before.clone());
            values.push(after.clone());
        }
        values
    }

    pub(crate) fn frame_count(&self) -> usize {
        self.frames.len()
    }

    pub(crate) fn wind_count(&self) -> usize {
        self.winds.len()
    }
}

/// 需要机器状态的控制原语；经 `Value::Control` 作为一等值传递。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ControlOp {
    CallCc,
    DynamicWind,
    Map,
    ForEach,
    Apply,
    /// `values`：构造多值载体。
    Values,
    /// `call-with-values`。
    CallWithValues,
    /// `raise`（非 continuable）。
    Raise,
    /// `raise-continuable`。
    RaiseContinuable,
    /// `with-exception-handler`。
    WithExceptionHandler,
}

/// 异常处理入口：guard 子句或动态安装的过程。
enum HandlerEntry {
    Guard {
        variable: Rc<str>,
        clauses: Vec<Value>,
        env: Env,
        frames: Vec<Frame>,
        winds: Vec<WindPair>,
        handlers: usize,
    },
    Procedure {
        procedure: Value,
        frames: Vec<Frame>,
        winds: Vec<WindPair>,
        handlers: usize,
    },
}

/// 显式栈帧：收到「值就绪」后给出下一步。
#[derive(Clone)]
enum Frame {
    /// 操作符值就绪；继续求操作数或直接应用。
    Operator { operands: Vec<Value>, env: Env },
    /// 一个操作数就绪；收集齐后应用。
    Operands {
        operator: Value,
        pending: Vec<Value>,
        evaluated: Vec<Value>,
        env: Env,
    },
    /// `if` 测试就绪；进入命中分支（尾位置）。
    IfBranch {
        consequent: Value,
        alternative: Option<Value>,
        env: Env,
    },
    /// 序列（begin / body）前序表达式完成；`pending` 尾表达式直接回传。
    Sequence { pending: Vec<Value>, env: Env },
    /// `define` 值就绪；绑定。
    DefineValue { name: Rc<str>, env: Env },
    /// `set!` 值就绪；沿链修改。
    SetValue { name: Rc<str>, env: Env },
    AndOperands { pending: Vec<Value>, env: Env },
    OrOperands { pending: Vec<Value>, env: Env },
    /// `when` / `unless` 测试就绪；按 negate 决定是否进入 body。
    WhenBody {
        body: Vec<Value>,
        negate: bool,
        env: Env,
    },
    /// `cond` 测试就绪；真值进入 `body`，假值推进剩余子句。
    CondClauses {
        body: Vec<Value>,
        clauses: Vec<Value>,
        env: Env,
    },
    /// `cond =>` 的接收过程就绪；以测试值应用。
    CondReceiver { test: Value },
    /// `case` 键就绪；按 datum 匹配选择子句。
    CaseMatch { clauses: Vec<Value>, env: Env },
    /// dynamic-wind：before 完成 → 登记 wind 并进入 thunk。
    WindBefore { wind: WindPair, thunk: Value },
    /// dynamic-wind：thunk 完成 → 撤销 wind 并执行 after。
    WindThunk { wind: WindPair },
    /// dynamic-wind：after 完成；thunk 结果是 dynamic-wind 的值。
    WindResult { result: Value },
    /// map / for-each 的逐行迭代：回调返回后推进或收尾。
    MapLoop {
        procedure: Value,
        rows: Vec<Vec<Value>>,
        results: Vec<Value>,
        collect: bool,
    },
    /// `call-with-values`：producer 结果就绪；展开为 consumer 参数。
    ApplyConsumer { consumer: Value },
    /// `let-values` 系：producer 结果就绪；解构绑定并继续剩余绑定。
    LetValuesBind {
        formals: Value,
        remaining: Vec<(Value, Value)>,
        body: Vec<Value>,
        env: Env,
        sequential: bool,
    },
    /// `define-values`：producer 结果就绪；解构绑定到当前环境。
    DefineValues { formals: Value, env: Env },
    /// guard / with-exception-handler 体内值就绪；恢复 handler 栈深度。
    HandlerCleanup { saved: usize },
}

impl Frame {
    /// 该帧是否接收多值（values 载体不投影为单值）。
    fn accepts_values(&self) -> bool {
        matches!(self, Frame::ApplyConsumer { .. } | Frame::LetValuesBind { .. } | Frame::DefineValues { .. })
    }

    fn collect_values(&self, out: &mut Vec<Value>) {
        match self {
            Frame::Operator { operands, env } => {
                out.extend(operands.iter().cloned());
                env_values(env, out);
            }
            Frame::Operands {
                operator,
                pending,
                evaluated,
                env,
            } => {
                out.push(operator.clone());
                out.extend(pending.iter().cloned());
                out.extend(evaluated.iter().cloned());
                env_values(env, out);
            }
            Frame::IfBranch {
                consequent,
                alternative,
                env,
            } => {
                out.push(consequent.clone());
                if let Some(alternative) = alternative {
                    out.push(alternative.clone());
                }
                env_values(env, out);
            }
            Frame::Sequence { pending, env } => {
                out.extend(pending.iter().cloned());
                env_values(env, out);
            }
            Frame::DefineValue { name: _, env } | Frame::SetValue { name: _, env } => {
                env_values(env, out);
            }
            Frame::AndOperands { pending, env } | Frame::OrOperands { pending, env } => {
                out.extend(pending.iter().cloned());
                env_values(env, out);
            }
            Frame::WhenBody { body, env, .. } => {
                out.extend(body.iter().cloned());
                env_values(env, out);
            }
            Frame::CondClauses { body, clauses, env } => {
                out.extend(body.iter().cloned());
                out.extend(clauses.iter().cloned());
                env_values(env, out);
            }
            Frame::CondReceiver { test } => out.push(test.clone()),
            Frame::CaseMatch { clauses, env } => {
                out.extend(clauses.iter().cloned());
                env_values(env, out);
            }
            Frame::WindBefore { wind, thunk } => {
                out.push(wind.0.clone());
                out.push(wind.1.clone());
                out.push(thunk.clone());
            }
            Frame::WindThunk { wind } => {
                out.push(wind.0.clone());
                out.push(wind.1.clone());
            }
            Frame::WindResult { result } => out.push(result.clone()),
            Frame::MapLoop {
                procedure,
                rows,
                results,
                ..
            } => {
                out.push(procedure.clone());
                out.extend(rows.iter().flatten().cloned());
                out.extend(results.iter().cloned());
            }
            Frame::ApplyConsumer { consumer } => out.push(consumer.clone()),
            Frame::LetValuesBind {
                formals,
                remaining,
                body,
                env,
                ..
            } => {
                out.push(formals.clone());
                out.extend(remaining.iter().flat_map(|(formals, init)| {
                    [formals.clone(), init.clone()]
                }));
                out.extend(body.iter().cloned());
                env_values(env, out);
            }
            Frame::DefineValues { formals, env } => {
                out.push(formals.clone());
                env_values(env, out);
            }
            Frame::HandlerCleanup { saved: _ } => {}
        }
    }

    fn collect_envs(&self, out: &mut Vec<Env>) {
        match self {
            Frame::Operator { env: _, .. }
            | Frame::Operands { env: _, .. }
            | Frame::IfBranch { env: _, .. }
            | Frame::Sequence { env: _, .. }
            | Frame::DefineValue { env: _, .. }
            | Frame::SetValue { env: _, .. }
            | Frame::AndOperands { env: _, .. }
            | Frame::OrOperands { env: _, .. }
            | Frame::WhenBody { env: _, .. }
            | Frame::CondClauses { env: _, .. }
            | Frame::CaseMatch { env: _, .. }
            | Frame::LetValuesBind { env: _, .. }
            | Frame::DefineValues { env: _, .. } => {}
            _ => {}
        }
    }

    fn frame_env(&self) -> Option<&Env> {
        match self {
            Frame::Operator { env, .. }
            | Frame::Operands { env, .. }
            | Frame::IfBranch { env, .. }
            | Frame::Sequence { env, .. }
            | Frame::DefineValue { env, .. }
            | Frame::SetValue { env, .. }
            | Frame::AndOperands { env, .. }
            | Frame::OrOperands { env, .. }
            | Frame::WhenBody { env, .. }
            | Frame::CondClauses { env, .. }
            | Frame::CaseMatch { env, .. }
            | Frame::LetValuesBind { env, .. }
            | Frame::DefineValues { env, .. } => Some(env),
            _ => None,
        }
    }
}

fn env_values(env: &Env, out: &mut Vec<Value>) {
    // 环境经引擎 mark_env 遍历；这里只需保证 env 可达性由帧直接持有。
    if let Ok(borrowed) = env.try_borrow() {
        out.extend(borrowed.bindings.values().cloned());
    }
}

/// 帧收到值后的下一步。
enum Next {
    /// 求值一个新表达式（当前帧已消费）。
    Eval(Value, Env),
    /// 值向上回传（尾位置语义）。
    Value(Value),
    /// 应用过程。
    Apply(Value, Vec<Value>),
}

/// 显式栈机器。
pub(crate) struct Machine {
    frames: Vec<Frame>,
    winds: Vec<WindPair>,
    handlers: Vec<HandlerEntry>,
}

impl Default for Machine {
    fn default() -> Self {
        Self {
            frames: Vec::new(),
            winds: Vec::new(),
            handlers: Vec::new(),
        }
    }
}

impl Machine {
    /// 从给定下一步跑到帧栈为空，返回程序值。
    fn run(&mut self, engine: &mut SchemeEngine, start: Next) -> Result<Value, SchemeError> {
        let mut state = start;
        loop {
            engine.tick()?;
            if self.frames.len() > engine.limits().maximum_depth as usize {
                return Err(SchemeError::DepthLimitExceeded {
                    maximum: engine.limits().maximum_depth,
                });
            }
            if engine.gc_due() {
                // 安全校验：当前 Next 状态（求值中的表达式、环境与值）
                // 尚未进入帧栈，必须一并作为根，否则会被误清空。
                let (mut value_roots, mut env_roots) = self.collect_roots();
                match &state {
                    Next::Eval(expression, environment) => {
                        value_roots.push(expression.clone());
                        env_roots.push(environment.clone());
                    }
                    Next::Value(value) => value_roots.push(value.clone()),
                    Next::Apply(procedure, arguments) => {
                        value_roots.push(procedure.clone());
                        value_roots.extend(arguments.iter().cloned());
                    }
                }
                engine.collect_garbage(&value_roots, &env_roots)?;
            }
            let outcome = match state {
                Next::Value(value) => match self.frames.pop() {
                    None => return Ok(single_value(value)),
                    Some(frame) => {
                        let value = if frame.accepts_values() {
                            value
                        } else {
                            single_value(value)
                        };
                        self.advance(engine, frame, value)
                    }
                },
                Next::Eval(expression, environment) => {
                    self.step(engine, expression, environment)
                }
                Next::Apply(procedure, arguments) => {
                    self.apply_action(engine, procedure, arguments)
                }
            };
            match outcome {
                Ok(next) => state = next,
                Err(SchemeError::Raised(payload)) => {
                    let Some(dispatch) = self.dispatch_raised(engine, payload.clone())? else {
                        return Err(SchemeError::Raised(payload));
                    };
                    state = dispatch;
                }
                Err(other) => return Err(other),
            }
        }
    }

    /// 异常分发：恢复最近 handler 的帧 / wind / handler 快照。
    ///
    /// 返回 `None` 表示无 handler，异常继续向顶层传播。
    fn dispatch_raised(
        &mut self,
        engine: &mut SchemeEngine,
        payload: Value,
    ) -> Result<Option<Next>, SchemeError> {
        let Some(handler) = self.handlers.pop() else {
            return Ok(None);
        };
        match handler {
            HandlerEntry::Guard {
                variable,
                clauses,
                env,
                frames,
                winds,
                handlers,
            } => {
                self.transfer_winds(engine, &winds)?;
                self.frames = frames;
                self.winds = winds;
                self.handlers.truncate(handlers);
                let scope = engine.new_env(&env);
                EnvNode::define(&scope, &variable, payload);
                let mut elements = vec![Value::Symbol("cond".into())];
                elements.extend(clauses.iter().cloned());
                let cond_form = engine.list_from_slice(&elements)?;
                Ok(Some(Next::Eval(cond_form, scope)))
            }
            HandlerEntry::Procedure {
                procedure,
                frames: _,
                winds,
                handlers,
            } => {
                self.transfer_winds(engine, &winds)?;
                self.handlers.truncate(handlers);
                // R7RS：handler 返回时（非 continuable）以其返回值继续
                // 向外层 raise。子机器调用，体内 continuation 作用域为
                // 已知限制（登记于兼容矩阵）。
                let outcome = apply_procedure(engine, &procedure, &[payload]);
                match outcome {
                    Ok(value) => Err(SchemeError::Raised(value)),
                    Err(SchemeError::Raised(inner)) => Err(SchemeError::Raised(inner)),
                    Err(other) => Err(other),
                }
            }
        }
    }

    /// 把当前 wind 链退到目标链：执行多出的 after，恢复目标链。
    fn transfer_winds(&mut self, engine: &mut SchemeEngine, target: &[WindPair]) -> Result<(), SchemeError> {
        let common = self
            .winds
            .iter()
            .zip(target.iter())
            .take_while(|((left_before, left_after), (right_before, right_after))| {
                same_procedure(left_before, right_before)
                    && same_procedure(left_after, right_after)
            })
            .count();
        for (_, after) in self.winds[common..].iter().rev() {
            apply_procedure(engine, after, &[])?;
        }
        self.winds.truncate(common);
        self.winds.extend_from_slice(target);
        Ok(())
    }

    /// 机器根集合：帧栈、wind 链与 handler 栈持有的值与环境。
    fn collect_roots(&self) -> (Vec<Value>, Vec<Env>) {
        let mut values = Vec::new();
        let mut envs = Vec::new();
        for frame in &self.frames {
            frame.collect_values(&mut values);
            if let Some(env) = frame.frame_env() {
                envs.push(env.clone());
            }
        }
        for (before, after) in &self.winds {
            values.push(before.clone());
            values.push(after.clone());
        }
        for handler in &self.handlers {
            match handler {
                HandlerEntry::Guard {
                    variable: _,
                    clauses,
                    env,
                    frames,
                    winds,
                    ..
                } => {
                    values.extend(clauses.iter().cloned());
                    envs.push(env.clone());
                    for frame in frames {
                        frame.collect_values(&mut values);
                        if let Some(env) = frame.frame_env() {
                            envs.push(env.clone());
                        }
                    }
                    for (before, after) in winds {
                        values.push(before.clone());
                        values.push(after.clone());
                    }
                }
                HandlerEntry::Procedure { procedure, frames, winds, .. } => {
                    values.push(procedure.clone());
                    for frame in frames {
                        frame.collect_values(&mut values);
                    }
                    for (before, after) in winds {
                        values.push(before.clone());
                        values.push(after.clone());
                    }
                }
            }
        }
        (values, envs)
    }

    /// 求值单个表达式。
    fn step(
        &mut self,
        engine: &mut SchemeEngine,
        expression: Value,
        environment: Env,
    ) -> Result<Next, SchemeError> {
        match &expression {
            Value::Symbol(name) => EnvNode::lookup(&environment, name)
                .ok_or_else(|| SchemeError::UnboundVariable {
                    name: name.to_string(),
                })
                .map(Next::Value),
            Value::Pair(pair) => {
                let (head, rest) = split_application(pair)?;
                let operator: Option<std::rc::Rc<str>> = match &head {
                    Value::Symbol(name) => Some(name.clone()),
                    _ => None,
                };
                if let Some(operator) = operator {
                    match operator.as_ref() {
                        "quote" => {
                            expect_arity("quote", &rest, 1)?;
                            // 契约：元素数量已在前置 arity / 长度检查中确认，解包安全。
                            Ok(Next::Value(rest.into_iter().next().expect("已检查数量")))
                        }
                        "if" => {
                            expect_arity_range("if", &rest, 2, 3)?;
                            self.frames.push(Frame::IfBranch {
                                consequent: rest[1].clone(),
                                alternative: rest.get(2).cloned(),
                                env: environment.clone(),
                            });
                            Ok(Next::Eval(rest[0].clone(), environment))
                        }
                        "define" => self.step_define(engine, rest, environment),
                        "set!" => {
                            expect_arity("set!", &rest, 2)?;
                            let name = match &rest[0] {
                                Value::Symbol(name) => name.clone(),
                                _ => return Err(invalid("set!", "首参数必须是符号")),
                            };
                            self.frames.push(Frame::SetValue {
                                name,
                                env: environment.clone(),
                            });
                            Ok(Next::Eval(rest[1].clone(), environment))
                        }
                        "lambda" => {
                            if rest.len() < 2 {
                                return Err(invalid("lambda", "需要形式参数与主体"));
                            }
                            make_closure(engine, &rest[0], &rest[1..], environment).map(Next::Value)
                        }
                        "begin" => Ok(self.enter_sequence(rest, environment)),
                        "let" => {
                            let desugared = desugar_let(engine, &rest)?;
                            Ok(Next::Eval(desugared, environment))
                        }
                        "let*" => {
                            let desugared = desugar_let_star(engine, &rest)?;
                            Ok(Next::Eval(desugared, environment))
                        }
                        "letrec" | "letrec*" => {
                            let desugared = desugar_letrec(engine, &rest)?;
                            Ok(Next::Eval(desugared, environment))
                        }
                        "do" => {
                            let desugared = desugar_do(engine, &rest)?;
                            Ok(Next::Eval(desugared, environment))
                        }
                        "cond" => self.step_cond(rest, environment),
                        "case" => {
                            expect_arity_at_least("case", &rest, 1)?;
                            self.frames.push(Frame::CaseMatch {
                                clauses: rest[1..].to_vec(),
                                env: environment.clone(),
                            });
                            Ok(Next::Eval(rest[0].clone(), environment))
                        }
                        "and" => match rest.split_first() {
                            None => Ok(Next::Value(Value::Bool(true))),
                            Some((first, remaining)) => {
                                self.push_pending_frame(remaining, |pending| Frame::AndOperands {
                                    pending,
                                    env: environment.clone(),
                                });
                                Ok(Next::Eval(first.clone(), environment))
                            }
                        },
                        "or" => match rest.split_first() {
                            None => Ok(Next::Value(Value::Bool(false))),
                            Some((first, remaining)) => {
                                self.push_pending_frame(remaining, |pending| Frame::OrOperands {
                                    pending,
                                    env: environment.clone(),
                                });
                                Ok(Next::Eval(first.clone(), environment))
                            }
                        },
                        "when" | "unless" => {
                            let negate = operator.as_ref() == "unless";
                            let form = if negate { "unless" } else { "when" };
                            let body = body_of(form, &rest)?.to_vec();
                            self.frames.push(Frame::WhenBody {
                                body,
                                negate,
                                env: environment.clone(),
                            });
                            Ok(Next::Eval(rest[0].clone(), environment))
                        }
                        "define-record-type" => {
                            eval_define_record_type(engine, &rest, &environment)
                                .map(Next::Value)
                        }
                        "syntax-rules" => {
                            let transformer = MacroTransformer::parse(&rest)?;
                            engine.new_macro(transformer).map(Next::Value)
                        }
                        "define-syntax" => {
                            expect_arity_range("define-syntax", &rest, 2, 2)?;
                            let name = match &rest[0] {
                                Value::Symbol(name) => name.clone(),
                                _ => return Err(invalid("define-syntax", "宏名必须是符号")),
                            };
                            self.frames.push(Frame::DefineValue {
                                name,
                                env: environment.clone(),
                            });
                            Ok(Next::Eval(rest[1].clone(), environment))
                        }
                        "let-syntax" | "letrec-syntax" => {
                            // 宏绑定与变量共用值空间，直接复用 let 脱糖。
                            let desugared = if operator.as_ref() == "let-syntax" {
                                desugar_let(engine, &rest)?
                            } else {
                                desugar_letrec(engine, &rest)?
                            };
                            Ok(Next::Eval(desugared, environment))
                        }
                        "define-library" => {
                            eval_define_library(engine, &rest, &environment)
                                .map(Next::Value)
                        }
                        "import" => {
                            eval_import(engine, &rest, &environment).map(Next::Value)
                        }
                        "quasiquote" => {
                            expect_arity("quasiquote", &rest, 1)?;
                            let expanded = expand_quasiquote(engine, &rest[0], 1)?;
                            Ok(Next::Value(expanded))
                        }
                        "unquote" | "unquote-splicing" => Err(invalid(
                            if operator.as_ref() == "unquote" { "unquote" } else { "unquote-splicing" },
                            "只允许出现在 quasiquote 内",
                        )),
                        "let-values" => self.step_let_values(rest, environment, false),
                        "let*-values" => self.step_let_values(rest, environment, true),
                        "define-values" => {
                            if rest.len() != 2 {
                                return Err(invalid("define-values", "需要形式参数与产生器"));
                            }
                            self.frames.push(Frame::DefineValues {
                                formals: rest[0].clone(),
                                env: environment.clone(),
                            });
                            Ok(Next::Eval(rest[1].clone(), environment))
                        }
                        "guard" => self.step_guard(rest, environment),
                        "parameterize" => {
                            let desugared = desugar_parameterize(engine, &rest)?;
                            Ok(Next::Eval(desugared, environment))
                        }
                        "delay" | "delay-force" => {
                            if rest.is_empty() {
                                return Err(invalid(
                                    if operator.as_ref() == "delay" { "delay" } else { "delay-force" },
                                    "需要主体",
                                ));
                            }
                            let thunk =
                                make_closure(engine, &Value::Null, &rest, environment.clone())?;
                            engine.new_promise(super::value::PromiseState::Pending(thunk))
                                .map(Next::Value)
                        }
                        "case-lambda" => {
                            let desugared = desugar_case_lambda(engine, &rest)?;
                            Ok(Next::Eval(desugared, environment))
                        }
                        "assert" => {
                            expect_arity("assert", &rest, 1)?;
                            let quoted = super::reader::wrap_with_quote(engine, rest[0].clone())?;
                            let message = engine.new_string_from("assertion failed".to_string())?;
                            let failure_form = engine.list_from_slice(&[
                                Value::Symbol("error".into()),
                                message,
                                quoted,
                            ])?;
                            let assert_form = engine.list_from_slice(&[
                                Value::Symbol("if".into()),
                                rest[0].clone(),
                                Value::Unspecified,
                                failure_form,
                            ])?;
                            Ok(Next::Eval(assert_form, environment))
                        }
                        "include" | "include-ci" => {
                            let statements = eval_include(engine, &rest)?;
                            Ok(self.enter_sequence(statements, environment))
                        }
                        "cond-expand" => {
                            let selected = select_cond_expand(engine, &rest)?;
                            match selected {
                                None => Ok(Next::Value(Value::Unspecified)),
                                Some(statements) => {
                                    Ok(self.enter_sequence(statements, environment))
                                }
                            }
                        }
                        _ => {
                            // 宏使用点：head 绑定变换器时按规则展开后重新求值。
                            if let Some(Value::Macro(transformer)) =
                                EnvNode::lookup(&environment, &operator)
                            {
                                let expansion = engine.next_macro_expansion();
                                let expanded =
                                    transformer.expand(engine, &operator, &rest, expansion)?;
                                return Ok(Next::Eval(expanded, environment));
                            }
                            self.step_application(head.clone(), rest, environment)
                        }
                    }
                } else {
                    self.step_application(head, rest, environment)
                }
            }
            _ => Ok(Next::Value(expression)),
        }
    }

    fn step_define(
        &mut self,
        engine: &mut SchemeEngine,
        rest: Vec<Value>,
        environment: Env,
    ) -> Result<Next, SchemeError> {
        let target = rest
            .first()
            .ok_or_else(|| invalid("define", "缺少定义目标"))?;
        match target {
            Value::Symbol(name) => {
                expect_arity("define", &rest, 2)?;
                self.frames.push(Frame::DefineValue {
                    name: name.clone(),
                    env: environment.clone(),
                });
                Ok(Next::Eval(rest[1].clone(), environment))
            }
            Value::Pair(pair) => {
                let (name, formals) = {
                    let borrowed = pair.borrow();
                    let name = match &borrowed.car {
                        Value::Symbol(name) => name.clone(),
                        _ => return Err(invalid("define", "函数定义名必须是符号")),
                    };
                    (name, borrowed.cdr.clone())
                };
                let body = &rest[1..];
                if body.is_empty() {
                    return Err(invalid("define", "函数定义需要主体"));
                }
                let closure = make_closure(engine, &formals, body, environment.clone())?;
                EnvNode::define(&environment, &name, closure);
                Ok(Next::Value(Value::Unspecified))
            }
            _ => Err(invalid("define", "定义目标必须是符号或函数形式")),
        }
    }

    /// cond 逐子句推进：求值测试前压入携带「命中后延续」的帧。
    fn step_cond(&mut self, clauses: Vec<Value>, environment: Env) -> Result<Next, SchemeError> {
        match clauses.split_first() {
            None => Ok(Next::Value(Value::Unspecified)),
            Some((clause, remaining)) => {
                let elements = flatten_strict_list("cond", clause)?;
                if elements.is_empty() {
                    return Err(invalid("cond", "子句不能为空"));
                }
                let is_else =
                    matches!(&elements[0], Value::Symbol(name) if name.as_ref() == "else");
                if is_else {
                    if !remaining.is_empty() {
                        return Err(invalid("cond", "else 子句必须是最后一个"));
                    }
                    if elements.len() < 2 {
                        return Err(invalid("cond", "else 子句需要主体"));
                    }
                    return Ok(self.enter_sequence(elements[1..].to_vec(), environment));
                }
                self.frames.push(Frame::CondClauses {
                    body: elements[1..].to_vec(),
                    clauses: remaining.to_vec(),
                    env: environment.clone(),
                });
                Ok(Next::Eval(elements[0].clone(), environment))
            }
        }
    }

    /// `let-values` / `let*-values`：逐绑定求值 producer 并解构。
    fn step_let_values(
        &mut self,
        rest: Vec<Value>,
        environment: Env,
        sequential: bool,
    ) -> Result<Next, SchemeError> {
        if rest.len() < 2 {
            return Err(invalid("let-values", "需要绑定列表与主体"));
        }
        let binding_list = flatten_strict_list("let-values", &rest[0])?;
        let body = rest[1..].to_vec();
        let mut bindings = Vec::with_capacity(binding_list.len());
        for binding in binding_list {
            let elements = flatten_strict_list("let-values", &binding)?;
            if elements.len() != 2 {
                return Err(invalid("let-values", "每个绑定是 (formals init)"));
            }
            bindings.push((elements[0].clone(), elements[1].clone()));
        }
        match bindings.split_first() {
            None => Ok(self.enter_sequence(body, environment)),
            Some(((formals, init), remaining)) => {
                self.frames.push(Frame::LetValuesBind {
                    formals: formals.clone(),
                    remaining: remaining.to_vec(),
                    body,
                    env: environment.clone(),
                    sequential,
                });
                Ok(Next::Eval(init.clone(), environment))
            }
        }
    }

    /// `(guard (var clause...) body...)`。
    fn step_guard(
        &mut self,
        rest: Vec<Value>,
        environment: Env,
    ) -> Result<Next, SchemeError> {
        if rest.len() < 2 {
            return Err(invalid("guard", "需要异常子句与主体"));
        }
        let spec = flatten_strict_list("guard", &rest[0])?;
        let Some(Value::Symbol(variable)) = spec.first() else {
            return Err(invalid("guard", "异常变量必须是符号"));
        };
        let clauses = spec[1..].to_vec();
        let body = rest[1..].to_vec();
        self.handlers.push(HandlerEntry::Guard {
            variable: variable.clone(),
            clauses,
            env: environment.clone(),
            frames: self.frames.clone(),
            winds: self.winds.clone(),
            handlers: self.handlers.len(),
        });
        let saved = self.handlers.len();
        self.frames.push(Frame::HandlerCleanup { saved });
        Ok(self.enter_sequence(body, environment))
    }

    fn step_application(
        &mut self,
        head: Value,
        rest: Vec<Value>,
        environment: Env,
    ) -> Result<Next, SchemeError> {
        self.frames.push(Frame::Operator {
            operands: rest,
            env: environment.clone(),
        });
        Ok(Next::Eval(head, environment))
    }

    /// 进入表达式序列：尾表达式的值直接回传（proper tail call）。
    fn enter_sequence(&mut self, statements: Vec<Value>, environment: Env) -> Next {
        match statements.split_first() {
            None => Next::Value(Value::Unspecified),
            Some((first, remaining)) => {
                if !remaining.is_empty() {
                    self.frames.push(Frame::Sequence {
                        pending: remaining.to_vec(),
                        env: environment.clone(),
                    });
                }
                Next::Eval(first.clone(), environment)
            }
        }
    }

    fn push_pending_frame(
        &mut self,
        remaining: &[Value],
        build: impl FnOnce(Vec<Value>) -> Frame,
    ) {
        if !remaining.is_empty() {
            self.frames.push(build(remaining.to_vec()));
        }
    }

    /// 帧收到值后的推进。
    fn advance(
        &mut self,
        engine: &mut SchemeEngine,
        frame: Frame,
        value: Value,
    ) -> Result<Next, SchemeError> {
        match frame {
            Frame::Operator { operands, env } => match operands.split_first() {
                None => Ok(Next::Apply(value, Vec::new())),
                Some((first, remaining)) => {
                    self.frames.push(Frame::Operands {
                        operator: value,
                        pending: remaining.to_vec(),
                        evaluated: Vec::new(),
                        env: env.clone(),
                    });
                    Ok(Next::Eval(first.clone(), env))
                }
            },
            Frame::Operands {
                operator,
                pending,
                mut evaluated,
                env,
            } => {
                evaluated.push(value);
                match pending.split_first() {
                    None => Ok(Next::Apply(operator, evaluated)),
                    Some((first, remaining)) => {
                        self.frames.push(Frame::Operands {
                            operator,
                            pending: remaining.to_vec(),
                            evaluated,
                            env: env.clone(),
                        });
                        Ok(Next::Eval(first.clone(), env))
                    }
                }
            }
            Frame::IfBranch {
                consequent,
                alternative,
                env,
            } => {
                if value.is_false() {
                    match alternative {
                        Some(alternative) => Ok(Next::Eval(alternative, env)),
                        None => Ok(Next::Value(Value::Unspecified)),
                    }
                } else {
                    Ok(Next::Eval(consequent, env))
                }
            }
            Frame::Sequence { pending, env } => Ok(self.enter_sequence(pending, env)),
            Frame::DefineValue { name, env } => {
                EnvNode::define(&env, &name, value);
                Ok(Next::Value(Value::Unspecified))
            }
            Frame::SetValue { name, env } => {
                if !EnvNode::set(&env, &name, value) {
                    return Err(SchemeError::UnboundVariable { name: name.to_string() });
                }
                Ok(Next::Value(Value::Unspecified))
            }
            Frame::AndOperands { pending, env } => {
                if value.is_false() {
                    return Ok(Next::Value(value));
                }
                // 逐操作数推进：每个值都要短路检查，尾操作数直接回传。
                match pending.split_first() {
                    None => Ok(Next::Value(value)),
                    Some((first, remaining)) => {
                        self.push_pending_frame(remaining, |pending| Frame::AndOperands {
                            pending,
                            env: env.clone(),
                        });
                        Ok(Next::Eval(first.clone(), env))
                    }
                }
            }
            Frame::OrOperands { pending, env } => {
                if !value.is_false() {
                    return Ok(Next::Value(value));
                }
                match pending.split_first() {
                    None => Ok(Next::Value(value)),
                    Some((first, remaining)) => {
                        self.push_pending_frame(remaining, |pending| Frame::OrOperands {
                            pending,
                            env: env.clone(),
                        });
                        Ok(Next::Eval(first.clone(), env))
                    }
                }
            }
            Frame::WhenBody {
                body,
                negate,
                env,
            } => {
                let truthy = !value.is_false();
                let take_branch = if negate { !truthy } else { truthy };
                if take_branch {
                    Ok(self.enter_sequence(body, env))
                } else {
                    Ok(Next::Value(Value::Unspecified))
                }
            }
            Frame::CondClauses {
                body,
                clauses,
                env,
            } => {
                if !value.is_false() {
                    // 测试为真：无 body 回传测试值；`=>` 应用接收过程；否则序列。
                    if body.is_empty() {
                        return Ok(Next::Value(value));
                    }
                    if matches!(&body[0], Value::Symbol(name) if name.as_ref() == "=>") {
                        if body.len() != 2 {
                            return Err(invalid("cond =>", "恰好需要一个接收过程"));
                        }
                        self.frames.push(Frame::CondReceiver { test: value });
                        return Ok(Next::Eval(body[1].clone(), env));
                    }
                    return Ok(self.enter_sequence(body, env));
                }
                self.step_cond(clauses, env)
            }
            Frame::CondReceiver { test } => Ok(Next::Apply(value, vec![test])),
            Frame::CaseMatch { clauses, env } => {
                for clause in &clauses {
                    let elements = flatten_strict_list("case", clause)?;
                    if elements.is_empty() {
                        return Err(invalid("case", "子句不能为空"));
                    }
                    let is_else =
                        matches!(&elements[0], Value::Symbol(name) if name.as_ref() == "else");
                    let matched = is_else
                        || flatten_strict_list("case", &elements[0])?.contains(&value);
                    if matched {
                        if elements.len() < 2 {
                            return Err(invalid("case", "子句需要主体"));
                        }
                        return Ok(self.enter_sequence(elements[1..].to_vec(), env));
                    }
                }
                Ok(Next::Value(Value::Unspecified))
            }
            Frame::WindBefore { wind, thunk } => {
                self.winds.push(wind.clone());
                self.frames.push(Frame::WindThunk { wind });
                Ok(Next::Apply(thunk, Vec::new()))
            }
            Frame::WindThunk { wind } => {
                self.winds.pop();
                self.frames.push(Frame::WindResult { result: value });
                Ok(Next::Apply(wind.1, Vec::new()))
            }
            Frame::WindResult { result } => Ok(Next::Value(result)),
            Frame::MapLoop {
                procedure,
                rows,
                mut results,
                collect,
            } => {
                results.push(value);
                match rows.split_first() {
                    None => Ok(Next::Value(if collect {
                        engine.list_from_slice(&results)?
                    } else {
                        Value::Unspecified
                    })),
                    Some((next_row, remaining)) => {
                        self.frames.push(Frame::MapLoop {
                            procedure: procedure.clone(),
                            rows: remaining.to_vec(),
                            results,
                            collect,
                        });
                        Ok(Next::Apply(procedure, next_row.clone()))
                    }
                }
            }
            Frame::ApplyConsumer { consumer } => {
                let values = match value {
                    Value::Values(items) => (*items).clone(),
                    single => vec![single],
                };
                Ok(Next::Apply(consumer, values))
            }
            Frame::LetValuesBind {
                formals,
                remaining,
                body,
                env,
                sequential,
            } => {
                let values = match value {
                    Value::Values(items) => (*items).clone(),
                    single => vec![single],
                };
                bind_formals(engine, &env, &formals, &values)?;
                match remaining.split_first() {
                    None => Ok(self.enter_sequence(body, env)),
                    Some(((next_formals, next_init), rest_bindings)) => {
                        let next_env = env.clone();
                        self.frames.push(Frame::LetValuesBind {
                            formals: next_formals.clone(),
                            remaining: rest_bindings.to_vec(),
                            body,
                            env,
                            sequential,
                        });
                        Ok(Next::Eval(next_init.clone(), next_env))
                    }
                }
            }
            Frame::DefineValues { formals, env } => {
                let values = match value {
                    Value::Values(items) => (*items).clone(),
                    single => vec![single],
                };
                bind_formals(engine, &env, &formals, &values)?;
                Ok(Next::Value(Value::Unspecified))
            }
            Frame::HandlerCleanup { saved } => {
                self.handlers.truncate(saved);
                Ok(Next::Value(value))
            }
        }
    }

    /// 应用一个过程：闭包进入 body（尾位置），其余同步或控制转移。
    fn apply_action(
        &mut self,
        engine: &mut SchemeEngine,
        procedure: Value,
        arguments: Vec<Value>,
    ) -> Result<Next, SchemeError> {
        match procedure {
            Value::Closure(closure) => {
                let scope = bind_closure_parameters(engine, &closure, arguments)?;
                let body: Vec<Value> = closure.body.to_vec();
                Ok(self.enter_sequence(body, scope))
            }
            Value::Primitive(function) => {
                let value = function(engine, &arguments)?;
                Ok(Next::Value(value))
            }
            Value::Host(host) => {
                let value = engine.call_host(&host, &arguments)?;
                Ok(Next::Value(value))
            }
            Value::Parameter(cell) => match arguments.len() {
                0 => {
                    let value = cell.borrow().current.clone();
                    Ok(Next::Value(value))
                }
                1 => {
                    let converted = {
                        let converter = cell.borrow().converter.clone();
                        match converter {
                            Some(converter) => {
                                apply_procedure(engine, &converter, &arguments)?
                            }
                            // 契约：元素数量已在前置 arity / 长度检查中确认，解包安全。
                            None => arguments.into_iter().next().expect("已检查数量"),
                        }
                    };
                    cell.borrow_mut().current = converted.clone();
                    Ok(Next::Value(converted))
                }
                actual => Err(SchemeError::ArityMismatch {
                    procedure: "#<parameter>".to_string(),
                    expected: "0 或 1".to_string(),
                    actual,
                }),
            },
            Value::Control(ControlOp::CallCc) => {
                if arguments.len() != 1 {
                    return Err(SchemeError::ArityMismatch {
                        procedure: "call/cc".to_string(),
                        expected: "1".to_string(),
                        actual: arguments.len(),
                    });
                }
                // 契约：元素数量已在前置 arity / 长度检查中确认，解包安全。
                let receiver = arguments.into_iter().next().expect("已检查数量");
                let snapshot = ContinuationSnapshot {
                    frames: self.frames.clone(),
                    winds: self.winds.clone(),
                };
                let continuation = engine.new_continuation(snapshot)?;
                Ok(Next::Apply(receiver, vec![continuation]))
            }
            Value::Control(ControlOp::DynamicWind) => {
                if arguments.len() != 3 {
                    return Err(SchemeError::ArityMismatch {
                        procedure: "dynamic-wind".to_string(),
                        expected: "3".to_string(),
                        actual: arguments.len(),
                    });
                }
                let mut values = arguments.into_iter();
                // 契约：元素数量已在前置 arity / 长度检查中确认，解包安全。
                let before = values.next().expect("已检查数量");
                // 契约：元素数量已在前置 arity / 长度检查中确认，解包安全。
                let thunk = values.next().expect("已检查数量");
                // 契约：元素数量已在前置 arity / 长度检查中确认，解包安全。
                let after = values.next().expect("已检查数量");
                self.frames.push(Frame::WindBefore {
                    wind: (before.clone(), after),
                    thunk,
                });
                Ok(Next::Apply(before, Vec::new()))
            }
            Value::Continuation(snapshot) => {
                let value = arguments.into_iter().next().unwrap_or(Value::Unspecified);
                self.call_continuation(engine, &snapshot, value)
            }
            Value::Control(ControlOp::Map) | Value::Control(ControlOp::ForEach) => {
                let collect = matches!(procedure, Value::Control(ControlOp::Map));
                if arguments.len() < 2 {
                    return Err(SchemeError::ArityMismatch {
                        procedure: if collect { "map".into() } else { "for-each".into() },
                        expected: "至少 2".into(),
                        actual: arguments.len(),
                    });
                }
                let mut iter = arguments.into_iter();
                // 契约：元素数量已在前置 arity / 长度检查中确认，解包安全。
                let target = iter.next().expect("已检查数量");
                let mut rows: Vec<Vec<Value>> = Vec::new();
                for list in iter {
                    rows.push(flatten_strict_list("map", &list)?);
                }
                let length = rows.first().map(Vec::len).unwrap_or(0);
                if rows.iter().any(|row| row.len() != length) {
                    return Err(SchemeError::InvalidSyntax {
                        form: "map",
                        reason: "列表必须等长",
                    });
                }
                let mut transposed: Vec<Vec<Value>> = Vec::with_capacity(length);
                for index in 0..length {
                    transposed.push(rows.iter().map(|row| row[index].clone()).collect());
                }
                match transposed.split_first() {
                    None => Ok(Next::Value(if collect {
                        Value::Null
                    } else {
                        Value::Unspecified
                    })),
                    Some((first, remaining)) => {
                        self.frames.push(Frame::MapLoop {
                            procedure: target.clone(),
                            rows: remaining.to_vec(),
                            results: Vec::new(),
                            collect,
                        });
                        Ok(Next::Apply(target, first.clone()))
                    }
                }
            }
            Value::Control(ControlOp::Apply) => {
                if arguments.len() < 2 {
                    return Err(SchemeError::ArityMismatch {
                        procedure: "apply".into(),
                        expected: "至少 2".into(),
                        actual: arguments.len(),
                    });
                }
                let argument_count = arguments.len();
                let mut iter = arguments.into_iter();
                // 契约：元素数量已在前置 arity / 长度检查中确认，解包安全。
                let target = iter.next().expect("已检查数量");
                let prefix: Vec<Value> = iter.by_ref().take(argument_count - 2).collect();
                // 契约：元素数量已在前置 arity / 长度检查中确认，解包安全。
                let spread = iter.next().expect("已检查数量");
                let spread_items = flatten_strict_list("apply", &spread)?;
                // 参数重组后直接转发应用（尾语义：无额外帧）。
                let mut resolved = prefix;
                resolved.extend(spread_items);
                Ok(Next::Apply(target, resolved))
            }
            Value::Control(ControlOp::Values) => match arguments.len() {
                0 => Ok(Next::Value(Value::Unspecified)),
                1 => Ok(Next::Value(
                    // 契约：元素数量已在前置 arity / 长度检查中确认，解包安全。
                    arguments.into_iter().next().expect("已检查数量"),
                )),
                _ => engine.new_values(arguments).map(Next::Value),
            },
            Value::Control(ControlOp::CallWithValues) => {
                if arguments.len() != 2 {
                    return Err(SchemeError::ArityMismatch {
                        procedure: "call-with-values".to_string(),
                        expected: "2".to_string(),
                        actual: arguments.len(),
                    });
                }
                let mut values = arguments.into_iter();
                // 契约：元素数量已在前置 arity / 长度检查中确认，解包安全。
                let producer = values.next().expect("已检查数量");
                // 契约：元素数量已在前置 arity / 长度检查中确认，解包安全。
                let consumer = values.next().expect("已检查数量");
                self.frames.push(Frame::ApplyConsumer { consumer });
                Ok(Next::Apply(producer, Vec::new()))
            }
            Value::Control(ControlOp::Raise) => {
                let payload = arguments.into_iter().next().unwrap_or(Value::Unspecified);
                Err(SchemeError::Raised(payload))
            }
            Value::Control(ControlOp::RaiseContinuable) => {
                let payload = arguments.into_iter().next().unwrap_or(Value::Unspecified);
                let Some(HandlerEntry::Procedure {
                    procedure,
                    frames: _,
                    winds,
                    handlers,
                }) = self.handlers.pop()
                else {
                    return Err(SchemeError::Raised(payload));
                };
                self.transfer_winds(engine, &winds)?;
                self.handlers.truncate(handlers);
                let outcome = apply_procedure(engine, &procedure, &[payload]);
                match outcome {
                    Ok(value) => Ok(Next::Value(value)),
                    Err(SchemeError::Raised(inner)) => Err(SchemeError::Raised(inner)),
                    Err(other) => Err(other),
                }
            }
            Value::Control(ControlOp::WithExceptionHandler) => {
                if arguments.len() != 2 {
                    return Err(SchemeError::ArityMismatch {
                        procedure: "with-exception-handler".to_string(),
                        expected: "2".to_string(),
                        actual: arguments.len(),
                    });
                }
                let mut values = arguments.into_iter();
                // 契约：元素数量已在前置 arity / 长度检查中确认，解包安全。
                let handler = values.next().expect("已检查数量");
                // 契约：元素数量已在前置 arity / 长度检查中确认，解包安全。
                let thunk = values.next().expect("已检查数量");
                self.handlers.push(HandlerEntry::Procedure {
                    procedure: handler,
                    frames: self.frames.clone(),
                    winds: self.winds.clone(),
                    handlers: self.handlers.len(),
                });
                let saved = self.handlers.len();
                self.frames.push(Frame::HandlerCleanup { saved });
                Ok(Next::Apply(thunk, Vec::new()))
            }
            _ => Err(SchemeError::NotCallable),
        }
    }

    /// 控制转移：退栈执行当前链多出的 after，重入执行目标链多出的
    /// before，然后恢复捕获的帧栈与 wind 链。
    fn call_continuation(
        &mut self,
        engine: &mut SchemeEngine,
        snapshot: &Rc<ContinuationSnapshot>,
        value: Value,
    ) -> Result<Next, SchemeError> {
        let common = self
            .winds
            .iter()
            .zip(snapshot.winds.iter())
            .take_while(|((left_before, left_after), (right_before, right_after))| {
                same_procedure(left_before, right_before)
                    && same_procedure(left_after, right_after)
            })
            .count();
        for (_, after) in self.winds[common..].iter().rev() {
            apply_procedure(engine, after, &[])?;
        }
        let reenter: Vec<Value> = snapshot.winds[common..]
            .iter()
            .map(|(before, _)| before.clone())
            .collect();
        for before in reenter {
            apply_procedure(engine, &before, &[])?;
        }
        self.frames = snapshot.frames.clone();
        self.winds = snapshot.winds.clone();
        Ok(Next::Value(value))
    }
}

/// 过程身份比较：同一闭包 / 原语 / 宿主函数即同一 wind 组件。
fn same_procedure(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Closure(left), Value::Closure(right)) => Rc::ptr_eq(left, right),
        (Value::Primitive(left), Value::Primitive(right)) => std::ptr::fn_addr_eq(*left, *right),
        (Value::Host(left), Value::Host(right)) => Rc::ptr_eq(left, right),
        _ => false,
    }
}

/// 求值一个表达式：独立机器跑到帧栈为空。
pub(crate) fn eval(
    engine: &mut SchemeEngine,
    expression: &Value,
    environment: &Env,
) -> Result<Value, SchemeError> {
    Machine::default().run(
        engine,
        Next::Eval(expression.clone(), environment.clone()),
    )
}

/// 对已有参数应用过程；map / apply / cond => 与 wind 转移的调用入口。
///
/// 以子机器执行：回调内捕获的 continuation 只作用于子机器（wind 转移
/// 中执行的 before / after 同理），跨机器控制语义是已知限制。
pub(crate) fn apply_procedure(
    engine: &mut SchemeEngine,
    procedure: &Value,
    arguments: &[Value],
) -> Result<Value, SchemeError> {
    Machine::default().run(
        engine,
        Next::Apply(procedure.clone(), arguments.to_vec()),
    )
}

fn split_application(
    pair: &Rc<RefCell<Pair>>,
) -> Result<(Value, Vec<Value>), SchemeError> {
    let borrowed = pair.borrow();
    Ok((
        borrowed.car.clone(),
        flatten_strict_list("application", &borrowed.cdr)?,
    ))
}

/// 严格展平（宏模块复用）。
pub(crate) fn flatten_public(value: &Value) -> Result<Vec<Value>, SchemeError> {
    flatten_strict_list("syntax-rules", value)
}

/// 宽松展平：非严格尾保留为最后一项原值。
pub(crate) fn flatten_public_relaxed(value: &Value) -> Result<Vec<Value>, SchemeError> {
    let mut elements = Vec::new();
    let mut current = value.clone();
    loop {
        match current {
            Value::Null => return Ok(elements),
            Value::Pair(pair) => {
                let borrowed = pair.borrow();
                elements.push(borrowed.car.clone());
                match borrowed.cdr.clone() {
                    Value::Pair(_) | Value::Null => current = borrowed.cdr.clone(),
                    tail => {
                        elements.push(tail);
                        return Ok(elements);
                    }
                }
            }
            other => {
                elements.push(other);
                return Ok(elements);
            }
        }
    }
}

/// 把严格列表展平为向量；非严格尾按语法错误拒绝。
fn flatten_strict_list(form: &'static str, value: &Value) -> Result<Vec<Value>, SchemeError> {
    let mut elements = Vec::new();
    let mut current = value.clone();
    loop {
        match current {
            Value::Null => return Ok(elements),
            Value::Pair(pair) => {
                let borrowed = pair.borrow();
                elements.push(borrowed.car.clone());
                current = borrowed.cdr.clone();
            }
            _ => return Err(invalid(form, "此处需要严格列表")),
        }
    }
}

fn invalid(form: &'static str, reason: &'static str) -> SchemeError {
    SchemeError::InvalidSyntax { form, reason }
}

fn expect_arity(
    form: &'static str,
    values: &[Value],
    expected: usize,
) -> Result<(), SchemeError> {
    if values.len() != expected {
        return Err(invalid(form, "操作数数量不符"));
    }
    Ok(())
}

fn expect_arity_range(
    form: &'static str,
    values: &[Value],
    minimum: usize,
    maximum: usize,
) -> Result<(), SchemeError> {
    if values.len() < minimum || values.len() > maximum {
        return Err(invalid(form, "操作数数量不符"));
    }
    Ok(())
}

fn expect_arity_at_least(
    form: &'static str,
    values: &[Value],
    minimum: usize,
) -> Result<(), SchemeError> {
    if values.len() < minimum {
        return Err(invalid(form, "操作数数量不符"));
    }
    Ok(())
}

// ---------- let 系脱糖（AST 构造走分配记账） ----------

/// `(let ((n i) ...) body...)` → `((lambda (n ...) body...) i ...)`。
fn desugar_let(engine: &mut SchemeEngine, rest: &[Value]) -> Result<Value, SchemeError> {
    match rest.first() {
        Some(Value::Symbol(_)) => desugar_named_let(engine, rest),
        _ => {
            let (bindings, body) = split_bindings_and_body("let", rest)?;
            desugar_application(engine, bindings, body)
        }
    }
}

/// `(let* (...) body...)` → 嵌套 lambda 应用。
fn desugar_let_star(engine: &mut SchemeEngine, rest: &[Value]) -> Result<Value, SchemeError> {
    let (bindings, body) = split_bindings_and_body("let*", rest)?;
    if bindings.is_empty() {
        return desugar_application(engine, bindings, body);
    }
    let mut current = desugar_application(engine, Vec::new(), body)?;
    for (name, initializer) in bindings.into_iter().rev() {
        let procedure = build_lambda(engine, vec![Value::Symbol(name)], vec![current])?;
        current = build_application(engine, vec![procedure], vec![initializer])?;
    }
    Ok(current)
}

/// `(letrec ((n i) ...) body...)` → `((lambda (n ...) (set! ...) body...) 占位...)`。
fn desugar_letrec(engine: &mut SchemeEngine, rest: &[Value]) -> Result<Value, SchemeError> {
    let (bindings, body) = split_bindings_and_body("letrec", rest)?;
    let mut statements = Vec::with_capacity(bindings.len() + body.len());
    for (name, initializer) in bindings.iter() {
        statements.push(build_application(
            engine,
            vec![Value::Symbol("set!".into())],
            vec![Value::Symbol(name.clone()), initializer.clone()],
        )?);
    }
    statements.extend(body.iter().cloned());
    desugar_application(engine, bindings, statements)
}

/// named let：外层 lambda 以占位符绑定循环名，体内 set! 自定义后尾调用。
fn desugar_named_let(engine: &mut SchemeEngine, rest: &[Value]) -> Result<Value, SchemeError> {
    let name = match rest.first() {
        Some(Value::Symbol(name)) => name.clone(),
        _ => return Err(invalid("let", "named let 结构无效")),
    };
    let (bindings, body) = split_bindings_and_body("let", &rest[1..])?;
    let loop_procedure = build_lambda(
        engine,
        bindings
            .iter()
            .map(|(parameter, _)| Value::Symbol(parameter.clone()))
            .collect(),
        body,
    )?;
    let set_form = build_application(
        engine,
        vec![Value::Symbol("set!".into())],
        vec![Value::Symbol(name.clone()), loop_procedure],
    )?;
    let call_form = build_application(
        engine,
        vec![Value::Symbol(name.clone())],
        bindings
            .into_iter()
            .map(|(_, initializer)| initializer)
            .collect(),
    )?;
    let outer = build_lambda(
        engine,
        vec![Value::Symbol(name)],
        vec![set_form, call_form],
    )?;
    // 外层占位实参：'()（随后即被 set! 覆盖）。
    let placeholder = build_application(
        engine,
        vec![Value::Symbol("quote".into())],
        vec![Value::Null],
    )?;
    build_application(engine, vec![outer], vec![placeholder])
}

/// `((lambda (names...) body...) inits...)`。
fn desugar_application(
    engine: &mut SchemeEngine,
    bindings: Bindings,
    body: Vec<Value>,
) -> Result<Value, SchemeError> {
    let procedure = build_lambda(
        engine,
        bindings
            .iter()
            .map(|(name, _)| Value::Symbol(name.clone()))
            .collect(),
        body,
    )?;
    build_application(
        engine,
        vec![procedure],
        bindings
            .into_iter()
            .map(|(_, initializer)| initializer)
            .collect(),
    )
}

fn split_bindings_and_body(
    form: &'static str,
    rest: &[Value],
) -> Result<(Bindings, Vec<Value>), SchemeError> {
    let bindings = parse_bindings(form, rest)?;
    let body = body_of(form, rest)?.to_vec();
    Ok((bindings, body))
}

/// 构造 `(lambda (names...) body...)` 的 AST。
fn build_lambda(
    engine: &mut SchemeEngine,
    names: Vec<Value>,
    body: Vec<Value>,
) -> Result<Value, SchemeError> {
    let formals = engine.list_from_slice(&names)?;
    let mut elements = vec![Value::Symbol("lambda".into()), formals];
    elements.extend(body);
    engine.list_from_slice(&elements)
}

/// 构造 `(operator operands...)` 的 AST。
fn build_application(
    engine: &mut SchemeEngine,
    operator: Vec<Value>,
    operands: Vec<Value>,
) -> Result<Value, SchemeError> {
    let mut elements = operator;
    elements.extend(operands);
    engine.list_from_slice(&elements)
}

/// `(do ((var init [step])...) (test result...) body...)` 脱糖为 named let：
/// `(let loop ((var init)...) (if test (begin result...) (begin body... (loop step...))))`。
/// step 缺省取当前变量；result 与 body 缺省为未指定。
fn desugar_do(engine: &mut SchemeEngine, rest: &[Value]) -> Result<Value, SchemeError> {
    if rest.len() < 2 {
        return Err(invalid("do", "需要绑定与终止测试"));
    }
    let specs = flatten_strict_list("do", &rest[0])?;
    let test_clause = flatten_strict_list("do", &rest[1])?;
    if test_clause.is_empty() {
        return Err(invalid("do", "终止测试子句不能为空"));
    }
    let mut names: Vec<Value> = Vec::with_capacity(specs.len());
    let mut inits: Vec<Value> = Vec::with_capacity(specs.len());
    let mut steps: Vec<Value> = Vec::with_capacity(specs.len());
    for spec in specs {
        let elements = flatten_strict_list("do", &spec)?;
        if elements.is_empty() || elements.len() > 3 {
            return Err(invalid("do", "绑定是 (var init [step])"));
        }
        let name = match &elements[0] {
            Value::Symbol(name) => name.clone(),
            _ => return Err(invalid("do", "循环变量必须是符号")),
        };
        names.push(Value::Symbol(name.clone()));
        inits.push(elements[1].clone());
        steps.push(elements.get(2).cloned().unwrap_or(Value::Symbol(name)));
    }
    // test 为真分支：result 序列（可为空）。
    let result_body: Vec<Value> = test_clause[1..].to_vec();
    // test 为假分支：body 序列 + 尾调用 (loop steps...)。
    let loop_call = build_application(
        engine,
        vec![Value::Symbol("__do".into())],
        steps,
    )?;
    let mut false_body: Vec<Value> = rest[2..].to_vec();
    false_body.push(loop_call);
    let true_branch = build_application(
        engine,
        vec![Value::Symbol("begin".into())],
        result_body,
    )?;
    let false_branch = build_application(
        engine,
        vec![Value::Symbol("begin".into())],
        false_body,
    )?;
    let test_form = build_application(
        engine,
        vec![Value::Symbol("if".into())],
        vec![test_clause[0].clone(), true_branch, false_branch],
    )?;
    // named let：(let __do ((n i)...) test_form)
    let binding_pairs: Vec<Value> = names
        .iter()
        .zip(inits)
        .map(|(name, init)| build_application(engine, vec![name.clone()], vec![init]))
        .collect::<Result<_, _>>()?;
    let bindings_form = engine.list_from_slice(&binding_pairs)?;
    let let_form = engine.list_from_slice(&[
        Value::Symbol("let".into()),
        Value::Symbol("__do".into()),
        bindings_form,
        test_form,
    ])?;
    Ok(let_form)
}

// ---------- define / formals / record ----------

/// 解析 `(name initializer)` 绑定列表。
fn parse_bindings(
    form: &'static str,
    rest: &[Value],
) -> Result<Bindings, SchemeError> {
    let bindings_value = rest
        .first()
        .ok_or_else(|| invalid(form, "缺少绑定列表"))?;
    let binding_pairs = flatten_strict_list(form, bindings_value)?;
    let mut parsed = Vec::with_capacity(binding_pairs.len());
    for pair in binding_pairs {
        let elements = flatten_strict_list(form, &pair)?;
        if elements.len() != 2 {
            return Err(invalid(form, "每个绑定必须是 (name initializer)"));
        }
        let name = match &elements[0] {
            Value::Symbol(name) => name.clone(),
            _ => return Err(invalid(form, "绑定名必须是符号")),
        };
        parsed.push((name, elements[1].clone()));
    }
    Ok(parsed)
}

/// 返回 `rest[1..]` 主体；保证非空。
fn body_of<'a>(form: &'static str, rest: &'a [Value]) -> Result<&'a [Value], SchemeError> {
    if rest.len() < 2 {
        return Err(invalid(form, "缺少主体"));
    }
    Ok(&rest[1..])
}

/// 解析形式参数；支持 `(a b)`、`(a . rest)` 与 `rest`。
fn parse_formals(
    form: &'static str,
    formals: &Value,
) -> Result<Formals, SchemeError> {
    match formals {
        Value::Symbol(rest) => Ok((Vec::new(), Some(rest.clone()))),
        _ => {
            let mut parameters = Vec::new();
            let mut rest_parameter = None;
            let mut seen = BTreeSet::new();
            let mut current = formals.clone();
            loop {
                match current {
                    Value::Null => break,
                    Value::Symbol(rest) => {
                        rest_parameter = Some(rest);
                        break;
                    }
                    Value::Pair(pair) => {
                        let borrowed = pair.borrow();
                        match &borrowed.car {
                            Value::Symbol(name) => {
                                if !seen.insert(name.to_string()) {
                                    return Err(invalid(form, "形式参数名重复"));
                                }
                                parameters.push(name.clone());
                            }
                            _ => return Err(invalid(form, "形式参数必须是符号")),
                        }
                        current = borrowed.cdr.clone();
                    }
                    _ => return Err(invalid(form, "形式参数必须是符号列表")),
                }
            }
            Ok((parameters, rest_parameter))
        }
    }
}

fn make_closure(
    engine: &mut SchemeEngine,
    formals: &Value,
    body: &[Value],
    environment: Env,
) -> Result<Value, SchemeError> {
    let (parameters, rest) = parse_formals("lambda", formals)?;
    engine.new_closure(Closure {
        parameters,
        rest,
        body: body.to_vec().into(),
        env: environment,
    })
}

/// 绑定闭包参数并返回新环境。
fn bind_closure_parameters(
    engine: &mut SchemeEngine,
    closure: &Closure,
    arguments: Vec<Value>,
) -> Result<Env, SchemeError> {
    if arguments.len() < closure.parameters.len()
        || (closure.rest.is_none() && arguments.len() > closure.parameters.len())
    {
        return Err(SchemeError::ArityMismatch {
            procedure: "#<procedure>".to_string(),
            expected: match closure.rest {
                Some(_) => format!("至少 {}", closure.parameters.len()),
                None => closure.parameters.len().to_string(),
            },
            actual: arguments.len(),
        });
    }
    let scope = engine.new_env(&closure.env);
    for (name, value) in closure.parameters.iter().zip(arguments.iter()) {
        EnvNode::define(&scope, name, value.clone());
    }
    if let Some(rest) = &closure.rest {
        let mut collected = Value::Null;
        for value in arguments[closure.parameters.len()..].iter().rev() {
            collected = engine.new_pair(value.clone(), collected)?;
        }
        EnvNode::define(&scope, rest, collected);
    }
    Ok(scope)
}

/// `let-values` 系的解构绑定；formals 支持嵌套（R7RS let-values 语义）。
fn bind_formals(
    engine: &mut SchemeEngine,
    env: &Env,
    formals: &Value,
    values: &[Value],
) -> Result<(), SchemeError> {
    match formals {
        Value::Symbol(rest) => {
            let mut collected = Value::Null;
            for value in values.iter().rev() {
                collected = engine.new_pair(value.clone(), collected)?;
            }
            EnvNode::define(env, rest, collected);
            Ok(())
        }
        Value::Null => Ok(()),
        Value::Pair(pair) => {
            let (head, tail) = {
                let borrowed = pair.borrow();
                (borrowed.car.clone(), borrowed.cdr.clone())
            };
            let Some((first, rest)) = values.split_first() else {
                return Err(SchemeError::ArityMismatch {
                    procedure: "let-values".to_string(),
                    expected: "至少 1 个值".to_string(),
                    actual: values.len(),
                });
            };
            bind_formals(engine, env, &head, std::slice::from_ref(first))?;
            bind_formals(engine, env, &tail, rest)
        }
        _ => Err(invalid("let-values", "形式参数必须是符号或嵌套列表")),
    }
}

// ---------- quasiquote ----------

/// `(quasiquote template)` 求值期展开；depth 跟踪嵌套层级。
fn expand_quasiquote(
    engine: &mut SchemeEngine,
    template: &Value,
    depth: u32,
) -> Result<Value, SchemeError> {
    match template {
        Value::Pair(pair) => {
            let (head, tail) = {
                let borrowed = pair.borrow();
                (borrowed.car.clone(), borrowed.cdr.clone())
            };
            if let Value::Symbol(name) = &head {
                match name.as_ref() {
                    "unquote" if depth == 1 => {
                        let items = flatten_strict_list("unquote", &tail)?;
                        if items.len() != 1 {
                            return Err(invalid("unquote", "恰好一个表达式"));
                        }
                        return eval(engine, &items[0], &engine.globals_snapshot());
                    }
                    "unquote" => {
                        let items = flatten_strict_list("unquote", &tail)?;
                        if items.len() != 1 {
                            return Err(invalid("unquote", "恰好一个表达式"));
                        }
                        let inner = expand_quasiquote(engine, &items[0], depth - 1)?;
                        return build_application(
                            engine,
                            vec![Value::Symbol("unquote".into())],
                            vec![inner],
                        );
                    }
                    "quasiquote" => {
                        let items = flatten_strict_list("quasiquote", &tail)?;
                        if items.len() != 1 {
                            return Err(invalid("quasiquote", "恰好一个模板"));
                        }
                        let inner = expand_quasiquote(engine, &items[0], depth + 1)?;
                        return build_application(
                            engine,
                            vec![Value::Symbol("quasiquote".into())],
                            vec![inner],
                        );
                    }
                    _ => {}
                }
            }
            // 列表模板：逐元素展开，处理 unquote-splicing。
            let items = flatten_public_relaxed(template)?;
            let mut expanded: Vec<Value> = Vec::with_capacity(items.len());
            let mut tail_value: Option<Value> = None;
            for (index, item) in items.iter().enumerate() {
                if let Some((spliced, rest)) = unquote_splicing_head(item) && depth == 1 {
                    let spliced_values = eval(engine, &spliced, &engine.globals_snapshot())?;
                    let mut elements = engine.value_to_vec("unquote-splicing", &spliced_values)?;
                    expanded.append(&mut elements);
                    if let Some(rest) = rest {
                        tail_value = Some(expand_quasiquote(engine, &rest, depth)?);
                    }
                    let _ = index;
                    break;
                }
                expanded.push(expand_quasiquote(engine, item, depth)?);
            }
            let mut list = tail_value.unwrap_or(Value::Null);
            for element in expanded.into_iter().rev() {
                list = engine.new_pair(element, list)?;
            }
            Ok(list)
        }
        Value::Vector(items) => {
            let items = items.borrow().clone();
            let mut expanded: Vec<Value> = Vec::with_capacity(items.len());
            for item in &items {
                if let Some((spliced, None)) = unquote_splicing_head(item) && depth == 1 {
                    let spliced_values = eval(engine, &spliced, &engine.globals_snapshot())?;
                    let mut elements = engine.value_to_vec("unquote-splicing", &spliced_values)?;
                    expanded.append(&mut elements);
                } else {
                    expanded.push(expand_quasiquote(engine, item, depth)?);
                }
            }
            engine.new_vector_from(expanded)
        }
        _ => Ok(template.clone()),
    }
}

/// `(unquote-splicing expr)` 头部识别；返回表达式与剩余尾部（若在非严格尾）。
fn unquote_splicing_head(item: &Value) -> Option<(Value, Option<Value>)> {
    if let Value::Pair(pair) = item {
        let borrowed = pair.borrow();
        if matches!(&borrowed.car, Value::Symbol(name) if name.as_ref() == "unquote-splicing") {
            let items = flatten_public_relaxed(&borrowed.cdr).ok()?;
            let expression = items.first().cloned()?;
            let rest = if items.len() > 1 { items.last().cloned() } else { None };
            return Some((expression, rest));
        }
    }
    None
}

// ---------- parameterize / case-lambda ----------

/// `(parameterize ((p v)...) body...)` 脱糖为内部原语 + dynamic-wind。
fn desugar_parameterize(
    engine: &mut SchemeEngine,
    rest: &[Value],
) -> Result<Value, SchemeError> {
    if rest.len() < 2 {
        return Err(invalid("parameterize", "需要绑定列表与主体"));
    }
    let binding_list = flatten_strict_list("parameterize", &rest[0])?;
    let body = rest[1..].to_vec();
    let mut names: Vec<Value> = Vec::with_capacity(binding_list.len());
    let mut inits: Vec<Value> = Vec::with_capacity(binding_list.len());
    for binding in binding_list {
        let elements = flatten_strict_list("parameterize", &binding)?;
        if elements.len() != 2 {
            return Err(invalid("parameterize", "每个绑定是 (parameter value)"));
        }
        names.push(elements[0].clone());
        inits.push(elements[1].clone());
    }
    // let 绑定：t_i = value_i，s_i = ( param-ref p_i)。
    let mut let_bindings: Vec<Value> = Vec::with_capacity(names.len() * 2);
    for (index, name) in names.iter().enumerate() {
        let pair = build_application(
            engine,
            vec![name.clone()],
            vec![inits[index].clone()],
        )?;
        let_bindings.push(pair);
    }
    for name in &names {
        let ref_call = build_application(
            engine,
            vec![Value::Symbol(" param-ref".into())],
            vec![name.clone()],
        )?;
        let pair = build_application(
            engine,
            vec![Value::Symbol("__psave".into())],
            vec![ref_call],
        )?;
        let_bindings.push(pair);
    }
    // before：逐个 ( param-set! p t)；after：逐个 ( param-set! p __psave)。
    let mut before_body = Vec::with_capacity(names.len());
    let mut after_body = Vec::with_capacity(names.len());
    for (index, name) in names.iter().enumerate() {
        let set_new = build_application(
            engine,
            vec![Value::Symbol(" param-set!".into())],
            vec![
                name.clone(),
                Value::Symbol(format!("__pnew{index}").into()),
            ],
        )?;
        before_body.push(set_new);
        let restore = build_application(
            engine,
            vec![Value::Symbol(" param-set!".into())],
            vec![
                name.clone(),
                Value::Symbol(format!("__psave{index}").into()),
            ],
        )?;
        after_body.push(restore);
    }
    // before / after / thunk 的 lambda。
    let before_proc = build_lambda(engine, Vec::new(), before_body)?;
    let thunk = build_lambda(engine, Vec::new(), body)?;
    let after_proc = build_lambda(engine, Vec::new(), after_body)?;
    let wind_form = build_application(
        engine,
        vec![
            Value::Symbol("dynamic-wind".into()),
            before_proc,
            thunk,
            after_proc,
        ],
        Vec::new(),
    )?;
    // 外层 let：命名约定 t_i = __pnew{i}，s_i = __psave{i}。
    let mut flat_bindings: Vec<Value> = Vec::with_capacity(let_bindings.len());
    for (index, binding) in let_bindings.iter().enumerate() {
        let elements = flatten_strict_list("parameterize", binding)?;
        let name = if index < names.len() {
            Value::Symbol(format!("__pnew{index}").into())
        } else {
            Value::Symbol(format!("__psave{}", index - names.len()).into())
        };
        let rewritten = build_application(
            engine,
            vec![name],
            vec![elements[1].clone()],
        )?;
        flat_bindings.push(rewritten);
    }
    let bindings_form = engine.list_from_slice(&flat_bindings)?;
    let let_form = engine.list_from_slice(&[
        Value::Symbol("let".into()),
        bindings_form,
        wind_form,
    ])?;
    Ok(let_form)
}

/// `(case-lambda (formals body...)...)` 脱糖为按 arity 分派的 lambda。
fn desugar_case_lambda(
    engine: &mut SchemeEngine,
    rest: &[Value],
) -> Result<Value, SchemeError> {
    if rest.is_empty() {
        return Err(invalid("case-lambda", "至少一个子句"));
    }
    let args_symbol: Rc<str> = "__case-args".into();
    let mut clauses: Vec<(Option<usize>, Value)> = Vec::with_capacity(rest.len());
    let mut variadic: Option<Value> = None;
    for clause in rest {
        let elements = flatten_strict_list("case-lambda", &clause)?;
        if elements.len() < 2 {
            return Err(invalid("case-lambda", "子句是 (formals body...)"));
        }
        let formals = elements[0].clone();
        let body = elements[1..].to_vec();
        let branch = build_lambda(engine, vec![formals.clone()], body)?;
        match &formals {
            Value::Symbol(_) => variadic = Some(branch),
            other => {
                let items = flatten_strict_list("case-lambda", other)?;
                let arity = items.len();
                if clauses.iter().any(|(existing, _)| *existing == Some(arity)) {
                    return Err(invalid("case-lambda", "固定参数数量重复"));
                }
                clauses.push((Some(arity), branch));
            }
        }
    }
    // cond 子句：((= (length __case-args) n) (apply proc __case-args))。
    let mut cond_clauses: Vec<Value> = Vec::with_capacity(clauses.len() + 1);
    for (arity, branch) in &clauses {
        let Some(arity) = arity else { continue };
        let length_call = build_application(
            engine,
            vec![Value::Symbol("length".into())],
            vec![Value::Symbol(args_symbol.clone())],
        )?;
        let test = build_application(
            engine,
            vec![
                Value::Symbol("=".into()),
                length_call,
                Value::Fixnum(*arity as i64),
            ],
            Vec::new(),
        )?;
        let application = build_application(
            engine,
            vec![
                Value::Symbol("apply".into()),
                branch.clone(),
                Value::Symbol(args_symbol.clone()),
            ],
            Vec::new(),
        )?;
        let clause = engine.list_from_slice(&[test, application])?;
        cond_clauses.push(clause);
    }
    let mut else_body: Vec<Value> = Vec::new();
    match variadic {
        Some(branch) => {
            let application = build_application(
                engine,
                vec![
                    Value::Symbol("apply".into()),
                    branch,
                    Value::Symbol(args_symbol.clone()),
                ],
                Vec::new(),
            )?;
            else_body.push(application);
        }
        None => {
            let message = engine.new_string_from("case-lambda: 没有匹配的参数数量".to_string())?;
            let failure = build_application(
                engine,
                vec![Value::Symbol("error".into()), message],
                Vec::new(),
            )?;
            else_body.push(failure);
        }
    }
    let else_clause = engine.list_from_slice(&{
        let mut items = vec![Value::Symbol("else".into())];
        items.extend(else_body);
        items
    })?;
    cond_clauses.push(else_clause);
    let cond_form = {
        let mut items = vec![Value::Symbol("cond".into())];
        items.extend(cond_clauses);
        engine.list_from_slice(&items)?
    };
    build_lambda(engine, vec![Value::Symbol(args_symbol)], vec![cond_form])
}

// ---------- include / cond-expand ----------

/// `(include "path")`：经引擎注入的来源解析器读取并拼接顶层表达式。
fn eval_include(
    engine: &mut SchemeEngine,
    rest: &[Value],
) -> Result<Vec<Value>, SchemeError> {
    let mut statements = Vec::new();
    for item in rest {
        let Value::String(cell) = item else {
            return Err(invalid("include", "来源必须是字符串路径"));
        };
        let path = cell.borrow().clone();
        let Some(source) = engine.include_source(&path) else {
            return Err(SchemeError::IncludeNotFound { path });
        };
        let data = super::reader::read_all(engine, &source)?;
        statements.extend(data);
    }
    Ok(statements)
}

/// `(cond-expand (feature-test body...) ... (else ...))`。
/// feature 标识符冻结：r7rs、uix-extension、exact-closed、ieee-float、
/// ratios、full-unicode；`(library (name...))` 查已登记库。
fn select_cond_expand(
    engine: &mut SchemeEngine,
    clauses: &[Value],
) -> Result<Option<Vec<Value>>, SchemeError> {
    const FEATURES: &[&str] = &[
        "r7rs",
        "uix-extension",
        "exact-closed",
        "ieee-float",
        "ratios",
        "full-unicode",
    ];
    for clause in clauses {
        let elements = flatten_strict_list("cond-expand", clause)?;
        if elements.is_empty() {
            return Err(invalid("cond-expand", "子句不能为空"));
        }
        let is_else = matches!(&elements[0], Value::Symbol(name) if name.as_ref() == "else");
        if is_else {
            return Ok(Some(elements[1..].to_vec()));
        }
        if feature_test_satisfied(engine, &elements[0], FEATURES)? {
            return Ok(Some(elements[1..].to_vec()));
        }
    }
    // 无匹配且无 else：R7RS 规定为错误。
    Err(invalid("cond-expand", "没有满足的子句且缺少 else"))
}

fn feature_test_satisfied(
    engine: &SchemeEngine,
    test: &Value,
    features: &[&str],
) -> Result<bool, SchemeError> {
    match test {
        Value::Symbol(name) => Ok(features.contains(&name.as_ref())),
        Value::Pair(_) => {
            let items = flatten_strict_list("cond-expand", test)?;
            let Some(Value::Symbol(operator)) = items.first() else {
                return Err(invalid("cond-expand", "组合测试必须是 and/or/not/library"));
            };
            match operator.as_ref() {
                "and" => {
                    for sub in &items[1..] {
                        if !feature_test_satisfied(engine, sub, features)? {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                }
                "or" => {
                    for sub in &items[1..] {
                        if feature_test_satisfied(engine, sub, features)? {
                            return Ok(true);
                        }
                    }
                    Ok(false)
                }
                "not" => {
                    let Some(sub) = items.get(1) else {
                        return Err(invalid("cond-expand", "not 需要一个测试"));
                    };
                    Ok(!feature_test_satisfied(engine, sub, features)?)
                }
                "library" => {
                    let Some(spec) = items.get(1) else {
                        return Err(invalid("cond-expand", "library 需要库名"));
                    };
                    let name = library_name_of(spec)?;
                    Ok(SchemeEngine::is_standard_library(&name)
                        || engine.has_library(&name))
                }
                other => Err(invalid("cond-expand", {
                    let _ = other;
                    "未知组合测试"
                })),
            }
        }
        _ => Err(invalid("cond-expand", "测试必须是符号或列表")),
    }
}

// ---------- define-record-type ----------

/// `define-record-type <type> (ctor field...) pred (field acc [mod])...`。
///
/// 展开为经内部支撑原语合成的闭包；名称带前导空格的原语不能被 reader
/// 产生，record 的构造、判定与字段访问因此全部走标准求值路径（燃料、
/// 取消与帧数边界全程受控）。
fn eval_define_record_type(
    engine: &mut SchemeEngine,
    rest: &[Value],
    environment: &Env,
) -> Result<Value, SchemeError> {
    const RECORD_MAKE: &str = " make-record";
    const RECORD_REF: &str = " record-ref";
    const RECORD_SET: &str = " record-set!";
    const RECORD_PRED: &str = " record-pred";
    if rest.len() < 3 {
        return Err(invalid("define-record-type", "缺少类型名、构造器或谓词"));
    }
    let type_name = match &rest[0] {
        Value::Symbol(name) => name.clone(),
        _ => return Err(invalid("define-record-type", "类型名必须是符号")),
    };
    let constructor_spec = flatten_strict_list("define-record-type", &rest[1])?;
    if constructor_spec.is_empty() {
        return Err(invalid("define-record-type", "构造器规格不能为空"));
    }
    let constructor_name = match &constructor_spec[0] {
        Value::Symbol(name) => name.clone(),
        _ => return Err(invalid("define-record-type", "构造器名必须是符号")),
    };
    let mut constructor_fields = Vec::with_capacity(constructor_spec.len() - 1);
    for parameter in &constructor_spec[1..] {
        match parameter {
            Value::Symbol(name) => constructor_fields.push(name.clone()),
            _ => return Err(invalid("define-record-type", "构造器参数必须是符号")),
        }
    }
    let predicate_name = match &rest[2] {
        Value::Symbol(name) => name.clone(),
        _ => return Err(invalid("define-record-type", "谓词必须是符号")),
    };
    let mut field_names: Vec<Rc<str>> = Vec::new();
    let mut field_specs: Vec<(Rc<str>, Option<Rc<str>>)> = Vec::new();
    for spec in &rest[3..] {
        let elements = flatten_strict_list("define-record-type", spec)?;
        if elements.len() < 2 || elements.len() > 3 {
            return Err(invalid("define-record-type", "字段规格是 (field accessor [modifier])"));
        }
        let field = match &elements[0] {
            Value::Symbol(name) => name.clone(),
            _ => return Err(invalid("define-record-type", "字段名必须是符号")),
        };
        let accessor = match &elements[1] {
            Value::Symbol(name) => name.clone(),
            _ => return Err(invalid("define-record-type", "访问器必须是符号")),
        };
        let modifier = match elements.get(2) {
            Some(Value::Symbol(name)) => Some(name.clone()),
            Some(_) => return Err(invalid("define-record-type", "修改器必须是符号")),
            None => None,
        };
        field_names.push(field);
        field_specs.push((accessor, modifier));
    }
    let mut constructor_indices: Vec<Value> = Vec::with_capacity(constructor_fields.len());
    let mut seen = BTreeSet::new();
    for parameter in &constructor_fields {
        let index = field_names
            .iter()
            .position(|name| name == parameter)
            .ok_or_else(|| invalid("define-record-type", "构造器参数必须是声明字段"))?;
        if !seen.insert(index) {
            return Err(invalid("define-record-type", "构造器参数重复"));
        }
        constructor_indices.push(Value::Fixnum(index as i64));
    }
    let mut field_name_set = BTreeSet::new();
    for name in &field_names {
        if !field_name_set.insert(name.to_string()) {
            return Err(invalid("define-record-type", "字段名重复"));
        }
    }

    // 构造器：((空格)make-record 'type #(indices) 参数...)。
    let mut constructor_body = vec![
        Value::Symbol(Rc::from(RECORD_MAKE)),
        super::reader::wrap_with_quote(engine, Value::Symbol(type_name.clone()))?,
    ];
    let indices_vector = engine.new_vector_from(constructor_indices)?;
    constructor_body.push(super::reader::wrap_with_quote(engine, indices_vector)?);
    constructor_body.extend(
        constructor_fields
            .iter()
            .map(|name| Value::Symbol(name.clone())),
    );
    let formals = build_symbol_list(engine, &constructor_fields)?;
    let constructor_body_form = engine.list_from_slice(&constructor_body)?;
    let constructor = make_closure(engine, &formals, &[constructor_body_form], environment.clone())?;
    EnvNode::define(environment, &constructor_name, constructor);

    // 谓词。
    let mut predicate_elements = vec![
        Value::Symbol(Rc::from(RECORD_PRED)),
        Value::Symbol("record".into()),
    ];
    predicate_elements
        .push(super::reader::wrap_with_quote(engine, Value::Symbol(type_name.clone()))?);
    let predicate_body = engine.list_from_slice(&predicate_elements)?;
    let predicate_formals = build_symbol_list(engine, &["record".into()])?;
    let predicate = make_closure(
        engine,
        &predicate_formals,
        &[predicate_body],
        environment.clone(),
    )?;
    EnvNode::define(environment, &predicate_name, predicate);

    // 访问器与可选修改器。
    for (index, (accessor, modifier)) in field_specs.iter().enumerate() {
        let mut accessor_elements = vec![
            Value::Symbol(Rc::from(RECORD_REF)),
            Value::Symbol("record".into()),
        ];
        accessor_elements
            .push(super::reader::wrap_with_quote(engine, Value::Symbol(type_name.clone()))?);
        accessor_elements.push(Value::Fixnum(index as i64));
        let accessor_body = engine.list_from_slice(&accessor_elements)?;
        let accessor_formals = build_symbol_list(engine, &["record".into()])?;
        let accessor_procedure = make_closure(
            engine,
            &accessor_formals,
            &[accessor_body],
            environment.clone(),
        )?;
        EnvNode::define(environment, accessor, accessor_procedure);
        if let Some(modifier) = modifier {
            let mut modifier_elements = vec![
                Value::Symbol(Rc::from(RECORD_SET)),
                Value::Symbol("record".into()),
            ];
            modifier_elements
                .push(super::reader::wrap_with_quote(engine, Value::Symbol(type_name.clone()))?);
            modifier_elements.push(Value::Fixnum(index as i64));
            modifier_elements.push(Value::Symbol("value".into()));
            let modifier_body = engine.list_from_slice(&modifier_elements)?;
            let modifier_formals =
                build_symbol_list(engine, &["record".into(), "value".into()])?;
            let modifier_procedure = make_closure(
                engine,
                &modifier_formals,
                &[modifier_body],
                environment.clone(),
            )?;
            EnvNode::define(environment, modifier, modifier_procedure);
        }
    }
    Ok(Value::Unspecified)
}

fn build_symbol_list(
    engine: &mut SchemeEngine,
    names: &[Rc<str>],
) -> Result<Value, SchemeError> {
    let symbols: Vec<Value> = names
        .iter()
        .map(|name| Value::Symbol(name.clone()))
        .collect();
    engine.list_from_slice(&symbols)
}

// ---------- 库系统 ----------

/// 库名 `(a b c)` 规范化为 `a/b/c`。
fn library_name_of(form: &Value) -> Result<String, SchemeError> {
    let items = flatten_strict_list("define-library", form)?;
    let mut parts = Vec::with_capacity(items.len());
    for item in items {
        match item {
            Value::Symbol(part) => parts.push(part.to_string()),
            Value::Fixnum(index) => parts.push(index.to_string()),
            _ => {
                return Err(SchemeError::InvalidLibrary {
                    reason: "库名段必须是符号或整数",
                })
            }
        }
    }
    if parts.is_empty() {
        return Err(SchemeError::InvalidLibrary {
            reason: "库名不能为空",
        });
    }
    Ok(parts.join("/"))
}

/// `(define-library (name ...) (import ...) (export ...) (begin ...))`。
///
/// 库体在以全局环境为父的独立命名环境求值；只有 export 声明的绑定经
/// `import` 可见，库内定义不泄漏。声明按序处理；`include` 以 begin 表达
/// 式并入（include 声明拼接为已知近似，登记于兼容矩阵）。
fn eval_define_library(
    engine: &mut SchemeEngine,
    rest: &[Value],
    environment: &Env,
) -> Result<Value, SchemeError> {
    let _ = environment;
    if rest.is_empty() {
        return Err(SchemeError::InvalidLibrary {
            reason: "缺少库名",
        });
    }
    let name = library_name_of(&rest[0])?;
    let library_env = engine.new_env(&engine.globals_snapshot());
    let mut exports = BTreeSet::new();
    for declaration in &rest[1..] {
        let items = flatten_strict_list("define-library", declaration)?;
        let Some(Value::Symbol(head)) = items.first() else {
            return Err(SchemeError::InvalidLibrary {
                reason: "声明必须以符号开头",
            });
        };
        match head.as_ref() {
            "export" => {
                for item in &items[1..] {
                    match item {
                        Value::Symbol(exported) => {
                            exports.insert(exported.to_string());
                        }
                        _ => {
                            return Err(SchemeError::InvalidLibrary {
                                reason: "导出名必须是符号",
                            })
                        }
                    }
                }
            }
            "import" => {
                import_declarations(engine, &items[1..], &library_env)?;
            }
            "begin" => {
                for statement in &items[1..] {
                    eval(engine, statement, &library_env)?;
                }
            }
            "include" | "include-ci" => {
                let statements = eval_include(engine, &items[1..])?;
                for statement in statements {
                    eval(engine, &statement, &library_env)?;
                }
            }
            "cond-expand" => {
                let Some(selected) = select_cond_expand(engine, &items[1..])? else {
                    continue;
                };
                for declaration in selected {
                    let inner = flatten_strict_list("define-library", &declaration)?;
                    let Some(Value::Symbol(inner_head)) = inner.first() else {
                        return Err(SchemeError::InvalidLibrary {
                            reason: "声明必须以符号开头",
                        });
                    };
                    match inner_head.as_ref() {
                        "export" => {
                            for item in &inner[1..] {
                                if let Value::Symbol(exported) = item {
                                    exports.insert(exported.to_string());
                                }
                            }
                        }
                        "import" => {
                            import_declarations(engine, &inner[1..], &library_env)?;
                        }
                        "begin" => {
                            for statement in &inner[1..] {
                                eval(engine, statement, &library_env)?;
                            }
                        }
                        "include" | "include-ci" => {
                            let statements = eval_include(engine, &inner[1..])?;
                            for statement in statements {
                                eval(engine, &statement, &library_env)?;
                            }
                        }
                        _ => {
                            return Err(SchemeError::InvalidLibrary {
                                reason: "cond-expand 内只支持 export/import/begin/include",
                            })
                        }
                    }
                }
            }
            "include-library-declarations" => {
                return Err(SchemeError::NotImplemented {
                    feature: "include-library-declarations",
                });
            }
            other => {
                let _ = other;
                return Err(SchemeError::InvalidLibrary {
                    reason: "未知库声明",
                });
            }
        }
    }
    engine.register_library(name, library_env, exports);
    Ok(Value::Unspecified)
}

/// 顶层 `(import (lib ...) ...)`：把库导出绑定进当前环境。
fn eval_import(
    engine: &mut SchemeEngine,
    rest: &[Value],
    environment: &Env,
) -> Result<Value, SchemeError> {
    import_declarations(engine, rest, environment)?;
    Ok(Value::Unspecified)
}

/// import 声明：裸库名与 `only` / `except` / `prefix` / `rename` 修饰符。
fn import_declarations(
    engine: &mut SchemeEngine,
    specifications: &[Value],
    target: &Env,
) -> Result<(), SchemeError> {
    for specification in specifications {
        let items = flatten_strict_list("import", specification)?;
        if items.is_empty() {
            return Err(SchemeError::InvalidLibrary {
                reason: "import 声明不能为空",
            });
        }
        // 修饰符形式：(only (lib) name...) / (except (lib) name...) /
        // (prefix (lib) prefix) / (rename (lib) (from to)...)。
        if let Value::Symbol(modifier) = &items[0] {
            match modifier.as_ref() {
                "only" | "except" | "prefix" | "rename" => {
                    if items.len() < 2 {
                        return Err(SchemeError::InvalidLibrary {
                            reason: "import 修饰符需要库名",
                        });
                    }
                    let name = library_name_of(&items[1])?;
                    let bindings = resolve_library(engine, &name)?;
                    match modifier.as_ref() {
                        "only" => {
                            let selected = symbol_set("only", &items[2..])?;
                            for (exported, value) in bindings {
                                if selected.contains(exported.as_str()) {
                                    let key: Rc<str> = Rc::from(exported);
                                    EnvNode::define(target, &key, value);
                                }
                            }
                        }
                        "except" => {
                            let excluded = symbol_set("except", &items[2..])?;
                            for (exported, value) in bindings {
                                if !excluded.contains(exported.as_str()) {
                                    let key: Rc<str> = Rc::from(exported);
                                    EnvNode::define(target, &key, value);
                                }
                            }
                        }
                        "prefix" => {
                            let [Value::Symbol(prefix)] = &items[2..] else {
                                return Err(SchemeError::InvalidLibrary {
                                    reason: "prefix 需要一个前缀符号",
                                });
                            };
                            let prefixed: String = format!("{prefix}/");
                            for (exported, value) in bindings {
                                let key: Rc<str> =
                                    Rc::from(format!("{prefixed}{exported}"));
                                EnvNode::define(target, &key, value);
                            }
                        }
                        "rename" => {
                            let mut renames = std::collections::BTreeMap::new();
                            for pair in &items[2..] {
                                let elements = flatten_strict_list("rename", pair)?;
                                if elements.len() != 2 {
                                    return Err(SchemeError::InvalidLibrary {
                                        reason: "rename 对必须是 (from to)",
                                    });
                                }
                                let (Value::Symbol(from), Value::Symbol(to)) =
                                    (&elements[0], &elements[1])
                                else {
                                    return Err(SchemeError::InvalidLibrary {
                                        reason: "rename 对必须是符号",
                                    });
                                };
                                renames.insert(from.to_string(), to.to_string());
                            }
                            for (exported, value) in bindings {
                                let renamed = renames
                                    .get(exported.as_str())
                                    .cloned()
                                    .unwrap_or_else(|| exported.clone());
                                let key: Rc<str> = Rc::from(renamed);
                                EnvNode::define(target, &key, value);
                            }
                        }
                        // 契约：修饰符分支在外层 match 已穷尽，此处不可达。
                        _ => unreachable!("修饰符已匹配"),
                    }
                    continue;
                }
                _ => {}
            }
        }
        // 裸库名。
        let name = library_name_of(specification)?;
        if SchemeEngine::is_standard_library(&name) {
            // 标准库分组已随宿主环境预导入，幂等成功。
            continue;
        }
        let bindings = resolve_library(engine, &name)?;
        for (exported, value) in bindings {
            let key: Rc<str> = Rc::from(exported);
            EnvNode::define(target, &key, value);
        }
    }
    Ok(())
}

fn resolve_library(
    engine: &SchemeEngine,
    name: &str,
) -> Result<Vec<(String, Value)>, SchemeError> {
    let Some(library) = engine.library_exports(name) else {
        return Err(SchemeError::LibraryNotFound { name: name.to_string() });
    };
    Ok(library
        .exports()
        .map(|(exported, value)| (exported.to_string(), value))
        .collect())
}

fn symbol_set(
    form: &'static str,
    items: &[Value],
) -> Result<BTreeSet<String>, SchemeError> {
    let mut set = BTreeSet::new();
    for item in items {
        match item {
            Value::Symbol(name) => {
                set.insert(name.to_string());
            }
            _ => {
                return Err(SchemeError::InvalidLibrary {
                    reason: match form {
                        "only" => "only 列表必须是符号",
                        _ => "except 列表必须是符号",
                    },
                })
            }
        }
    }
    Ok(set)
}
