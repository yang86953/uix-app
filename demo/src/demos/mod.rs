//! UIX 演示程序 — UIX 框架的命令行与 GUI 功能展示。
//! 这些是仅二进制模块（不属于 `uix` 库 crate）。

pub mod cli;

// 跨平台 GUI 演示 — 平台抽象层处理平台差异。
pub mod dashboard;
