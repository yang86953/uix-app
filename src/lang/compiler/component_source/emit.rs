//! 同一受检组件的原生 Rust 后端。输出函数体，不在目标中嵌入源码或执行 IR。
use super::*;
use crate::lang::runtime::{self, components as rt};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
mod values;
use values::*;

/// 返回构造 Program 的 Rust 表达式；目标依赖只需 uix-components。
pub fn emit_native(
    checked: &CheckedSource,
    entry_export: &str,
) -> Result<TokenStream, CompilerDiagnostic> {
    let program = lower(checked, entry_export)?;
    Ok(Emitter::default().program(&program))
}
#[derive(Default)]
struct Emitter {
    definitions: Vec<TokenStream>,
}
impl Emitter {
    fn program(&mut self, program: &rt::Program) -> TokenStream {
        let functions=program.functions.iter().map(|function|{
            let signature=signature(&function.signature);let parameters=&function.parameters;let captures=&function.captures;let component=optional_number(function.component);let location=location(&function.location);
            let defaults=function.defaults.iter().map(|body|self.optional_body(body)).collect::<Vec<_>>();let body=self.body(&function.body);
            quote!{C::Function{signature:#signature,parameters:vec![#(#parameters),*],defaults:vec![#(#defaults),*],captures:vec![#(#captures),*],component:#component,body:#body,location:#location}}
        }).collect::<Vec<_>>();
        let components=program.components.iter().map(|component|{
            let name=&component.name;let signature=component_signature(&component.signature);let inputs=&component.inputs;let render=component.render;let location=location(&component.location);
            let defaults=component.defaults.iter().map(|body|self.optional_body(body)).collect::<Vec<_>>();let states=component.states.iter().map(|(binding,body)|{let body=self.body(body);quote!{(#binding,#body)}}).collect::<Vec<_>>();
            quote!{C::Component{name:#name.into(),signature:#signature,inputs:vec![#(#inputs),*],defaults:vec![#(#defaults),*],states:vec![#(#states),*],render:#render,location:#location}}
        }).collect::<Vec<_>>();
        let bindings = program.bindings.iter().map(|binding| {
            let name = &binding.name;
            let ty = ty(&binding.ty);
            let owner = match binding.owner {
                rt::Owner::Component(id) => quote! {C::Owner::Component(#id)},
                rt::Owner::Function(id) => quote! {C::Owner::Function(#id)},
            };
            let kind = match binding.kind {
                rt::BindingKind::Input => quote! {C::BindingKind::Input},
                rt::BindingKind::State => quote! {C::BindingKind::State},
                rt::BindingKind::Parameter => quote! {C::BindingKind::Parameter},
                rt::BindingKind::Local => quote! {C::BindingKind::Local},
                rt::BindingKind::Function(id) => quote! {C::BindingKind::Function(#id)},
            };
            quote! {C::Binding{name:#name.into(),ty:#ty,owner:#owner,kind:#kind}}
        });
        let natives = program.natives.iter().map(|(key, export)| {
            let key = export_key(key);
            let export = match export {
                rt::NativeExport::Component(value) => {
                    let value = component_signature(value);
                    quote! {C::NativeExport::Component(#value)}
                }
                rt::NativeExport::Function(value) => {
                    let value = signature(value);
                    quote! {C::NativeExport::Function(#value)}
                }
                rt::NativeExport::Type(value) => {
                    let value = ty(value);
                    quote! {C::NativeExport::Type(#value)}
                }
            };
            quote! {(#key,#export)}
        });
        let entry = program.entry;
        let definitions = &self.definitions;
        quote! {{use ::uix_app::lang::runtime as R;use R::components as C;#(#definitions)* C::Program{functions:vec![#(#functions),*],components:vec![#(#components),*],bindings:vec![#(#bindings),*],natives: ::std::collections::BTreeMap::from([#(#natives),*]),entry:#entry}}}
    }
    fn optional_body(&mut self, body: &Option<rt::Body>) -> TokenStream {
        match body {
            Some(body) => {
                let body = self.body(body);
                quote! {Some(#body)}
            }
            None => quote! {None},
        }
    }
    fn body(&mut self, body: &rt::Body) -> TokenStream {
        let rt::Body::Dynamic(statements) = body else {
            unreachable!("共同降低只生成构建 IR")
        };
        let name = format_ident!("__uix_component_body_{}", self.definitions.len());
        let block = block(statements);
        self.definitions.push(quote!{fn #name(frame:&mut C::Frame<'_,'_>)->R::RuntimeResult<C::Value>{let value=#block?;Ok(value.unwrap_or_else(C::Value::unit))}});
        quote! {C::Body::Native(#name)}
    }
}
fn block(statements: &[rt::ir::Statement]) -> TokenStream {
    let statements=statements.iter().map(|statement|{
        let location=location(&statement.location);let kind=match &statement.kind {
            rt::ir::StatementKind::Let(id,value)=>{let value=expr(value);quote!{let value=#value?;frame.local(#id,value)?;Ok(None)}},
            rt::ir::StatementKind::Set(id,value)=>{let value=expr(value);quote!{let value=#value?;frame.set(#id,value)?;Ok(None)}},
            rt::ir::StatementKind::Evaluate(value)=>{let value=expr(value);quote!{#value?;Ok(None)}},
            rt::ir::StatementKind::Return(value)=>{let value=value.as_ref().map(expr).unwrap_or_else(||quote!{Ok::<_,R::RuntimeError>(C::Value::unit())});quote!{Ok(Some(#value?))}},
            rt::ir::StatementKind::If(condition,yes,no)=>{let condition=expr(condition);let yes=block(yes);let no=block(no);quote!{let condition=#condition?;if frame.boolean(&condition)?{#yes}else{#no}}},
        };quote!{let returned=frame.scope(&#location,|frame|->R::RuntimeResult<Option<C::Value>>{#kind})?;if returned.is_some(){return Ok(returned);}}
    });
    quote! {{(||->R::RuntimeResult<Option<C::Value>>{#(#statements)* Ok(None)})()}}
}
fn expr(expression: &rt::ir::Expr) -> TokenStream {
    use rt::ir::ExprKind as K;
    let location = location(&expression.location);
    let kind = match &expression.kind {
        K::Constant(value) => {
            let value = data_value(value);
            quote! {Ok::<_,R::RuntimeError>(C::Value::data(#value))}
        }
        K::Get(id) => quote! {frame.get(#id)},
        K::Closure(id) => quote! {frame.closure(#id)},
        K::NativeFunction(key) => {
            let key = export_key(key);
            quote! {frame.native_function(#key)}
        }
        K::Unary { negate, value } => {
            let value = expr(value);
            quote! {{let value=#value?;frame.unary(#negate,value)}}
        }
        K::Binary { op, left, right } => {
            let left = expr(left);
            let right = expr(right);
            let operator = format_ident!("{}", format!("{op:?}"));
            let short = match op {
                runtime::Binary::And => quote! {!frame.boolean(&left)?},
                runtime::Binary::Or => quote! {frame.boolean(&left)?},
                _ => quote! {false},
            };
            quote! {{let left=#left?;if #short{Ok(left)}else{let right=#right?;frame.binary(R::Binary::#operator,left,right)}}}
        }
        K::Conditional { condition, yes, no } => {
            let condition = expr(condition);
            let yes = expr(yes);
            let no = expr(no);
            quote! {{let condition=#condition?;if frame.boolean(&condition)?{#yes}else{#no}}}
        }
        K::Array { values, rich } => {
            let values = values.iter().map(expr);
            quote! {{let values=vec![#(#values?),*];frame.array_typed(values,#rich)}}
        }
        K::Fragment(values) | K::Concat(values) => {
            let method = match &expression.kind {
                K::Fragment(_) => format_ident!("fragment"),
                _ => format_ident!("concat"),
            };
            let values = values.iter().map(expr);
            quote! {{let values=vec![#(#values?),*];frame.#method(values)}}
        }
        K::Record(fields) => {
            let fields = fields.iter().map(|(name, value)| {
                let value = expr(value);
                quote! {(#name.into(),#value?)}
            });
            quote! {{let fields=vec![#(#fields),*];frame.record(fields)}}
        }
        K::Member { value, name } => {
            let value = expr(value);
            quote! {{let value=#value?;frame.member(value,#name)}}
        }
        K::Index { value, index } => {
            let value = expr(value);
            let index = expr(index);
            quote! {{let value=#value?;let index=#index?;frame.index(value,index)}}
        }
        K::Call { callee, arguments } => {
            let callee = expr(callee);
            let arguments = arguments.iter().map(expr);
            quote! {{let callee=#callee?;let arguments=vec![#(#arguments?),*];frame.call(callee,arguments)}}
        }
        K::Method {
            value,
            name,
            arguments,
        } => {
            let value = expr(value);
            let arguments = arguments.iter().map(expr);
            quote! {{let value=#value?;let arguments=vec![#(#arguments?),*];frame.method(value,#name,arguments)}}
        }
        K::Map {
            value,
            callback,
            filter,
        } => {
            let value = expr(value);
            let callback = expr(callback);
            quote! {{let value=#value?;let callback=#callback?;frame.map(value,callback,#filter)}}
        }
        K::Element {
            site,
            target,
            attributes,
        } => {
            let target = target_code(target);
            let key = if attributes
                .iter()
                .any(|attribute| matches!(attribute, rt::ir::Attribute::Key(_)))
            {
                quote! {let key;}
            } else {
                quote! {let key=None;}
            };
            let attributes = attributes.iter().map(|attribute| match attribute {
                rt::ir::Attribute::Key(value) => {
                    let value = expr(value);
                    quote! {key=Some(#value?);}
                }
                rt::ir::Attribute::Property(name, value) => {
                    let value = expr(value);
                    quote! {properties.push((#name.into(),#value?));}
                }
            });
            quote! {{#key #[allow(unused_mut)]let mut properties=Vec::new();#(#attributes)* frame.element(&#location,#site,#target,key,properties)}}
        }
    };
    quote! {frame.scope(&#location,|frame|->R::RuntimeResult<C::Value>{let value=(#kind)?;frame.checked(value)})}
}
fn target_code(target: &rt::Target) -> TokenStream {
    match target {
        rt::Target::Component(id) => quote! {C::Target::Component(#id)},
        rt::Target::Native(key) => {
            let key = export_key(key);
            quote! {C::Target::Native(#key)}
        }
    }
}
