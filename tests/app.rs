//! app 域集成测试。

mod common;

#[path = "app/application.rs"]
mod application;

#[path = "app/window.rs"]
mod window;

#[path = "app/cli_and_di.rs"]
mod cli_and_di;

#[cfg(feature = "test-harness")]
#[path = "app/event_loop.rs"]
mod event_loop;
