//! 平台公开契约。

pub mod display;
pub mod event;
pub mod input;
pub mod platform;
pub mod present;
pub mod system;
pub mod window;

pub use display::*;
pub use event::*;
pub use input::*;
pub use platform::Platform;
pub use present::*;
pub use system::*;
pub use window::*;
