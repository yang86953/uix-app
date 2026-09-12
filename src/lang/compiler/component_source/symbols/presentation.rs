use super::*;
use std::fmt::{self, Write};

// 大类型/别名按 UTF-8 边界截断展示；身份和查询范围不受展示上限影响。
struct Preview(String, bool);
pub(super) fn preview(arguments: fmt::Arguments<'_>) -> String {
    let mut output = Preview(String::new(), false);
    let _ = output.write_fmt(arguments);
    output.0
}
impl Write for Preview {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.1 {
            return Err(fmt::Error);
        }
        let remaining = 8192usize.saturating_sub(self.0.len());
        if value.len() <= remaining {
            self.0.push_str(value);
            return Ok(());
        }
        let mut end = remaining;
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        self.0.push_str(&value[..end]);
        self.0.push('…');
        self.1 = true;
        Err(fmt::Error)
    }
}

impl SymbolIndex<'_> {
    /// 同一身份的展示说明。类型来自受检签名，别名保留作者的定义文本。
    pub fn describe(&self, target: &SymbolTarget) -> Option<String> {
        let symbol = self.symbols.get(target)?;
        let mut text = Preview(String::new(), false);
        let _ = write!(text, "{} {}", symbol.kind.as_str(), symbol.name);
        let _ = match target {
            SymbolTarget::Binding(id) => write!(text, ": {:?}", self.checked.bindings()[id].ty),
            SymbolTarget::Function(id) => write!(text, ": {:?}", self.checked.functions()[id]),
            SymbolTarget::Declaration(id) => {
                let declaration =
                    &self.checked.source().units[&id.source_id].declarations[id.index];
                match &declaration.kind {
                    DeclarationKind::Component { .. } => {
                        write!(text, ": {:?}", self.checked.components()[id])
                    }
                    DeclarationKind::Type(ty) => {
                        let file = self.checked.source().source_graph.file(id.source_id)?;
                        write!(text, " = {}", &file.source[ty.span.start..ty.span.end])
                    }
                    DeclarationKind::Function(_) => unreachable!(),
                }
            }
            SymbolTarget::Native { package, name } => write!(
                text,
                " from {package}: {:?}",
                self.checked.native_imports()[package][name]
            ),
            SymbolTarget::NativeParameter {
                package,
                name,
                parameter,
            } => {
                let NativeExport::Component(signature) =
                    &self.checked.native_imports()[package][name]
                else {
                    return None;
                };
                let (_, ty) = signature
                    .parameters
                    .iter()
                    .find(|(key, _)| key == parameter)?;
                write!(
                    text,
                    " of {package}/{name}: {:?}{}",
                    ty,
                    if signature.required.contains(parameter) {
                        " (required)"
                    } else {
                        " (optional)"
                    }
                )
            }
        };
        Some(text.0)
    }
    pub fn hover(&self, occurrence: &Occurrence) -> Option<String> {
        if let Some(SymbolTarget::Binding(_)) = &occurrence.fact.target {
            if let Some(expression) = occurrence.fact.expression {
                let fact = self.checked.expressions().get(&expression)?;
                let symbol = self.symbols.get(occurrence.fact.target.as_ref()?)?;
                let mut text = Preview(String::new(), false);
                let _ = write!(
                    text,
                    "{} {}: {:?}; effect: {:?}",
                    symbol.kind.as_str(),
                    symbol.name,
                    fact.ty,
                    fact.effect
                );
                return Some(text.0);
            }
        }
        if let Some(target) = &occurrence.fact.target {
            return self.describe(target);
        }
        let fact = self
            .checked
            .expressions()
            .get(&occurrence.fact.expression?)?;
        let mut text = Preview(String::new(), false);
        let _ = write!(text, "{:?}; effect: {:?}", fact.ty, fact.effect);
        Some(text.0)
    }
}
impl SymbolKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Component => "component",
            Self::Function => "function",
            Self::Type => "type",
            Self::Input => "input",
            Self::State => "state",
            Self::Local => "let",
            Self::Property => "property",
        }
    }
}
