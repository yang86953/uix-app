// Auto-organized test modules. Tests live only under src/tests.

mod clipboard;
mod cursor;
mod display;
mod dpi;
mod filesystem;
mod frame_pacer;
mod gdi_presenter;
mod hardware_matrix;
mod ime_dispatch;
mod ime_foreground;
mod keyboard;
mod notification;
mod platform;
#[cfg(feature = "vulkan")]
mod software_recovery;
mod text_input;
mod tsf_session;
mod tsf_text_store;
mod util;
#[cfg(feature = "vulkan")]
mod vulkan_fault_recovery;
