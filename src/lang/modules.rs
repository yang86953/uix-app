//! 可移植 UIX 应用模块。实例、状态与宿主授权由应用持有。
//!
//! `uix-modules` 支持 AOT 产物；`uix-dynamic` 额外启用运行期源码分析。

pub use crate::lang::runtime::{
    AsyncCall, AsyncCompletion, AsyncHostPort, AsyncHostPorts, OperationId,
};
pub use crate::lang::runtime::{
    Cancellation, Effect, ErrorKind, HostPort, HostPorts, Instance, Limits, Location, Module,
    RuntimeError, RuntimeResult, Signature, Type, Value,
};
pub use crate::lang::runtime::{
    ModuleHandle, ModuleRequest, ModuleSnapshot, ModuleWorker, ViewKind, ViewSnapshot,
};

/// 生成代码使用的共享合同，不作为应用手写执行计划入口。
#[doc(hidden)]
pub use crate::lang::runtime as __runtime;

#[cfg(feature = "uix-dynamic")]
pub use crate::lang::compiler::CompilerDiagnostic;

/// 解析并冻结明确提供的模块源码；失败不触碰任何运行实例。
#[cfg(feature = "uix-dynamic")]
pub fn load_module(source: &str, source_name: &str) -> Result<Module, CompilerDiagnostic> {
    dynamic_load(source_name, || {
        crate::lang::compiler::modules::check_inline(source, source_name)
    })
}

/// 只装载应用明确指定的文件，不扫描或监听目录。
#[cfg(feature = "uix-dynamic")]
pub fn load_module_file(path: &std::path::Path) -> Result<Module, CompilerDiagnostic> {
    dynamic_load(&path.to_string_lossy(), || {
        crate::lang::compiler::modules::check_file(path)
    })
}

#[cfg(feature = "uix-dynamic")]
fn dynamic_load(
    name: &str,
    check: impl FnOnce() -> Result<crate::lang::compiler::modules::ModuleOutput, CompilerDiagnostic>,
) -> Result<Module, CompilerDiagnostic> {
    use crate::lang::compiler::{DiagnosticPhase, components};
    let error = |message: String| CompilerDiagnostic {
        code: "UIX2200",
        phase: DiagnosticPhase::Semantic,
        source_id: crate::lang::compiler::source_graph::SourceId::from_source_name(name),
        source_name: name.into(),
        start: 0,
        end: 0,
        line: 1,
        column: 1,
        message,
        suggestion: "Declare the required units in uix.json dynamic and rebuild.".into(),
    };
    crate::build_policy::require_dynamic("uix-app/uix-dynamic").map_err(&error)?;
    let (output, units) = if crate::build_policy::STRICT {
        let libraries: Vec<serde_json::Value> =
            serde_json::from_str(crate::build_policy::LIBRARIES)
                .map_err(|e| error(e.to_string()))?;
        let mut catalog = components::ComponentCatalog::new();
        for library in libraries {
            catalog
                .read_library_source(&library.to_string())
                .map_err(&error)?;
        }
        std::sync::Arc::new(catalog).with(|| components::capture_units(check))
    } else {
        components::capture_units(check)
    };
    let output = output?;
    for unit in units {
        crate::build_policy::require_dynamic(&unit).map_err(&error)?;
    }
    Ok(output.module)
}
