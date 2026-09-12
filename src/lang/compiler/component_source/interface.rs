//! 编译/编辑工具的只读原生接口文件，运行宿主仍须提供并核对真实绑定。
use super::*;
use crate::lang::compiler::{DiagnosticPhase, source_graph::SourceId};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
};

const MAX_BYTES: usize = 1_048_576;
mod json;
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    libraries: NativeLibraries,
}
/// 从原生库的同一 NativeProps 签名导出确定性接口；纯 .uix 声明不登记于此。
pub fn encode(libraries: &NativeLibraries) -> Result<String, CompilerDiagnostic> {
    validate(libraries, "<native interfaces>")?;
    let value = Manifest {
        version: 1,
        libraries: libraries.clone(),
    };
    let result = serde_json::to_string_pretty(&value)
        .map_err(|error| diagnostic("<native interfaces>", error.to_string()))?
        + "\n";
    if result.len() > MAX_BYTES {
        return Err(diagnostic("<native interfaces>", "原生接口文件超过 1 MiB"));
    }
    // serde_json 的有界 JSON 嵌套同样适用于导出，不能产生本接口无法再读取的文件。
    json::read(&result).map_err(|error| diagnostic("<native interfaces>", error.to_string()))?;
    Ok(result)
}
pub fn decode(source: &str, source_name: &str) -> Result<NativeLibraries, CompilerDiagnostic> {
    if source.len() > MAX_BYTES {
        return Err(diagnostic(source_name, "原生接口文件超过 1 MiB"));
    }
    let value = json::read(source).map_err(|error| {
        let mut result = diagnostic(source_name, error.to_string());
        result.line = error.line();
        let start = source
            .split_inclusive('\n')
            .take(error.line().saturating_sub(1))
            .map(str::len)
            .sum::<usize>();
        let mut offset = (start + error.column().saturating_sub(1)).min(source.len());
        while !source.is_char_boundary(offset) {
            offset -= 1;
        }
        result.start = offset;
        result.end = offset + source[offset..].chars().next().map_or(0, char::len_utf8);
        result.column = source[start.min(offset)..offset].chars().count() + 1;
        result
    })?;
    let manifest: Manifest = serde_json::from_value(value)
        .map_err(|error| diagnostic(source_name, error.to_string()))?;
    if manifest.version != 1 {
        return Err(diagnostic(
            source_name,
            format!("不支持的组件接口版本 {}", manifest.version),
        ));
    }
    validate(&manifest.libraries, source_name)?;
    Ok(manifest.libraries)
}
fn validate(libraries: &NativeLibraries, source: &str) -> Result<(), CompilerDiagnostic> {
    let mut cost = 0usize;
    for (package, exports) in libraries {
        for (name, export) in exports {
            cost = cost
                .saturating_add(check::native_export_cost(export).map_err(|(_, message)| {
                    diagnostic(source, format!("{package}::{name}: {message}"))
                })?)
                .saturating_add(1);
            if cost > 262144 {
                return Err(diagnostic(source, "原生接口分析预算耗尽"));
            }
        }
    }
    Ok(())
}
/// 最近 uix.json 的 component_libraries；未配置时为空，不猜测官方组件。
pub fn for_file(path: &Path) -> Result<NativeLibraries, CompilerDiagnostic> {
    Ok(load_project(path)?.libraries)
}
#[derive(Debug, Default)]
pub struct ProjectInterfaces {
    pub libraries: NativeLibraries,
    pub tracked_files: Vec<PathBuf>,
}
/// 工具可将配置/接口文件加入自己的失效跟踪。
pub fn load_project(path: &Path) -> Result<ProjectInterfaces, CompilerDiagnostic> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()
            .map_err(|error| diagnostic(&path.display().to_string(), error.to_string()))?
            .join(path)
    };
    let start = if absolute.is_dir() {
        absolute.as_path()
    } else {
        absolute.parent().unwrap_or(&absolute)
    };
    for directory in start.ancestors() {
        let config = directory.join("uix.json");
        if !config.is_file() {
            continue;
        }
        #[derive(Deserialize)]
        struct Config {
            #[serde(default)]
            component_libraries: Vec<PathBuf>,
        }
        let configuration: Config = serde_json::from_value(
            json::read(&read(&config)?)
                .map_err(|error| diagnostic(&config.display().to_string(), error.to_string()))?,
        )
        .map_err(|error| diagnostic(&config.display().to_string(), error.to_string()))?;
        if configuration.component_libraries.len() > 32 {
            return Err(diagnostic(
                &config.display().to_string(),
                "原生接口文件不能超过 32 份",
            ));
        }
        let mut output = ProjectInterfaces {
            libraries: BTreeMap::new(),
            tracked_files: vec![config],
        };
        let mut total = 0usize;
        for relative in configuration.component_libraries {
            let file = directory.join(relative);
            let canonical = file
                .canonicalize()
                .map_err(|error| diagnostic(&file.display().to_string(), error.to_string()))?;
            if output.tracked_files.contains(&canonical) {
                continue;
            }
            let source = read(&canonical)?;
            total = total.saturating_add(source.len());
            if total > 8 * MAX_BYTES {
                return Err(diagnostic(
                    &canonical.display().to_string(),
                    "原生接口闭包超过 8 MiB",
                ));
            }
            let libraries = decode(&source, &canonical.display().to_string())?;
            for (package, exports) in libraries {
                let target = output.libraries.entry(package.clone()).or_default();
                for (name, export) in exports {
                    if target.insert(name.clone(), export).is_some() {
                        return Err(diagnostic(
                            &canonical.display().to_string(),
                            format!("重复原生导出 {package}::{name}"),
                        ));
                    }
                }
            }
            output.tracked_files.push(canonical);
        }
        validate(&output.libraries, &directory.display().to_string())?;
        return Ok(output);
    }
    Ok(ProjectInterfaces::default())
}
fn read(path: &Path) -> Result<String, CompilerDiagnostic> {
    let name = path.display().to_string();
    let file = std::fs::File::open(path).map_err(|error| diagnostic(&name, error.to_string()))?;
    let mut bytes = Vec::new();
    file.take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| diagnostic(&name, error.to_string()))?;
    if bytes.len() > MAX_BYTES {
        return Err(diagnostic(&name, "工具接口/配置文件超过 1 MiB"));
    }
    String::from_utf8(bytes).map_err(|error| diagnostic(&name, error.to_string()))
}
fn diagnostic(name: &str, message: impl Into<String>) -> CompilerDiagnostic {
    CompilerDiagnostic { code: "component-interface", phase: DiagnosticPhase::Source, source_id: SourceId::from_source_name(name), source_name: name.into(), start: 0, end: 0, line: 1, column: 1, message: message.into(), suggestion: "从同一 Rust 原生属性声明生成接口文件，并在 uix.json 的 component_libraries 明确引用；运行宿主仍须绑定实际实现".into() }
}
