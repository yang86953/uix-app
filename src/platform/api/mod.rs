//! # uix-platform 稳定公开 API
//!
//! 按功能域划分的平台契约。
//!
//! | 模块 | 职责 |
//! |------|------|
//! | [`error`] | 错误码、Error、Result 扩展 |
//! | [`geometry`] | Point / Size / Rect / EdgeInsets |
//! | [`event`] | UiEvent、EventBus、IEventLoop |
//! | [`window`] | 窗口创建、属性、PlatformWindow |
//! | [`present`] | IPresenter、IGraphicsContext、PresentDamage |
//! | [`input`] | 键盘、鼠标、光标、剪贴板、IME |
//! | [`display`] | 显示器信息与 DPI |
//! | [`system`] | 文件、对话框、通知、定时器、控制台、系统信息 |
//! | [`platform`] | Platform 聚合根接口 |

pub mod display;
pub mod error;
pub mod event;
pub mod geometry;
pub mod input;
pub mod platform;
pub mod present;
pub mod system;
pub mod window;

pub use display::*;
pub use error::*;
pub use event::*;
pub use geometry::*;
pub use input::*;
pub use platform::Platform;
pub use present::*;
pub use system::*;
pub use window::*;
