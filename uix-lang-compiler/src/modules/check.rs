//! 模块声明、函数签名、词法作用域及状态事务的语义检查。

use super::*;

pub(super) fn module(document: &Document, source: &str) -> Result<(Module, Vec<ModuleSymbol>), Diagnostic> {
    let root = &document.root;
    if root.name != "Module" { return Err(fail(root.span, "应用模块入口必须是 <Module>")); }
    if !document.declarations.is_empty() { return Err(fail(root.span, "模块声明放在 Module 内；不隐式绑定外部 Rust 声明")); }
    attributes(root, &["name", "version", "schema"])?;
    let name = required(root, "name")?.to_string();
    identifier(&name, root.span)?;
    let version = required(root, "version")?.to_string();
    if version.is_empty() { return Err(fail(root.span, "模块版本不能为空")); }
    let schema = required(root, "schema")?.parse::<u64>().map_err(|_| fail(root.span, "schema 必须是非负整数"))?;
    let mut nodes = Vec::new();
    for node in &root.children {
        match node {
            Node::Element(element) => nodes.push(element),
            Node::Text(t) if t.value.trim().is_empty() => {},
            _ => return Err(fail(root.span, "Module 只允许类型、状态、端口与函数声明")),
        }
    }
    let mut records = BTreeMap::new();
    for element in &nodes {
        if element.name != "Data" { continue; }
        attributes(element, &["name", "fields"])?;
        let name = required(element, "name")?;
        identifier(name, element.span)?;
        if records.contains_key(name) { return Err(fail(element.span, "重复数据类型")); }
        let fields = parameters(required(element, "fields")?, &records, element.span)?;
        records.insert(name.to_string(), Type::Record(fields.into_iter().collect()));
    }
    let mut states = Vec::new();
    let mut functions = Vec::new();
    let mut ports = Vec::new();
    let mut signatures = BTreeMap::new();
    let mut state_types = BTreeMap::new();
    let mut symbols = Vec::new();
    for element in &nodes {
        if element.name == "View" { continue; }
        let name = required(element, "name")?;
        identifier(name, element.span)?;
        if !element.children.iter().all(|n| matches!(n, Node::Text(t) if t.value.trim().is_empty())) {
            return Err(fail(element.span, "声明必须自闭合，主体使用 body 属性"));
        }
        match element.name.as_str() {
            "Data" => {},
            "State" => {
                attributes(element, &["name", "type", "value"])?;
                if state_types.contains_key(name) || signatures.contains_key(name) { return Err(fail(element.span, "模块名称重复")); }
                let ty = parse_type(required(element, "type")?, &records, element.span)?;
                let value = element.attributes.iter().find(|a| a.name == "value")
                    .ok_or_else(|| fail(element.span, "状态缺少 value"))?;
                let ast = match &value.value {
                    AttributeValue::Expression(v) => v.expression.clone(),
                    AttributeValue::Literal(v) => Expression { kind: ExpressionKind::String(v.clone()), span: value.span },
                    _ => return Err(fail(value.span, "状态初值必须是值表达式")),
                };
                let mut scope = Scope::new(source, &signatures, &state_types, Effect::Pure, Type::Unit);
                let expr = scope.expr(&ast, Some(&ty))?;
                let initial = constant(&expr).ok_or_else(|| fail(value.span, "状态初值必须是常量值图"))?;
                initial.validate_budget(1_048_576, 65_536).map_err(|e| fail(value.span, e.message))?;
                state_types.insert(name.to_string(), (states.len(), ty.clone()));
                states.push(StateField { name: name.to_string(), ty, initial });
            }
            "Function" | "Command" | "AsyncCommand" | "Query" | "Port" => {
                attributes(element, if element.name == "Port" { &["name", "params", "returns", "effect", "async"] }
                    else { &["name", "params", "returns", "body", "export"] })?;
                if signatures.contains_key(name) || state_types.contains_key(name) { return Err(fail(element.span, "模块名称重复")); }
                let effect = match element.name.as_str() {
                    "Function" => Effect::Pure, "Query" => Effect::Query, "Command" | "AsyncCommand" => Effect::Command,
                    _ => match required(element, "effect")? {
                        "query" => Effect::Query, "command" => Effect::Command,
                        _ => return Err(fail(element.span, "端口 effect 只能是 query 或 command")),
                    },
                };
                let signature = Signature { name: name.to_string(), effect,
                    parameters: parameters(attr(element, "params")?.unwrap_or(""), &records, element.span)?,
                    returns: parse_type(required(element, "returns")?, &records, element.span)?,
                    asynchronous: if element.name == "AsyncCommand" { true } else { match attr(element, "async")?.unwrap_or("false") {
                        "true" => true, "false" => false, _ => return Err(fail(element.span, "async 必须是 true 或 false")),
                    } } };
                // 当前运行器只执行同步函数。保留异步合同字段，但不能接受没有
                // 调度器与 AOT 支持的入口，更不能把 await 静默降级为同步调用。
                if signature.asynchronous {
                    return Err(fail(element.span, "当前应用模块运行器尚不支持 AsyncCommand 或异步端口"));
                }
                signatures.insert(name.to_string(), signature.clone());
                if element.name == "Port" { ports.push(signature); }
            }
            other => return Err(fail(element.span, format!("未知模块声明 {other}"))),
        }
        symbols.push(ModuleSymbol { name: name.to_string(), detail: element.name.clone(), start: element.span.start, end: element.span.end });
    }
    for element in &nodes {
        if !matches!(element.name.as_str(), "Function" | "Command" | "AsyncCommand" | "Query") { continue; }
        let name = required(element, "name")?;
        let signature = signatures[name].clone();
        let body_attribute = element.attributes.iter().find(|a| a.name == "body")
            .ok_or_else(|| fail(element.span, "函数缺少 body"))?;
        let body = parse_module_body(required(element, "body")?.trim(), body_attribute.span)?;
        let mut scope = Scope::new(source, &signatures, &state_types, signature.effect, signature.returns.clone());
        for (name, ty) in &signature.parameters { scope.bind(name, ty.clone(), element.span)?; }
        let exported = match attr(element, "export")?.unwrap_or(if element.name == "Function" { "false" } else { "true" }) {
            "true" => true, "false" => false, _ => return Err(fail(element.span, "export 必须为 true 或 false")),
        };
        if !body.awaits.is_empty() { return Err(fail(element.span, "当前应用模块运行器尚不支持 await")); }
        let (body, returns) = match body.body {
            ActionBody::Expression(expr) => (vec![Statement::Return(Some(scope.expr(&expr, Some(&signature.returns))?))], true),
            ActionBody::Block(block) => scope.block(&block)?,
        };
        if signature.returns != Type::Unit && !returns { return Err(fail(element.span, "非 Unit 函数必须在所有路径返回值")); }
        functions.push(Function { signature, exported, body: Body::Dynamic(body), local_count: scope.next_slot, location: location(source, element.span) });
    }
    let views: Vec<_> = nodes.iter().filter(|e| e.name == "View").collect();
    if views.len() > 1 { return Err(fail(root.span, "模块只允许一个 View")); }
    let view = views.first().map(|element| super::view::check(element, source, &signatures, &state_types, &mut functions)).transpose()?;
    Ok((Module { name, version, state_schema: schema, states, functions, ports, view, tasks: Vec::new() }, symbols))
}

