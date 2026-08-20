pub(crate) mod display_link;
mod objc_runtime;
pub(crate) mod text_input_view;
pub(crate) mod window_delegate;

// 保留 `backends::macos::platform` 逻辑路径，根组装物理归入 host。
#[path = "host/mod.rs"]
pub mod platform;
