//! CLI 源码查询直接投影共享符号索引，不经过旧组件登记目录。
use serde_json::json;
use std::path::Path;
use uix_app::lang::compiler::CompilerSystem;

pub(super) fn run(args: &[String]) -> Result<u8, String> {
    let mut source = None;
    let mut json_output = false;
    for argument in args {
        match argument.as_str() {
            "--json" => json_output = true,
            value if !value.starts_with('-') && source.is_none() => source = Some(value),
            _ => return Err(format!("未知或重复的源码 query 选项: {argument}")),
        }
    }
    let source = source.ok_or("用法: uix query source-symbols <入口.uix> [--json]")?;
    let output = CompilerSystem::new()
        .check_component_file(Path::new(source))
        .map_err(|error| format!("{error:?}"))?;
    let index = output.checked.symbol_index();
    let graph = &output.checked.source().source_graph;
    let mut symbols = index
        .symbols()
        .values()
        .filter(|symbol| symbol.definition.is_some())
        .collect::<Vec<_>>();
    symbols.sort_by_key(|symbol| symbol.definition);
    let entries = symbols
        .into_iter()
        .map(|symbol| {
            let at = symbol.definition.unwrap();
            let file = graph.file(at.source).unwrap();
            json!({"name":symbol.name,"kind":symbol.kind.as_str(),"path":file.path,
            "sourceId":at.source.value(),"start":at.start,"end":at.end,"exported":symbol.exported,
            "parent":symbol.parent.as_ref().and_then(|parent| index.definition(parent)).map(|parent|
                json!({"sourceId":parent.source.value(),"start":parent.start,"end":parent.end}))})
        })
        .collect::<Vec<_>>();
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&entries).map_err(|error| error.to_string())?
        );
    } else {
        for entry in entries {
            println!("{entry}");
        }
    }
    Ok(0)
}
