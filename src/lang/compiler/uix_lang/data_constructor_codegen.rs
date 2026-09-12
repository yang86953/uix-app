//! Generic data constructor and method mappings from library descriptors.
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use super::expression_codegen::generate_expression_inner;
use super::{CallArgument, Diagnostic, Expression, SourceSpan, data_constructor_spec, normalize_number_literals};
use crate::lang::compiler::components::CallDeclaration;

pub(super) fn generate_data_constructor(name: &str, arguments: &[CallArgument], span: SourceSpan, event: Option<&Ident>) -> Option<Result<TokenStream, Diagnostic>> {
    if let Some(declaration) = crate::lang::compiler::components::data_declaration(name) {
        crate::lang::compiler::components::use_unit(&declaration.unit);
        if let Some(call) = declaration.call { return Some(generate_call(&call, arguments, span, event, None)); }
    }
    let spec = data_constructor_spec(name)?;
    Some((|| {
        if arguments.iter().any(|argument| argument.name.is_some()) { return Err(Diagnostic::new(span, "data constructors take positional arguments", "Remove argument names")); }
        let arguments = arguments.iter().map(|argument| {
            let mut value = argument.value.clone();
            if spec.normalize_numbers { normalize_number_literals(&mut value); }
            generate_expression_inner(&value, event)
        }).collect::<Result<Vec<_>, _>>()?;
        let path = spec.path;
        let method = spec.method.unwrap_or_else(|| Ident::new("new", proc_macro2::Span::mixed_site()));
        Ok(quote! { #path::#method(#(#arguments),*) })
    })())
}

pub(super) fn generate_data_method(object: &Expression, member: &str, arguments: &[CallArgument], span: SourceSpan, event: Option<&Ident>) -> Option<Result<TokenStream, Diagnostic>> {
    let owner = super::data_chain_root(object)?;
    let declaration = crate::lang::compiler::components::data_declaration(owner)?;
    let call = declaration.methods.get(member)?;
    crate::lang::compiler::components::use_unit(&declaration.unit);
    Some((|| {
        let receiver = generate_expression_inner(object, event)?;
        generate_call(call, arguments, span, event, Some(receiver))
    })())
}

fn generate_call(call: &CallDeclaration, arguments: &[CallArgument], span: SourceSpan, event: Option<&Ident>, receiver: Option<TokenStream>) -> Result<TokenStream, Diagnostic> {
    if arguments.len() != call.parameters.len() || arguments.iter().any(|argument| argument.name.is_some()) {
        return Err(Diagnostic::new(span, format!("expected {} positional arguments", call.parameters.len()), "Follow the library's data declaration"));
    }
    let mut inputs = std::collections::BTreeMap::new();
    if let Some(receiver) = receiver { inputs.insert("receiver".into(), receiver); }
    for (index, (argument, parameter)) in arguments.iter().zip(&call.parameters).enumerate() {
        inputs.insert(format!("arg{index}"), super::component_codegen::expression_value(&argument.value, parameter, event)?);
    }
    super::component_codegen::template(&call.rust, &inputs, span)
}
