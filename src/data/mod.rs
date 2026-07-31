//! 数据能力 — 配置持久化。
//!
//! # SMC 边界（SMC-05）
//!
//! 本模块是 data System 的公开边界与私有实现。目标 Module 与依赖：
//!
//! | Module | 职责 | 依赖 |
//! |--------|------|------|
//! | [`settings`] | 键值设置服务（JSON 持久化，冷路径配置能力） | 无（只依赖 core 失败模型；`settings-serde` feature 下依赖 serde） |
//!
//! Module 为 `pub(crate)` 私有边界，公开面收口于本根
//! （`SettingsService`）；无兄弟 Module，跨 System 依赖为零。

pub(crate) mod settings;

pub use settings::SettingsService;
