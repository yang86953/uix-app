use super::*;
pub(super) fn optional_number(value: Option<usize>) -> TokenStream {
    match value {
        Some(value) => quote! {Some(#value)},
        None => quote! {None},
    }
}
pub(super) fn export_key(key: &rt::ExportKey) -> TokenStream {
    let package = &key.package;
    let name = &key.name;
    quote! {C::ExportKey::new(#package,#name)}
}
pub(super) fn location(value: &runtime::Location) -> TokenStream {
    let source = &value.source;
    let line = value.line;
    let column = value.column;
    quote! {R::Location{source:#source.into(),line:#line,column:#column}}
}
pub(super) fn signature(value: &rt::FunctionSignature) -> TokenStream {
    let minimum = value.minimum_arguments;
    let parameters = value.parameters.iter().map(ty);
    let returns = ty(&value.returns);
    let effect = format_ident!("{}", format!("{:?}", value.effect));
    quote! {C::FunctionSignature{minimum_arguments:#minimum,parameters:vec![#(#parameters),*],returns:Box::new(#returns),effect:R::Effect::#effect}}
}
pub(super) fn component_signature(value: &rt::ComponentSignature) -> TokenStream {
    let parameters = value.parameters.iter().map(|(name, value)| {
        let value = ty(value);
        quote! {(#name.into(),#value)}
    });
    let required = &value.required;
    quote! {C::ComponentSignature{parameters:vec![#(#parameters),*],required: ::std::collections::BTreeSet::from([#(#required.into()),*])}}
}
pub(super) fn ty(value: &rt::Type) -> TokenStream {
    match value {
        rt::Type::Data(value) => {
            let value = data_type(value);
            quote! {C::Type::Data(#value)}
        }
        rt::Type::View => quote! {C::Type::View},
        rt::Type::Function(value) => {
            let value = signature(value);
            quote! {C::Type::Function(#value)}
        }
        rt::Type::Array(value) => {
            let value = ty(value);
            quote! {C::Type::Array(Box::new(#value))}
        }
        rt::Type::Record(fields) => {
            let fields = fields.iter().map(|(name, value)| {
                let value = ty(value);
                quote! {(#name.into(),#value)}
            });
            quote! {C::Type::Record(::std::collections::BTreeMap::from([#(#fields),*]))}
        }
    }
}
fn data_type(value: &runtime::Type) -> TokenStream {
    use runtime::Type as T;
    match value {
        T::Array(value) => {
            let value = data_type(value);
            quote! {R::Type::Array(Box::new(#value))}
        }
        T::Optional(value) => {
            let value = data_type(value);
            quote! {R::Type::Optional(Box::new(#value))}
        }
        T::Result(ok, err) => {
            let ok = data_type(ok);
            let err = data_type(err);
            quote! {R::Type::Result(Box::new(#ok),Box::new(#err))}
        }
        T::Record(fields) => {
            let fields = fields.iter().map(|(name, value)| {
                let value = data_type(value);
                quote! {(#name.into(),#value)}
            });
            quote! {R::Type::Record(::std::collections::BTreeMap::from([#(#fields),*]))}
        }
        _ => {
            let variant = format_ident!("{}", format!("{value:?}"));
            quote! {R::Type::#variant}
        }
    }
}
pub(super) fn data_value(value: &runtime::Value) -> TokenStream {
    use runtime::Value as V;
    match value {
        V::Unit => quote! {R::Value::Unit},
        V::Bool(value) => quote! {R::Value::Bool(#value)},
        V::Int(value) => quote! {R::Value::Int(#value)},
        V::Float(value) => quote! {R::Value::Float(#value)},
        V::String(value) => quote! {R::Value::String(#value.into())},
        V::Bytes(value) => quote! {R::Value::Bytes(vec![#(#value),*])},
        V::Array(values) => {
            let values = values.iter().map(data_value);
            quote! {R::Value::Array(vec![#(#values),*])}
        }
        V::Record(fields) => {
            let fields = fields.iter().map(|(name, value)| {
                let value = data_value(value);
                quote! {(#name.into(),#value)}
            });
            quote! {R::Value::Record(::std::collections::BTreeMap::from([#(#fields),*]))}
        }
        V::Optional(None) => quote! {R::Value::Optional(None)},
        V::Optional(Some(value)) => {
            let value = data_value(value);
            quote! {R::Value::Optional(Some(Box::new(#value)))}
        }
        V::Result(Ok(value)) => {
            let value = data_value(value);
            quote! {R::Value::Result(Ok(Box::new(#value)))}
        }
        V::Result(Err(value)) => {
            let value = data_value(value);
            quote! {R::Value::Result(Err(Box::new(#value)))}
        }
    }
}
