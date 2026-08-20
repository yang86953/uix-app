//! core `identity` 模块：跨 ui、graphics、platform、app 传递的稳定身份值。
//!
//! SMC 边界：`WidgetId` / `WindowId` 只表达身份，不持有节点、不延长
//! 生命周期；解析身份必须由拥有存储的上层完成。本模块与 `geometry` /
//! `error` 彼此独立，不依赖兄弟模块。

mod widget_id;
mod window_id;

pub use widget_id::WidgetId;
pub use window_id::WindowId;
