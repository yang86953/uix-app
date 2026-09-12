use super::*;

impl<'a> SymbolIndex<'a> {
    pub(super) fn new(checked: &'a CheckedSource) -> Self {
        let mut index = Self {
            checked,
            symbols: BTreeMap::new(),
            occurrences: BTreeMap::new(),
        };
        let mut owners = BTreeMap::new();
        for unit in checked.source().units.values() {
            for (number, declaration) in unit.declarations.iter().enumerate() {
                let id = DeclarationId {
                    source_id: unit.source_id,
                    index: number,
                };
                let target = binding_target(checked.source(), &Binding::Declaration(id));
                let extent = node(unit.source_id, declaration.span);
                owners.insert(extent, target.clone());
                let kind = match declaration.kind {
                    DeclarationKind::Component { .. } => SymbolKind::Component,
                    DeclarationKind::Function(_) => SymbolKind::Function,
                    DeclarationKind::Type(_) => SymbolKind::Type,
                };
                index.add(Symbol {
                    target: target.clone(),
                    name: declaration.name.text.clone(),
                    kind,
                    definition: Some(node(unit.source_id, declaration.name.span)),
                    extent: Some(extent),
                    parent: None,
                    exported: declaration.exported,
                });
                if let DeclarationKind::Component { body, .. } = &declaration.kind {
                    for member in body {
                        if let ComponentMember::Function { name, span, .. } = member {
                            let extent = node(unit.source_id, *span);
                            let callable = SymbolTarget::Function(extent);
                            owners.insert(extent, callable.clone());
                            index.add(Symbol {
                                target: callable,
                                name: name.text.clone(),
                                kind: SymbolKind::Function,
                                definition: Some(node(unit.source_id, name.span)),
                                extent: Some(extent),
                                parent: Some(target.clone()),
                                exported: false,
                            });
                        }
                    }
                }
            }
        }
        for (id, binding) in checked.bindings() {
            if binding.kind == BindingKind::Function {
                continue;
            }
            index.add(Symbol {
                target: SymbolTarget::Binding(*id),
                name: binding.name.clone(),
                kind: match binding.kind {
                    BindingKind::Input => SymbolKind::Input,
                    BindingKind::State => SymbolKind::State,
                    BindingKind::Local => SymbolKind::Local,
                    BindingKind::Function => unreachable!(),
                },
                definition: Some(*id),
                extent: Some(*id),
                exported: false,
                parent: Some(
                    owners
                        .get(&binding.owner)
                        .cloned()
                        .unwrap_or(SymbolTarget::Function(binding.owner)),
                ),
            });
        }
        for (package, library) in checked.native_imports() {
            for (name, export) in library {
                let target = SymbolTarget::Native {
                    package: package.clone(),
                    name: name.clone(),
                };
                let kind = match export {
                    NativeExport::Component(_) => SymbolKind::Component,
                    NativeExport::Function(_) => SymbolKind::Function,
                    NativeExport::Type(_) => SymbolKind::Type,
                };
                index.add(Symbol {
                    target: target.clone(),
                    name: name.clone(),
                    kind,
                    definition: None,
                    extent: None,
                    parent: None,
                    exported: true,
                });
                if let NativeExport::Component(signature) = export {
                    for (parameter, _) in &signature.parameters {
                        index.add(Symbol {
                            target: SymbolTarget::NativeParameter {
                                package: package.clone(),
                                name: name.clone(),
                                parameter: parameter.clone(),
                            },
                            name: parameter.clone(),
                            kind: SymbolKind::Property,
                            definition: None,
                            extent: None,
                            parent: Some(target.clone()),
                            exported: false,
                        });
                    }
                }
            }
        }
        for (range, fact) in checked.names() {
            index.occurrences.insert(
                *range,
                Occurrence {
                    range: *range,
                    kind: OccurrenceKind::Reference,
                    fact: fact.clone(),
                },
            );
        }
        for unit in checked.source().units.values() {
            for import in &unit.imports {
                for name in &import.names {
                    let target = binding_target(checked.source(), &unit.bindings[&name.local.text]);
                    for name in [&name.imported, &name.local] {
                        let range = node(unit.source_id, name.span);
                        index.occurrences.insert(
                            range,
                            Occurrence {
                                range,
                                kind: OccurrenceKind::Import,
                                fact: NameFact {
                                    target: Some(target.clone()),
                                    expression: None,
                                },
                            },
                        );
                    }
                }
            }
        }
        index
    }
    fn add(&mut self, symbol: Symbol) {
        if let Some(range) = symbol.definition {
            self.occurrences.insert(
                range,
                Occurrence {
                    range,
                    kind: OccurrenceKind::Declaration,
                    fact: NameFact {
                        target: Some(symbol.target.clone()),
                        expression: None,
                    },
                },
            );
        }
        self.symbols.insert(symbol.target.clone(), symbol);
    }
}
