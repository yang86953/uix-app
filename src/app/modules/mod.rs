//! 可移植 UIX 应用模块。实例、状态与宿主授权由应用持有。
//!
//! `uix-modules` 支持 AOT 产物；`uix-dynamic` 额外启用运行期源码分析。

pub use uix_lang_runtime::{
    AsyncCall, AsyncCompletion, AsyncHostPort, AsyncHostPorts, OperationId,
};
pub use uix_lang_runtime::{
    Cancellation, Effect, ErrorKind, HostPort, HostPorts, Instance, Limits, Location, Module,
    RuntimeError, RuntimeResult, Signature, Type, Value,
};
pub use uix_lang_runtime::{
    ModuleHandle, ModuleRequest, ModuleSnapshot, ModuleWorker, ViewKind, ViewSnapshot,
};
mod project;
pub use project::ModuleView;

/// 生成代码使用的共享合同，不作为应用手写执行计划入口。
#[doc(hidden)]
pub use uix_lang_runtime as __runtime;

#[cfg(feature = "uix-dynamic")]
pub use uix_lang_compiler::CompilerDiagnostic;

/// 解析并冻结明确提供的模块源码；失败不触碰任何运行实例。
#[cfg(feature = "uix-dynamic")]
pub fn load_module(source: &str, source_name: &str) -> Result<Module, CompilerDiagnostic> {
    uix_lang_compiler::modules::check_inline(source, source_name).map(|output| output.module)
}

/// 只装载应用明确指定的文件，不扫描或监听目录。
#[cfg(feature = "uix-dynamic")]
pub fn load_module_file(path: &std::path::Path) -> Result<Module, CompilerDiagnostic> {
    uix_lang_compiler::modules::check_file(path).map(|output| output.module)
}
