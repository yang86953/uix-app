use super::*;
use crate::lang::compiler::component_source::{completion::ScopeChange, symbols::SymbolTarget};

impl Checker<'_> {
    pub(super) fn observe_scope(&mut self, scope: &mut Scope, span: Span) -> Result<()> {
        if !self.record_scopes {
            return Ok(());
        }
        self.charge(
            scope.source,
            span,
            scope.vars.len() + scope.unavailable.len() + 1,
        )?;
        let initial = scope
            .vars
            .iter()
            .map(|(name, variable)| (name.clone(), Some(target(variable))))
            .collect::<BTreeMap<_, _>>();
        let changes = scope
            .unavailable
            .iter()
            .map(|name| ScopeChange {
                at: span.start,
                name: name.clone(),
                target: None,
            })
            .collect();
        scope.trace = self.scopes.len();
        scope.visible_from = span.start;
        self.scopes.push(LexicalScope {
            range: NodeId::new(scope.source, span),
            initial,
            changes,
            finished: false,
            checked_until: span.start,
        });
        Ok(())
    }
    pub(super) fn finish_scope(&mut self, scope: &Scope) {
        if self.record_scopes {
            self.scopes[scope.trace].finished = true;
        }
    }
    pub(super) fn observe_progress(&mut self, scope: &Scope, end: usize) {
        if self.record_scopes {
            let trace = &mut self.scopes[scope.trace];
            trace.checked_until = trace.checked_until.max(end);
        }
    }
    pub(super) fn observe_hidden_parameters(
        &mut self,
        scope: &Scope,
        parameters: &[Parameter],
    ) -> Result<()> {
        if !self.record_scopes {
            return Ok(());
        }
        self.charge(
            scope.source,
            Span {
                start: scope.owner.start,
                end: scope.owner.end,
            },
            parameters.len(),
        )?;
        let trace = &mut self.scopes[scope.trace];
        for parameter in parameters {
            trace.changes.push(ScopeChange {
                at: trace.range.start,
                name: parameter.name.text.clone(),
                target: None,
            });
        }
        Ok(())
    }
    pub(super) fn observe_binding(&mut self, scope: &Scope, name: &Name) -> Result<()> {
        if !self.record_scopes {
            return Ok(());
        }
        self.charge(scope.source, name.span, 1)?;
        self.scopes[scope.trace].changes.push(ScopeChange {
            at: scope.visible_from,
            name: name.text.clone(),
            target: Some(target(&scope.vars[&name.text])),
        });
        Ok(())
    }
}
fn target(variable: &Variable) -> SymbolTarget {
    match variable.kind {
        VariableKind::Function(id) => SymbolTarget::Function(id),
        _ => SymbolTarget::Binding(variable.id),
    }
}

/// 同一前端允许工具读取失败点之前的事实；此类型不能传给 lower/emit_native。
pub(crate) fn inspect(
    linked: LinkedSource,
    libraries: &NativeLibraries,
) -> super::super::completion::Inspection {
    let mut checker = super::checker(&linked, libraries, true);
    let diagnostic = super::analyze(&mut checker).err().map(|error| *error);
    let checked_diagnostic = diagnostic.clone();
    let diagnostic = if diagnostic
        .as_ref()
        .is_some_and(|error| error.code.ends_with("-limit"))
    {
        diagnostic
    } else {
        linked
            .editor
            .as_ref()
            .and_then(|editor| editor.diagnostics.first())
            .cloned()
            .or(diagnostic)
    };
    let Checker {
        scopes,
        bindings,
        expressions,
        components,
        functions,
        native_imports,
        ..
    } = checker;
    super::super::completion::Inspection {
        linked,
        scopes,
        bindings,
        types: expressions
            .into_iter()
            .map(|(id, fact)| (id, fact.ty))
            .collect(),
        components,
        functions,
        native_imports,
        diagnostic,
        checked_diagnostic,
    }
}
