use super::*;

pub use crate::lang::runtime::components::{
    ComponentSignature, FunctionSignature, NativeExport, NativeLibraries, NativeLibrary, Type,
};

impl Checker<'_> {
    pub(super) fn native_interfaces(&mut self) -> Result<()> {
        let linked = self.linked;
        let libraries = self.libraries;
        for unit in linked.units.values() {
            for import in &unit.imports {
                for name in &import.names {
                    let Some(Binding::NativeImport {
                        package,
                        name: exported,
                    }) = unit.bindings.get(&name.local.text)
                    else {
                        continue;
                    };
                    if self
                        .native_imports
                        .get(package)
                        .is_some_and(|library| library.contains_key(exported))
                    {
                        continue;
                    }
                    let export = libraries
                        .get(package)
                        .and_then(|library| library.get(exported))
                        .ok_or_else(|| {
                            self.error(
                                unit.source_id,
                                name.imported.span,
                                "component-native-export",
                                format!("原生库 {package} 未提供受检导出 {exported}"),
                            )
                        })?;
                    let cost = native_export_cost(export).map_err(|(code, message)| {
                        self.error(unit.source_id, name.imported.span, code, message)
                    })?;
                    self.charge(unit.source_id, name.imported.span, cost)?;
                    self.native_imports
                        .entry(package.clone())
                        .or_default()
                        .insert(exported.clone(), canonical_export(export));
                }
            }
        }
        Ok(())
    }

    pub(super) fn ty(&mut self, source: SourceId, node: &TypeNode, depth: usize) -> Result<Type> {
        if depth > 64 {
            return Err(self.error(
                source,
                node.span,
                "component-type-limit",
                "类型展开超过 64 层",
            ));
        }
        let ty = match &node.kind {
            TypeKind::Array(ty) => Type::array(self.ty(source, ty, depth + 1)?),
            TypeKind::Record(fields) => {
                let mut result = BTreeMap::new();
                for (name, ty) in fields {
                    let ty = self.ty(source, ty, depth + 1)?;
                    if result.insert(name.text.clone(), ty).is_some() {
                        return Err(self.error(
                            source,
                            name.span,
                            "component-type-field",
                            "重复类型字段",
                        ));
                    }
                }
                Type::record(result)
            }
            TypeKind::Function {
                parameters,
                returns,
            } => {
                let mut names = BTreeSet::new();
                let mut types = Vec::new();
                for (name, ty) in parameters {
                    if !names.insert(&name.text) {
                        return Err(self.error(
                            source,
                            name.span,
                            "component-parameter",
                            "重复参数名称",
                        ));
                    }
                    types.push(self.ty(source, ty, depth + 1)?);
                }
                Type::Function(FunctionSignature {
                    minimum_arguments: types.len(),
                    parameters: types,
                    returns: Box::new(self.ty(source, returns, depth + 1)?),
                    effect: Effect::Command,
                })
            }
            TypeKind::Named { path, arguments } => {
                if path.len() != 1 {
                    return Err(self.error(
                        source,
                        node.span,
                        "component-type-name",
                        "使用显式导入的类型名称，不推断 Rust 路径",
                    ));
                }
                let name = &path[0].text;
                match (name.as_str(), arguments.as_slice()) {
                    ("Unit", []) => Type::Data(DataType::Unit),
                    ("Bool", []) => Type::Data(DataType::Bool),
                    ("Int", []) => Type::Data(DataType::Int),
                    ("Float", []) => Type::Data(DataType::Float),
                    ("String", []) => Type::Data(DataType::String),
                    ("Bytes", []) => Type::Data(DataType::Bytes),
                    ("View", []) => Type::View,
                    ("Array", [element]) => Type::array(self.ty(source, element, depth + 1)?),
                    ("Option", [element]) => {
                        let Type::Data(ty) = self.ty(source, element, depth + 1)? else {
                            return Err(self.error(
                                source,
                                node.span,
                                "component-data-type",
                                "Option 当前要求拥有型数据",
                            ));
                        };
                        Type::Data(DataType::Optional(Box::new(ty)))
                    }
                    ("Result", [ok, error]) => {
                        let (Type::Data(ok), Type::Data(error)) = (
                            self.ty(source, ok, depth + 1)?,
                            self.ty(source, error, depth + 1)?,
                        ) else {
                            return Err(self.error(
                                source,
                                node.span,
                                "component-data-type",
                                "Result 当前要求拥有型数据",
                            ));
                        };
                        Type::Data(DataType::Result(Box::new(ok), Box::new(error)))
                    }
                    (_, []) => {
                        let binding = self.linked.units[&source]
                            .bindings
                            .get(name)
                            .cloned()
                            .ok_or_else(|| {
                                self.error(
                                    source,
                                    node.span,
                                    "component-type-name",
                                    format!("未知类型 {name}"),
                                )
                            })?;
                        match binding {
                            Binding::Declaration(id) => self.alias(id, depth + 1)?,
                            Binding::NativeImport { package, name } => {
                                match self.native(&package, &name, source, node.span)? {
                                    NativeExport::Type(ty) => ty.clone(),
                                    _ => {
                                        return Err(self.error(
                                            source,
                                            node.span,
                                            "component-type-name",
                                            "该导出不是类型",
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    _ => {
                        return Err(self.error(
                            source,
                            node.span,
                            "component-type-arguments",
                            "类型名称或泛型参数数量不符",
                        ));
                    }
                }
            }
        };
        self.charge_type(source, node.span, &ty)?;
        Ok(ty)
    }

    fn alias(&mut self, id: DeclarationId, depth: usize) -> Result<Type> {
        if let Some(ty) = self.aliases.get(&id) {
            return Ok(ty.clone());
        }
        let declaration = &self.linked.units[&id.source_id].declarations[id.index];
        let DeclarationKind::Type(node) = &declaration.kind else {
            return Err(self.error(
                id.source_id,
                declaration.span,
                "component-type-name",
                "该声明不是类型",
            ));
        };
        if !self.visiting_types.insert(id) {
            return Err(self.error(
                id.source_id,
                node.span,
                "component-type-cycle",
                "类型别名出现递归循环",
            ));
        }
        let ty = self.ty(id.source_id, node, depth + 1)?;
        self.visiting_types.remove(&id);
        self.aliases.insert(id, ty.clone());
        Ok(ty)
    }
}

fn canonical_type(ty: &Type) -> Type {
    match ty {
        Type::Array(element) => Type::array(canonical_type(element)),
        Type::Record(fields) => Type::record(
            fields
                .iter()
                .map(|(name, ty)| (name.clone(), canonical_type(ty)))
                .collect(),
        ),
        Type::Function(signature) => Type::Function(canonical_signature(signature)),
        ty => ty.clone(),
    }
}
fn canonical_signature(signature: &FunctionSignature) -> FunctionSignature {
    FunctionSignature {
        minimum_arguments: signature.minimum_arguments,
        parameters: signature.parameters.iter().map(canonical_type).collect(),
        returns: Box::new(canonical_type(&signature.returns)),
        effect: signature.effect,
    }
}
fn canonical_export(export: &NativeExport) -> NativeExport {
    match export {
        NativeExport::Component(signature) => NativeExport::Component(ComponentSignature {
            parameters: signature
                .parameters
                .iter()
                .map(|(name, ty)| (name.clone(), canonical_type(ty)))
                .collect(),
            required: signature.required.clone(),
        }),
        NativeExport::Function(signature) => NativeExport::Function(canonical_signature(signature)),
        NativeExport::Type(ty) => NativeExport::Type(canonical_type(ty)),
    }
}

// 原生接口文件和源码检查共用同一形状/类型预算，不维护第二套签名规则。
pub(crate) fn native_export_cost(
    export: &NativeExport,
) -> std::result::Result<usize, (&'static str, &'static str)> {
    let signature_error = |message| ("component-native-signature", message);
    let mut types = Vec::new();
    match export {
        NativeExport::Component(signature) => {
            let mut fields = BTreeSet::new();
            for (field, ty) in &signature.parameters {
                if field == "key" || !fields.insert(field.as_str()) {
                    return Err(signature_error("原生组件不能重复参数或声明业务 key"));
                }
                types.push(ty);
            }
            if signature
                .required
                .iter()
                .any(|name| !fields.contains(name.as_str()))
            {
                return Err(signature_error("原生组件的 required 引用了未声明参数"));
            }
        }
        NativeExport::Function(signature) => {
            if signature.minimum_arguments > signature.parameters.len() {
                return Err(signature_error("原生函数最小参数数目无效"));
            }
            types.extend(&signature.parameters);
            types.push(&signature.returns);
        }
        NativeExport::Type(ty) => types.push(ty),
    }
    let mut total = 0_usize;
    for ty in types {
        let cost =
            type_cost(ty).ok_or(("component-type-limit", "展开后的类型超过 4096 节点或 64 层"))?;
        total = total.saturating_add(cost);
    }
    Ok(total)
}

impl Checker<'_> {
    pub(super) fn charge_type(&mut self, source: SourceId, span: Span, ty: &Type) -> Result<()> {
        let cost = type_cost(ty).ok_or_else(|| {
            self.error(
                source,
                span,
                "component-type-limit",
                "展开后的类型超过 4096 节点或 64 层",
            )
        })?;
        self.charge(source, span, cost)
    }
}

// 有界检查发生在类型保存/复制到更多节点前，防止很短的别名源产生指数型拥有树。
fn type_cost(ty: &Type) -> Option<usize> {
    enum Item<'a> {
        Component(&'a Type),
        Data(&'a DataType),
    }
    let mut pending = vec![(Item::Component(ty), 0usize)];
    let mut count = 0;
    while let Some((item, depth)) = pending.pop() {
        count += 1;
        if count > 4096 || depth > 64 {
            return None;
        }
        let next = depth + 1;
        match item {
            Item::Component(Type::Data(ty)) => pending.push((Item::Data(ty), depth)),
            Item::Component(Type::Array(ty)) => pending.push((Item::Component(ty), next)),
            Item::Component(Type::Record(fields)) => {
                pending.extend(fields.values().map(|ty| (Item::Component(ty), next)))
            }
            Item::Component(Type::Function(signature)) => {
                if signature.minimum_arguments > signature.parameters.len() {
                    return None;
                }
                pending.extend(
                    signature
                        .parameters
                        .iter()
                        .map(|ty| (Item::Component(ty), next)),
                );
                pending.push((Item::Component(&signature.returns), next));
            }
            Item::Data(DataType::Array(ty) | DataType::Optional(ty)) => {
                pending.push((Item::Data(ty), next))
            }
            Item::Data(DataType::Record(fields)) => {
                pending.extend(fields.values().map(|ty| (Item::Data(ty), next)))
            }
            Item::Data(DataType::Result(ok, error)) => {
                pending.push((Item::Data(ok), next));
                pending.push((Item::Data(error), next));
            }
            _ => {}
        }
        if pending.len() > 4096 {
            return None;
        }
    }
    Some(count)
}
