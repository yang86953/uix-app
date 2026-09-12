//! 平台公开面的呈现与图形装配契约。
//!
//! 本模块把组合根提供的 recipe 装配输入与 surface 描述收口为 platform
//! 公开窄 API，draw/app 启动路径不接触具体原生模块。

// CPU 像素 presenter 是平台中立的窗口呈现端口。
mod presenter;
pub(crate) use presenter::IPresenter;

// recipe、surface owner 与呈现生命周期合同由 platform 物理拥有。
mod contracts;
pub(crate) use contracts::*;

// platform System 唯一拥有跨图形 API 复用的资源、命令、提交与 Surface 契约。
// Drawing 只依赖本模块；Vulkan 等 Adapter 只把这些事实机械映射到原生 API。
pub(crate) mod rhi;

// recipe 装配输入与 backend 描述只供 crate 内部启动路径消费。
#[cfg(feature = "platform")]
pub(crate) use super::composition_root::{
    GraphicsRecipe, describe_backend_availability, gpu_recipe_candidates,
    graphics_runtime_platform, try_create_gpu_recipe_with_queue,
};
