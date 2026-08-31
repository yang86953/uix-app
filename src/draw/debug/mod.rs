//! 调试覆盖层（feature gate 预留）。

pub mod hud;
pub mod overlay;

pub use hud::{DebugHudState, HudLayout};
pub use overlay::{DebugFrameSnapshot, DebugRenderService};
