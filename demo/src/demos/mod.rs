//! UIX 演示程序 — UIX 框架的命令行与 GUI 功能展示。
//! 这些是仅二进制模块（不属于 `uix` 库 crate）。

pub mod cli;

// 跨平台 GUI 演示 — 平台抽象层处理平台差异。
pub mod dashboard;

// 简化 API 演示 — View 组合子 + State 自动脏标记 + App 一键启动。
pub mod simplified;
