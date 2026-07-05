//! 图形引擎 trait 体系（契约定义于 `api::render`）。

pub use crate::api::render::{
    Canvas2D, GraphicsCapabilities, GraphicsEngine, PresentationMode, RenderingBackend,
    UpdateStrategy,
};