fn parameters(source: &str, records: &BTreeMap<String, Type>, span: SourceSpan) -> Result<Vec<(String, Type)>, Diagnostic> {
    let mut parameters = Vec::new();
    for entry in split_types(source) {
        let (name, ty) = entry.split_once(':').ok_or_else(|| fail(span, "字段或参数需要 name: Type"))?;
        let name = name.trim();
        identifier(name, span)?;
        if parameters.iter().any(|(n, _)| n == name) { return Err(fail(span, "字段或参数名称重复")); }
        parameters.push((name.to_string(), parse_type(ty, records, span)?));
    }
    Ok(parameters)
}

pub(super) struct Scope<'a> {
    pub source: &'a str,
    pub signatures: &'a BTreeMap<String, Signature>,
    pub states: &'a BTreeMap<String, (usize, Type)>,
    pub locals: BTreeMap<String, (usize, Type)>,
    pub next_slot: usize,
    pub effect: Effect,
    pub returns: Type,
    pub view: bool,
}
impl<'a> Scope<'a> {
    pub(super) fn new(source: &'a str, signatures: &'a BTreeMap<String, Signature>, states: &'a BTreeMap<String, (usize, Type)>, effect: Effect, returns: Type) -> Self {
        Self { source, signatures, states, locals: BTreeMap::new(), next_slot: 0, effect, returns, view: false }
    }
    pub fn bind(&mut self, name: &str, ty: Type, span: SourceSpan) -> Result<usize, Diagnostic> {
        identifier(name, span)?;
        if self.locals.contains_key(name) || self.states.contains_key(name) || self.signatures.contains_key(name) {
            return Err(fail(span, format!("绑定 {name} 重复或遮蔽模块名称")));
        }
        let slot = self.next_slot;
        self.next_slot += 1;
        self.locals.insert(name.to_string(), (slot, ty));
        Ok(slot)
    }
    fn block(&mut self, block: &ActionBlock) -> Result<(Vec<Statement>, bool), Diagnostic> {
        self.block_scoped(block, true)
    }
    pub(super) fn block_scoped(&mut self, block: &ActionBlock, restore: bool) -> Result<(Vec<Statement>, bool), Diagnostic> {
        let before = self.locals.clone();
        let mut result = Vec::new();
        let mut returns = false;
        for statement in &block.statements {
            if returns { return Err(fail(block.span, "return 后的语句不可达")); }
            result.push(match statement {
                ActionStatement::Let { name, initializer, span, .. } => {
                    let expr = self.expr(initializer, None)?;
                    let slot = self.bind(name, expr.ty.clone(), *span)?;
                    Statement::Local(slot, expr)
                }
                ActionStatement::Assign { name, value, span } => {
                    let (slot, ty) = self.locals.get(name).cloned().ok_or_else(|| fail(*span, "赋值目标必须为当前局部；状态使用 setState"))?;
                    Statement::Local(slot, self.expr(value, Some(&ty))?)
                }
                ActionStatement::Expression { expression, span } => {
                    if let ExpressionKind::Call { callee, arguments } = &expression.kind {
                        if matches!(&callee.kind, ExpressionKind::Identifier(n) if n == "setState") {
                            if self.effect != Effect::Command { return Err(fail(*span, "只有 Command 可以 setState")); }
                            let mut updates = Vec::new();
                            for argument in arguments {
                                let name = argument.name.as_deref().ok_or_else(|| fail(argument.span, "setState 需要命名参数"))?;
                                let (slot, ty) = self.states.get(name).cloned().ok_or_else(|| fail(argument.span, "未知状态字段"))?;
                                if updates.iter().any(|(s, _)| *s == slot) { return Err(fail(argument.span, "重复状态更新")); }
                                updates.push((slot, self.expr(&argument.value, Some(&ty))?));
                            }
                            result.push(Statement::State(updates));
                            continue;
                        }
                    }
                    Statement::Evaluate(self.expr(expression, None)?)
                }
                ActionStatement::If { condition, then_block, else_block, .. } => {
                    let condition = self.expr(condition, Some(&Type::Bool))?;
                    let (yes, yes_returns) = self.block(then_block)?;
                    let (no, no_returns) = match else_block { Some(b) => self.block(b)?, None => (Vec::new(), false) };
                    returns = yes_returns && no_returns;
                    Statement::If(condition, yes, no)
                }
                ActionStatement::Return { value, span } => {
                    returns = true;
                    let expected = self.returns.clone();
                    if value.is_none() && expected != Type::Unit { return Err(fail(*span, "返回值缺失")); }
                    Statement::Return(value.as_ref().map(|v| self.expr(v, Some(&expected))).transpose()?)
                }
            });
        }
        if restore { self.locals = before; }
        Ok((result, returns))
    }
}

fn constant(expr: &Expr) -> Option<Value> {
    match &expr.kind {
        ExprKind::Literal(v) => Some(v.clone()),
        ExprKind::Array(v) => v.iter().map(constant).collect::<Option<Vec<_>>>().map(Value::Array),
        ExprKind::Record(v) => v.iter().map(|(k, v)| constant(v).map(|v| (k.clone(), v))).collect::<Option<BTreeMap<_, _>>>().map(Value::Record),
        ExprKind::Unary { negate, value } => match (*negate, constant(value)?) {
            (false, Value::Bool(v)) => Some(Value::Bool(!v)),
            (true, Value::Int(v)) => v.checked_neg().map(Value::Int),
            (true, Value::Float(v)) => Some(Value::Float(-v)), _ => None,
        },
        ExprKind::Call { name, arguments } if matches!(name.as_str(), "Some" | "None" | "Ok" | "Err") =>
            construct(name, arguments.iter().map(constant).collect::<Option<Vec<_>>>()?).ok(),
        _ => None,
    }
}
