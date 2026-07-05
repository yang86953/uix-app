//! UIX 演示程序。
//!
//! | 模式 | 命令 | 说明 |
//! |------|------|------|
//! | 简化 API | `--simple` | `prelude` + `App::new()`，入门推荐 |
//! | 组件库 | （默认） | `WidgetTree` + `run_widget_loop`，展示高级用法 |
//! | CLI | `--cli` | 各功能域 API 无 GUI 演示 |

pub mod cli;
pub mod dashboard;
pub mod simplified;
