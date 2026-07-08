//! UIX 演示程序。
//!
//! | 模式 | 命令 | 说明 |
//! |------|------|------|
//! | 简化 API | （默认） | `prelude` + `App::new()`，入门推荐 |
//! | 组件库 | `--dashboard` | `WidgetTree` + `run_widget_loop`，展示高级用法 |
//! | CLI | `--cli` | 各功能域 API 无 GUI 演示 |

pub mod cli;
pub mod dashboard;
pub mod simplified;
