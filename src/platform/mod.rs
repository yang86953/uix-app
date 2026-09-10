//! UIX 的稳定原生平台能力公开面。
//!
//! `crate::native` 保留窗口和图形实现；平台私有 adapter 封装已迁入的 OS
//! 差异，应用只通过本模块的 owned 描述值与线程亲和 [`Platform`] 消费能力。

// 具体 OS adapter 物理归属 Platform System，并按能力 feature 编译。
pub(crate) mod adapters;

// Platform System 的组合门面保留在根目录，不归属于任何单一功能域。
mod facade;
// 原生启动输入保持平台中立，并以所有权一次性交付组合根。
mod composition;
// 只有本叶同时依赖中立合同与具体平台实现。
mod composition_root;
// Platform System 根合同由中立叶唯一持有，原生后端只负责实现与组装。
pub(crate) mod platform;
// 显示器信息、DPI 与主题查询由平台中立叶唯一持有。
pub(crate) mod display;
// Platform System 的实例生命周期与主线程约束集中在根级 runtime 边界。
mod runtime;
// 三项无附属值类型的低层系统服务合同由单一内部叶持有。
pub(crate) mod system;
// host 功能域拥有宿主机信息、文件系统、对话框与通知能力。
#[path = "host/mod.rs"]
// 保持公开 `platform::capabilities` 模块路径不变。
pub mod capabilities;
// 将文件对话框验证组件定位到 host 功能域。
#[path = "host/file_dialog.rs"]
// 保持内部 `platform::file_dialog` 逻辑路径不变。
mod file_dialog;
// 将公开硬件描述契约定位到 host 功能域。
#[path = "host/hardware.rs"]
// 保持公开 `platform::hardware` 模块路径不变。
pub mod hardware;
// 将公开系统服务契约定位到 host 功能域。
#[path = "host/services.rs"]
// 保持公开 `platform::services` 模块路径不变。
pub mod services;
// 将公开图形选择值定位到 presentation 功能域。
#[path = "presentation/graphics.rs"]
// 保持公开 `platform::graphics` 模块路径不变。
pub mod graphics;
// presentation 功能域拥有图形配方与原生呈现装配边界。
pub mod presentation;
// windowing 功能域拥有输入值、剪贴板与事件循环唤醒契约。
pub mod windowing;

pub(crate) use composition::PendingNativeOptions;
pub(crate) use composition_root::create_platform_with_pending;
#[cfg(feature = "agent-control")]
pub(crate) use composition_root::native_font_system_info;
pub use facade::Platform;
// 继续从 platform 根公开 UI 线程入口，同时让实现物理归属 windowing 域。
pub use windowing::ui_thread::run_on_ui_thread;
