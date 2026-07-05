//! app 域集成测试。

mod common;

#[path = "app/application.rs"]
mod application;

#[path = "app/window.rs"]
mod window;

#[path = "app/cli_and_di.rs"]
mod cli_and_di;
