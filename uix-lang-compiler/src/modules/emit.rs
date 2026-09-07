//! 只消费类型化模块的 Rust Emitter；函数与控制流静态生成为 Rust。

use uix_lang_runtime::*;
use proc_macro2::TokenStream;
use quote::{quote, format_ident};

pub(super) fn module(module: &Module) -> TokenStream {
    let mut definitions = Vec::new();
    let mut functions = Vec::new();
    for (index, function) in module.functions.iter().enumerate() {
        let name = format_ident!("__uix_module_function_{index}");
        let statements = match &function.body { Body::Dynamic(body) => block(body), Body::Native(_) => unreachable!("Compiler 只生成动态 IR") };
        definitions.push(quote! {
            #[allow(unreachable_code)]
            fn #name(frame: &mut __uix_module::Frame<'_, '_>) -> __uix_module::RuntimeResult<__uix_module::Value> {
                #statements
                Ok(__uix_module::Value::Unit)
            }
        });
        let signature = signature(&function.signature);
        let exported = function.exported;
        let local_count = function.local_count;
        let location = location(&function.location);
        functions.push(quote! { __uix_module::Function {
            signature: #signature, exported: #exported, local_count: #local_count,
            body: __uix_module::Body::Native(#name), location: #location,
        } });
    }
    let states = module.states.iter().map(|field| {
        let name = &field.name;
        let ty = ty(&field.ty);
        let initial = value(&field.initial);
        quote! { __uix_module::StateField { name: #name.into(), ty: #ty, initial: #initial } }
    });
    let ports = module.ports.iter().map(signature);
    let name = &module.name;
    let version = &module.version;
    let schema = module.state_schema;
    let view = match &module.view { Some(v) => { let v = view(v); quote! { Some(#v) } }, None => quote! { None } };
    quote! {{
        use ::uix::app::modules::__runtime as __uix_module;
        #(#definitions)*
        __uix_module::Module { name: #name.into(), version: #version.into(), state_schema: #schema,
            states: vec![#(#states),*], functions: vec![#(#functions),*], ports: vec![#(#ports),*], view: #view,
            tasks: vec![] }
    }}
}

fn view(v: &ViewTemplate) -> TokenStream {
    let kind = format_ident!("{}", format!("{:?}", v.kind));
    let key = &v.key;
    let properties = v.properties.iter().map(|(k, v)| quote! { (#k.into(), #v.into()) });
    let handler = match &v.handler { Some(v) => quote! { Some(#v.into()) }, None => quote! { None } };
    let children = v.children.iter().map(view);
    quote! { __uix_module::ViewTemplate { kind: __uix_module::ViewKind::#kind, key: #key.into(),
        properties: ::std::collections::BTreeMap::from([#(#properties),*]), handler: #handler, children: vec![#(#children),*] } }
}

fn signature(s: &Signature) -> TokenStream {
    let name = &s.name;
    let parameters = s.parameters.iter().map(|(name, t)| { let t = ty(t); quote! { (#name.into(), #t) } });
    let returns = ty(&s.returns);
    let effect = format_ident!("{}", format!("{:?}", s.effect));
    let asynchronous = s.asynchronous;
    quote! { __uix_module::Signature { name: #name.into(), parameters: vec![#(#parameters),*],
        returns: #returns, effect: __uix_module::Effect::#effect, asynchronous: #asynchronous } }
}

fn ty(t: &Type) -> TokenStream {
    match t {
        Type::Array(t) => { let t = ty(t); quote! { __uix_module::Type::Array(Box::new(#t)) } }
        Type::Optional(t) => { let t = ty(t); quote! { __uix_module::Type::Optional(Box::new(#t)) } }
        Type::Result(a, b) => { let a = ty(a); let b = ty(b); quote! { __uix_module::Type::Result(Box::new(#a), Box::new(#b)) } }
        Type::Record(fields) => {
            let fields = fields.iter().map(|(k, t)| { let t = ty(t); quote! { (#k.into(), #t) } });
            quote! { __uix_module::Type::Record(::std::collections::BTreeMap::from([#(#fields),*])) }
        }
        _ => { let t = format_ident!("{}", format!("{t:?}")); quote! { __uix_module::Type::#t } }
    }
}
fn value(v: &Value) -> TokenStream {
    match v {
        Value::Unit => quote! { __uix_module::Value::Unit },
        Value::Bool(v) => quote! { __uix_module::Value::Bool(#v) },
        Value::Int(v) => quote! { __uix_module::Value::Int(#v) },
        Value::Float(v) => quote! { __uix_module::Value::Float(#v) },
        Value::String(v) => quote! { __uix_module::Value::String(#v.into()) },
        Value::Bytes(v) => quote! { __uix_module::Value::Bytes(vec![#(#v),*]) },
        Value::Array(v) => { let v = v.iter().map(value); quote! { __uix_module::Value::Array(vec![#(#v),*]) } }
        Value::Record(v) => {
            let v = v.iter().map(|(k, v)| { let v = value(v); quote! { (#k.into(), #v) } });
            quote! { __uix_module::Value::Record(::std::collections::BTreeMap::from([#(#v),*])) }
        }
        Value::Optional(None) => quote! { __uix_module::Value::Optional(None) },
        Value::Optional(Some(v)) => { let v = value(v); quote! { __uix_module::Value::Optional(Some(Box::new(#v))) } }
        Value::Result(Ok(v)) => { let v = value(v); quote! { __uix_module::Value::Result(Ok(Box::new(#v))) } }
        Value::Result(Err(v)) => { let v = value(v); quote! { __uix_module::Value::Result(Err(Box::new(#v))) } }
    }
}
fn location(l: &Location) -> TokenStream {
    let source = &l.source;
    let line = l.line;
    let column = l.column;
    quote! { __uix_module::Location { source: #source.into(), line: #line, column: #column } }
}

fn expression(e: &Expr) -> TokenStream {
    let origin = location(&e.location);
    let body = match &e.kind {
        ExprKind::Literal(v) => { let v = value(v); quote! { Ok(#v) } }
        ExprKind::Local(slot) => quote! { frame.local(#slot) },
        ExprKind::State(slot) => quote! { frame.state(#slot) },
        ExprKind::Unary { negate, value } => { let v = expression(value); quote! { { let v = #v?; frame.unary(#negate, v) } } }
        ExprKind::Binary { op, left, right } => {
            let a = expression(left);
            let b = expression(right);
            let code = format_ident!("{}", format!("{op:?}"));
            let operation = quote! { { let right = #b?; __uix_module::binary(__uix_module::Binary::#code, left, right) } };
            let operation = match op {
                Binary::And => quote! { if !left.as_bool()? { Ok(__uix_module::Value::Bool(false)) } else { #operation } },
                Binary::Or => quote! { if left.as_bool()? { Ok(__uix_module::Value::Bool(true)) } else { #operation } },
                _ => operation,
            };
            quote! { { let left = #a?; #operation } }
        }
        ExprKind::Conditional { condition, yes, no } => {
            let c = expression(condition); let a = expression(yes); let b = expression(no);
            quote! { if #c?.as_bool()? { #a } else { #b } }
        }
        ExprKind::Array(items) => { let items = items.iter().map(expression); quote! { Ok(__uix_module::Value::Array(vec![#(#items?),*])) } }
        ExprKind::Record(fields) => {
            let fields = fields.iter().map(|(k, e)| { let e = expression(e); quote! { (#k.into(), #e?) } });
            quote! { Ok(__uix_module::Value::Record(::std::collections::BTreeMap::from([#(#fields),*]))) }
        }
        ExprKind::Member { value, name } => { let v = expression(value); quote! { { let v = #v?; frame.member(v, #name) } } }
        ExprKind::Index { value, index } => { let v = expression(value); let i = expression(index); quote! { { let v = #v?; let i = #i?; frame.index(v, i) } } }
        ExprKind::Call { name, arguments } => {
            let arguments = arguments.iter().map(expression);
            let invoke = if matches!(name.as_str(), "Some" | "None" | "Ok" | "Err") {
                quote! { __uix_module::construct(#name, args) }
            } else { quote! { frame.call(#name, args) } };
            quote! { { let args = vec![#(#arguments?),*]; #invoke } }
        }
        ExprKind::Method { value, name, arguments } => {
            let v = expression(value); let arguments = arguments.iter().map(expression);
            quote! { { let v = #v?; let args = vec![#(#arguments?),*]; frame.method(v, #name, args) } }
        }
        ExprKind::Map { value, slot, body, filter } => {
            let v = expression(value); let body = expression(body);
            quote! { { let v = #v?; frame.map(v, #slot, #filter, |frame| #body) } }
        }
    };
    quote! { (|| -> __uix_module::RuntimeResult<__uix_module::Value> {
        let origin = #origin;
        frame.step(&origin)?;
        let value = (|| -> __uix_module::RuntimeResult<__uix_module::Value> { #body })()
            .map_err(|error| error.at(&origin))?;
        frame.checked(value, &origin)
    })() }
}

fn block(statements: &[Statement]) -> TokenStream {
    let statements = statements.iter().map(|statement| {
        let body = match statement {
            Statement::Local(slot, value) => { let value = expression(value); quote! { let value = #value?; frame.set_local(#slot, value)?; } }
            Statement::State(updates) => {
                let updates = updates.iter().map(|(s, v)| { let v = expression(v); quote! { (#s, #v?) } });
                quote! { let updates = vec![#(#updates),*]; frame.set_state(updates)?; }
            }
            Statement::Evaluate(value) => { let value = expression(value); quote! { #value?; } }
            Statement::If(condition, yes, no) => {
                let condition = expression(condition); let yes = block(yes); let no = block(no);
                quote! { if #condition?.as_bool()? { #yes } else { #no } }
            }
            Statement::Return(Some(v)) => { let v = expression(v); quote! { return #v; } }
            Statement::Return(None) => quote! { return Ok(__uix_module::Value::Unit); },
        };
        quote! { frame.step(&__uix_module::Location::default())?; #body }
    });
    quote! { #(#statements)* }
}
