//! 在实际类型/名称检查发生处记录工具事实，不重新模拟作用域。
use super::*;
use crate::lang::compiler::component_source::symbols::{
    NameFact, SymbolTarget, binding_target, resolved_target,
};

impl Checker<'_> {
    pub(super) fn record_type_name(&mut self, source: SourceId, name: &Name, binding: &Binding) {
        self.names.insert(
            NodeId::new(source, name.span),
            NameFact {
                target: Some(binding_target(self.linked, binding)),
                expression: None,
            },
        );
    }
    pub(super) fn record_expression_names(
        &mut self,
        source: SourceId,
        expression: &Expr,
        fact: &Fact,
    ) {
        let at = NodeId::new(source, expression.span);
        let target = fact.resolution.as_ref().and_then(resolved_target);
        match &expression.kind {
            ExprKind::Name(name) | ExprKind::Member { name, .. } => {
                self.names.insert(
                    NodeId::new(source, name.span),
                    NameFact {
                        target,
                        expression: Some(at),
                    },
                );
            }
            ExprKind::Element(element) => {
                for name in element
                    .name
                    .iter()
                    .chain(element.closing_name.iter().flatten())
                {
                    self.names.insert(
                        NodeId::new(source, name.span),
                        NameFact {
                            target: target.clone(),
                            expression: Some(at),
                        },
                    );
                }
                for attribute in &element.attributes {
                    if attribute.name.text == "key" {
                        continue;
                    }
                    let parameter = match target.as_ref() {
                        Some(SymbolTarget::Declaration(id)) => {
                            let DeclarationKind::Component { parameters, .. } =
                                &self.linked.units[&id.source_id].declarations[id.index].kind
                            else {
                                continue;
                            };
                            parameters
                                .iter()
                                .find(|p| p.name.text == attribute.name.text)
                                .map(|p| {
                                    SymbolTarget::Binding(NodeId::new(id.source_id, p.name.span))
                                })
                        }
                        Some(SymbolTarget::Native { package, name }) => {
                            Some(SymbolTarget::NativeParameter {
                                package: package.clone(),
                                name: name.clone(),
                                parameter: attribute.name.text.clone(),
                            })
                        }
                        _ => None,
                    };
                    self.names.insert(
                        NodeId::new(source, attribute.name.span),
                        NameFact {
                            target: parameter,
                            expression: None,
                        },
                    );
                }
            }
            _ => {}
        }
    }
}
