//! ui 域集成测试。

mod common;

#[path = "ui/core.rs"]
mod core;

#[path = "ui/layout.rs"]
mod layout;

#[path = "ui/theme.rs"]
mod theme;

#[path = "ui/animation.rs"]
mod animation;

#[path = "ui/widgets.rs"]
mod widgets;

#[path = "ui/invalidation.rs"]
mod invalidation;

#[path = "ui/render_context.rs"]
mod render_context;

#[path = "ui/render_baseline.rs"]
mod render_baseline;

#[path = "ui/with_native.rs"]
mod with_native;
