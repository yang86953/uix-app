//! 新组件特性只适配共同受检身份；不扫描名称文本或伪造原生实现位置。
use super::{document_snapshot, offset_at, request_position, request_uri, source_uri};
use crate::lang::{
    compiler::{
        component_source::{
            ComponentOutput, NodeId,
            symbols::{Symbol, SymbolKind, SymbolTarget},
        },
        source_graph::SourceId,
    },
    lsp::{
        position::LineIndex,
        session::{Session, same_document_path},
    },
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path, sync::Arc};

struct Request {
    output: Arc<ComponentOutput>,
    source: SourceId,
    offset: usize,
    uri: String,
}
fn analysis(session: &mut Session, request: &Value) -> Option<Request> {
    let snapshot = document_snapshot(session, &request_uri(request));
    let output = session.component_analysis(&snapshot.uri, &snapshot.text)?;
    let file = output
        .checked
        .source()
        .source_graph
        .files()
        .iter()
        .find(|file| {
            file.path == snapshot.name
                || snapshot
                    .path
                    .as_ref()
                    .is_some_and(|path| same_document_path(path, Path::new(&file.path)))
        })?;
    // 不能拿另一编辑版本的位置回答当前缓冲区。
    if file.source != snapshot.text {
        return None;
    }
    let source = file.id;
    let (line, character) = request_position(request);
    Some(Request {
        output,
        source,
        offset: offset_at(&snapshot.text, line, character),
        uri: snapshot.uri,
    })
}

struct Locations<'a> {
    files: BTreeMap<SourceId, (&'a str, String, LineIndex)>,
}
impl<'a> Locations<'a> {
    fn new(request: &'a Request) -> Self {
        Self {
            files: request
                .output
                .checked
                .source()
                .source_graph
                .files()
                .iter()
                .map(|file| {
                    (
                        file.id,
                        (
                            file.source.as_str(),
                            source_uri(&file.path, &request.uri),
                            LineIndex::new(&file.source),
                        ),
                    )
                })
                .collect(),
        }
    }
    fn range(&self, at: NodeId) -> Value {
        let (text, _, index) = &self.files[&at.source];
        let (sl, sc) = index.position(text, at.start);
        let (el, ec) = index.position(text, at.end);
        json!({"start":{"line":sl,"character":sc},"end":{"line":el,"character":ec}})
    }
    fn location(&self, at: NodeId) -> Value {
        json!({"uri":self.files[&at.source].1,"range":self.range(at)})
    }
}

pub(super) fn definition(session: &mut Session, request: &Value) -> Value {
    let Some(request) = analysis(session, request) else {
        return Value::Null;
    };
    let index = request.output.checked.symbol_index();
    index
        .at(request.source, request.offset)
        .and_then(|at| at.fact.target.as_ref())
        .and_then(|target| index.definition(target))
        .map_or(Value::Null, |at| Locations::new(&request).location(at))
}
pub(super) fn references(session: &mut Session, params: &Value) -> Value {
    let Some(request) = analysis(session, params) else {
        return json!([]);
    };
    let index = request.output.checked.symbol_index();
    let Some(target) = index
        .at(request.source, request.offset)
        .and_then(|at| at.fact.target.as_ref())
    else {
        return json!([]);
    };
    let include = params
        .pointer("/context/includeDeclaration")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let locations = Locations::new(&request);
    json!(
        index
            .references(target, include)
            .into_iter()
            .map(|at| locations.location(at))
            .collect::<Vec<_>>()
    )
}
pub(super) fn hover(session: &mut Session, params: &Value) -> Value {
    let Some(request) = analysis(session, params) else {
        return Value::Null;
    };
    let index = request.output.checked.symbol_index();
    let Some(at) = index.at(request.source, request.offset) else {
        return Value::Null;
    };
    let Some(text) = index.hover(at) else {
        return Value::Null;
    };
    // plaintext 防止用户定义的名称/别名正文被解释成 Markdown 指令或链接。
    json!({"contents":{"kind":"plaintext","value":text},"range":Locations::new(&request).range(at.range)})
}
pub(super) fn symbols(session: &mut Session, params: &Value) -> Value {
    let Some(request) = analysis(session, params) else {
        return json!([]);
    };
    let index = request.output.checked.symbol_index();
    let locations = Locations::new(&request);
    let mut children = BTreeMap::<&SymbolTarget, Vec<&Symbol>>::new();
    for symbol in index
        .symbols()
        .values()
        .filter(|symbol| symbol.definition.is_some())
    {
        if let Some(parent) = &symbol.parent {
            children.entry(parent).or_default().push(symbol);
        }
    }
    for children in children.values_mut() {
        children.sort_by_key(|symbol| symbol.definition);
    }
    let mut roots = index
        .symbols()
        .values()
        .filter(|symbol| {
            symbol.parent.is_none()
                && symbol
                    .definition
                    .is_some_and(|at| at.source == request.source)
        })
        .collect::<Vec<_>>();
    roots.sort_by_key(|symbol| symbol.definition);
    json!(
        roots
            .into_iter()
            .map(|symbol| document_symbol(&children, &locations, symbol))
            .collect::<Vec<_>>()
    )
}
fn document_symbol(
    children: &BTreeMap<&SymbolTarget, Vec<&Symbol>>,
    locations: &Locations<'_>,
    symbol: &Symbol,
) -> Value {
    json!({"name":symbol.name,"kind":kind(symbol.kind),
        "range":locations.range(symbol.extent.unwrap()),"selectionRange":locations.range(symbol.definition.unwrap()),
        "children":children.get(&symbol.target).into_iter().flatten().map(|child| document_symbol(children, locations, child)).collect::<Vec<_>>()})
}
fn kind(kind: SymbolKind) -> u32 {
    match kind {
        SymbolKind::Component => 5,
        SymbolKind::Function => 12,
        SymbolKind::Type => 26,
        SymbolKind::Input | SymbolKind::Property => 7,
        SymbolKind::State | SymbolKind::Local => 13,
    }
}
