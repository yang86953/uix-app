//! UIX Lang 工具链命令行 Adapter；语言规则全部由共享 compiler crate 持有。

use serde_json::json;
use std::{env, fs, path::PathBuf, process::ExitCode};
use uix_lang_compiler::{CompileTarget, CompilerDiagnostic, CompilerSystem};

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn run(args: Vec<String>) -> Result<u8, String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage());
    };
    match command {
        "check" => check(&args[1..]),
        "fmt" => format_files(&args[1..]),
        "compile" => compile(&args[1..]),
        "query" => query(&args[1..]),
        _ => Err(usage()),
    }
}

fn check(args: &[String]) -> Result<u8, String> {
    let (target, json_output, files) = parse_target_json(args, true)?;
    let system = CompilerSystem::new();
    for file in files {
        let result = if target == TargetArg::Auto {
            match system.check_file(&file, CompileTarget::View) {
                Ok(output) => Ok(output),
                Err(error) if error.message.contains("<App>") => {
                    system.check_file(&file, CompileTarget::App)
                }
                Err(error) => Err(error),
            }
        } else {
            system.check_file(&file, target.compile_target().expect("非 auto target"))
        };
        match result {
            Ok(_) => {
                if json_output {
                    println!("{}", json!({"path": file, "ok": true}));
                }
            }
            Err(error) => {
                print_diagnostic(&error, json_output)?;
                return Ok(1);
            }
        }
    }
    Ok(0)
}

fn format_files(args: &[String]) -> Result<u8, String> {
    let (check_only, files) = flag_files(args, "--check")?;
    if files.is_empty() {
        return Err("uix fmt 需要至少一个文件".into());
    }
    let system = CompilerSystem::new();
    let mut changed = false;
    for file in files {
        let output = system
            .format_file(&file)
            .map_err(|error| diagnostic_text(&error))?;
        changed |= output.changed;
        if output.changed && !check_only {
            fs::write(&file, output.formatted).map_err(|e| format!("{}: {e}", file.display()))?;
        }
        if check_only && output.changed {
            eprintln!("需要格式化: {}", file.display());
        }
    }
    Ok(u8::from(check_only && changed))
}

fn compile(args: &[String]) -> Result<u8, String> {
    let (target, _, files) = parse_target_json(args, false)?;
    let Some(file) = files.first() else {
        return Err("uix compile 需要一个文件".into());
    };
    let target = target.compile_target().ok_or("compile 不允许 auto")?;
    let output = CompilerSystem::new()
        .compile_file(file, target)
        .map_err(|error| diagnostic_text(&error))?;
    print!("{}", output.tokens);
    Ok(0)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TargetArg {
    Auto,
    View,
    App,
    Items,
}
impl TargetArg {
    fn compile_target(self) -> Option<CompileTarget> {
        match self {
            Self::Auto => None,
            Self::View => Some(CompileTarget::View),
            Self::App => Some(CompileTarget::App),
            Self::Items => Some(CompileTarget::Items),
        }
    }
}

fn parse_target_json(
    args: &[String],
    allow_auto: bool,
) -> Result<(TargetArg, bool, Vec<PathBuf>), String> {
    let mut target = TargetArg::Auto;
    let mut json_output = false;
    let mut files = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_output = true,
            "--target" => {
                i += 1;
                target = parse_target(args.get(i).ok_or("--target 缺少值")?)?;
            }
            value if value.starts_with('-') => return Err(format!("未知选项: {value}")),
            value => files.push(PathBuf::from(value)),
        }
        i += 1;
    }
    if !allow_auto && target == TargetArg::Auto {
        return Err("该命令必须指定 --target view|app|items".into());
    }
    if files.is_empty() {
        return Err("缺少输入文件".into());
    }
    Ok((target, json_output, files))
}

fn parse_target(value: &str) -> Result<TargetArg, String> {
    match value {
        "auto" => Ok(TargetArg::Auto),
        "view" => Ok(TargetArg::View),
        "app" => Ok(TargetArg::App),
        "items" => Ok(TargetArg::Items),
        _ => Err(format!("未知 target: {value}")),
    }
}
fn flag_files(args: &[String], flag: &str) -> Result<(bool, Vec<PathBuf>), String> {
    let mut check = false;
    let mut files = Vec::new();
    for value in args {
        if value == flag {
            check = true;
        } else if value.starts_with('-') {
            return Err(format!("未知选项: {value}"));
        } else {
            files.push(PathBuf::from(value));
        }
    }
    Ok((check, files))
}

fn query(args: &[String]) -> Result<u8, String> {
    let Some(kind) = args.first() else {
        return Err("uix query 需要类别".into());
    };
    let json_output = args.iter().any(|arg| arg == "--json");
    let schema = CompilerSystem::new().schema();
    let entries = match kind.as_str() {
        "components" => schema.components().iter().map(|e| json!({"id":e.id,"name":e.name,"status":format!("{:?}",e.status),"category":e.category.label(),"emitter":e.emitter,"capability":e.capability})).collect::<Vec<_>>(),
        "attributes" => schema.common_attributes().iter().map(|e| json!({"id":e.id,"name":e.name,"value_kind":format!("{:?}",e.value_kind)})).collect(),
        "events" => schema.events().iter().map(|e| json!({"id":e.id,"name":e.name,"fields":e.fields})).collect(),
        "styles" => schema.style_properties().iter().map(|e| json!({"id":e.id,"name":e.name,"value_kind":format!("{:?}",e.value_kind)})).collect(),
        "themes" => schema.theme_tokens().iter().map(|e| json!({"id":e.id,"name":e.name,"kind":format!("{:?}",e.kind),"alias_for":e.alias_for})).collect(),
        "handles" => schema.handle_slots().iter().map(|e| json!({"id":e.id,"component":e.component,"attribute":e.attribute,"value_type":e.value_type})).collect(),
        "slots" => schema.slots().iter().map(|e| json!({"id":e.id,"component":e.component,"name":e.name,"multiple":e.multiple})).collect(),
        "capabilities" => schema.capabilities().iter().map(|e| json!({"id":e.id,"name":e.name,"cargo_feature":e.cargo_feature})).collect(),
        "data" => schema.data_constructors().iter().map(|e| json!({"id":e.id,"name":e.name,"rust_path":e.rust_path,"method":e.method,"normalize_numbers":e.normalize_numbers,"capability":e.capability})).collect(),
        _ => return Err(format!("未知 query 类别: {kind}")),
    };
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())?
        );
    } else {
        for entry in entries {
            println!("{}", entry);
        }
    }
    Ok(0)
}

fn print_diagnostic(error: &CompilerDiagnostic, json_output: bool) -> Result<(), String> {
    if json_output {
        println!(
            "{}",
            serde_json::json!({"code":error.code,"phase":error.phase.as_str(),"path":error.source_name,"source_id":error.source_id.value(),"line":error.line,"column":error.column,"message":error.message,"suggestion":error.suggestion})
        );
    } else {
        eprintln!("{}", diagnostic_text(error));
    }
    Ok(())
}
fn diagnostic_text(error: &CompilerDiagnostic) -> String {
    format!(
        "{} {} {}:{}:{} {}\n建议: {}",
        error.code,
        error.phase.as_str(),
        error.source_name,
        error.line,
        error.column,
        error.message,
        error.suggestion
    )
}
fn usage() -> String {
    "用法: uix check|fmt|compile|query ...".into()
}
