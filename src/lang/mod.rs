//! UIX 语言、构建和开发工具；生产运行时只编译显式需要的执行能力。
#[cfg(feature = "lang-build")]
pub mod build;
#[cfg(feature = "lang-build")]
pub mod compiler;
mod include;
#[cfg(feature = "lang-tools")]
pub mod lsp;
#[cfg(any(feature = "lang-build", feature = "uix-modules"))]
pub mod runtime;

#[cfg(feature = "lang-tools")]
pub mod demand;

#[cfg(feature = "uix-modules")]
pub mod modules;
