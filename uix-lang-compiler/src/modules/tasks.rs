//! 共享 do AST 降低为控制流块；运行器把多个块合并为 await 之间的事务。

use super::check::Scope;
use super::*;
use std::collections::BTreeSet;

pub(super) fn check(
    signature: Signature,
    exported: bool,
    body: ActionBody,
    awaits: &[usize],
    scope: Scope<'_>,
    span: SourceSpan,
    ports: &[Signature],
) -> Result<Task, Diagnostic> {
    let body = match body {
        ActionBody::Block(block) => block,
        ActionBody::Expression(value) => ActionBlock {
            statements: vec![ActionStatement::Return {
                value: Some(value),
                span,
            }],
            span,
        },
    };
    let mut builder = Builder {
        scope,
        awaits: awaits.iter().copied().collect(),
        ports,
        stages: vec![],
    };
    let fragment = builder.block(&body, false)?;
    if !fragment.tails.is_empty() && signature.returns != Type::Unit {
        return Err(fail(span, "AsyncCommand 非 Unit 返回必须覆盖所有路径"));
    }
    if !fragment.tails.is_empty() {
        let finish = builder.stage(vec![], TaskExit::Finish, span)?;
        for tail in fragment.tails {
            builder.connect(tail, finish);
        }
    }
    if !builder.awaits.is_empty() {
        return Err(fail(span, "await 只能用于 AsyncCommand 的 let 绑定"));
    }
    Ok(Task {
        signature,
        exported,
        local_count: builder.scope.next_slot,
        stages: builder.stages,
    })
}

struct Fragment {
    entry: usize,
    tails: Vec<usize>,
}
struct Builder<'a> {
    scope: Scope<'a>,
    awaits: BTreeSet<usize>,
    ports: &'a [Signature],
    stages: Vec<TaskStage>,
}
impl Builder<'_> {
    fn stage(
        &mut self,
        statements: Vec<Statement>,
        exit: TaskExit,
        span: SourceSpan,
    ) -> Result<usize, Diagnostic> {
        if self.stages.len() >= 4096 {
            return Err(fail(span, "异步控制流超过 4096 块"));
        }
        let index = self.stages.len();
        self.stages.push(TaskStage {
            body: TaskBody::Dynamic { statements, exit },
            location: location(self.scope.source, span),
        });
        Ok(index)
    }
    fn set_exit(&mut self, index: usize, value: TaskExit) {
        if let TaskBody::Dynamic { exit, .. } = &mut self.stages[index].body {
            *exit = value;
        }
    }
    fn connect(&mut self, tail: usize, next: usize) {
        if let TaskBody::Dynamic { exit, .. } = &mut self.stages[tail].body {
            match exit {
                TaskExit::Await { next: target, .. } => *target = next,
                _ => *exit = TaskExit::Jump(next),
            }
        }
    }
    fn block(&mut self, block: &ActionBlock, restore: bool) -> Result<Fragment, Diagnostic> {
        let before = self.scope.locals.clone();
        let mut result: Option<Fragment> = None;
        for statement in &block.statements {
            if result
                .as_ref()
                .is_some_and(|fragment| fragment.tails.is_empty())
            {
                return Err(fail(block.span, "return 后的语句不可达"));
            }
            let next = self.statement(statement)?;
            result = Some(match result {
                None => next,
                Some(previous) => {
                    for tail in previous.tails {
                        self.connect(tail, next.entry);
                    }
                    Fragment {
                        entry: previous.entry,
                        tails: next.tails,
                    }
                }
            });
        }
        if restore {
            self.scope.locals = before;
        }
        match result {
            Some(result) => Ok(result),
            None => {
                let entry = self.stage(vec![], TaskExit::Jump(usize::MAX), block.span)?;
                Ok(Fragment {
                    entry,
                    tails: vec![entry],
                })
            }
        }
    }
    fn statement(&mut self, statement: &ActionStatement) -> Result<Fragment, Diagnostic> {
        if let ActionStatement::If {
            condition,
            then_block,
            else_block,
            span,
        } = statement
        {
            let condition = self.scope.expr(condition, Some(&Type::Bool))?;
            let entry = self.stage(vec![], TaskExit::Finish, *span)?;
            let yes = self.block(then_block, true)?;
            let no = self.block(
                else_block.as_ref().unwrap_or(&ActionBlock {
                    statements: vec![],
                    span: *span,
                }),
                true,
            )?;
            self.set_exit(
                entry,
                TaskExit::Branch {
                    condition,
                    yes: yes.entry,
                    no: no.entry,
                },
            );
            return Ok(Fragment {
                entry,
                tails: yes.tails.into_iter().chain(no.tails).collect(),
            });
        }
        if let ActionStatement::Let {
            name,
            initializer,
            span,
            ..
        } = statement
        {
            if self.awaits.remove(&span.start) {
                let ExpressionKind::Call { callee, arguments } = &initializer.kind else {
                    return Err(fail(*span, "await 需要异步宿主端口调用"));
                };
                let ExpressionKind::Identifier(port) = &callee.kind else {
                    return Err(fail(*span, "await 目标必须是显式端口名称"));
                };
                let signature = self
                    .ports
                    .iter()
                    .find(|signature| signature.name == *port && signature.asynchronous)
                    .ok_or_else(|| fail(*span, "await 只能等待声明的异步宿主端口"))?;
                let args = arguments_for(&mut self.scope, arguments, signature, *span)?;
                let slot = self.scope.bind(name, signature.returns.clone(), *span)?;
                let entry = self.stage(
                    vec![],
                    TaskExit::Await {
                        name: port.clone(),
                        arguments: args,
                        next: usize::MAX,
                        slot,
                    },
                    *span,
                )?;
                return Ok(Fragment {
                    entry,
                    tails: vec![entry],
                });
            }
        }
        let span = match statement {
            ActionStatement::Let { span, .. }
            | ActionStatement::Assign { span, .. }
            | ActionStatement::Expression { span, .. }
            | ActionStatement::Return { span, .. }
            | ActionStatement::If { span, .. } => *span,
        };
        let (body, returns) = self.scope.block_scoped(
            &ActionBlock {
                statements: vec![statement.clone()],
                span,
            },
            false,
        )?;
        let entry = self.stage(body, TaskExit::Jump(usize::MAX), span)?;
        Ok(Fragment {
            entry,
            tails: if returns { vec![] } else { vec![entry] },
        })
    }
}

pub(super) fn arguments_for(
    scope: &mut Scope<'_>,
    arguments: &[CallArgument],
    signature: &Signature,
    span: SourceSpan,
) -> Result<Vec<Expr>, Diagnostic> {
    if arguments.len() != signature.parameters.len()
        || arguments.iter().any(|argument| argument.name.is_some())
    {
        return Err(fail(span, "异步调用参数数量或形式不符"));
    }
    arguments
        .iter()
        .zip(&signature.parameters)
        .map(|(argument, (_, ty))| scope.expr(&argument.value, Some(ty)))
        .collect()
}
