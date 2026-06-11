// ============================================================================
// services/api.rs — Services 层的公共 API 出口
//
// 本文件定义 services 层对外暴露的公共接口。其他层只能通过本文件
// 使用 services 层的功能，禁止直接引用内部模块。
// ============================================================================

pub use crate::services::file_service::FileService;
pub use crate::services::middleware::{
    LogMiddleware, Middleware, MiddlewareContext, MiddlewarePipeline, RetryMiddleware,
};
pub use crate::services::settings::SettingsService;
