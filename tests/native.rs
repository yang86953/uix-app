//! native 域集成测试。

mod common;

#[path = "native/api.rs"]
mod api;

#[path = "native/event.rs"]
mod event;

#[path = "native/event_bus.rs"]
mod event_bus;

#[path = "native/shared.rs"]
mod shared;

#[path = "native/smoke.rs"]
mod smoke;

#[path = "native/types.rs"]
mod types;

#[path = "native/platform_integration.rs"]
mod platform_integration;
