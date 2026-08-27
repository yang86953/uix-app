//! macOS Metal GPU-native swapchain Adapter。

#[path = "adapter/mod.rs"]
pub(crate) mod platform;

pub(crate) use platform::enumerate_adapters;
